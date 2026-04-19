#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;
use walkdir::WalkDir;
use tracing::{info, warn, debug};
use crate::config::{FiltersConfig, LivePhotoConfig};
use crate::media::{MediaFile, MediaType, format, metadata, livephoto};

pub fn scan(
    source: &Path,
    recursive: bool,
    max_depth: usize,
    include_hidden: bool,
    filters: &FiltersConfig,
    livephoto_cfg: &LivePhotoConfig,
) -> Result<Vec<MediaFile>> {
    info!(source = %source.display(), "开始扫描");

    let mut walker = WalkDir::new(source);
    if !recursive {
        walker = walker.max_depth(1);
    } else if max_depth > 0 {
        walker = walker.max_depth(max_depth);
    }

    let mut files: Vec<MediaFile> = Vec::new();

    for entry in walker.into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => { warn!(error = %e, "无法访问条目"); continue; }
        };

        let path = entry.path();
        if path.is_dir() { continue; }

        if !include_hidden {
            if entry.path().components().any(|c| {
                c.as_os_str().to_str().map(|s| s.starts_with('.')).unwrap_or(false)
            }) {
                continue;
            }
        }

        let media_type = match format::detect_media_type(path) {
            Some(t) => t,
            None => {
                debug!(path = %path.display(), "跳过不支持的格式");
                continue;
            }
        };

        let file_size = match std::fs::metadata(path) {
            Ok(m) => m.len(),
            Err(e) => { warn!(path = %path.display(), error = %e, "无法读取文件元数据"); continue; }
        };

        // Apply FiltersConfig
        {
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            if !filters.include_extensions.is_empty()
                && !filters.include_extensions.iter().any(|e| e.to_lowercase() == ext)
            {
                debug!(path = %path.display(), "跳过（扩展名不在白名单）");
                continue;
            }
            if filters.exclude_extensions.iter().any(|e| e.to_lowercase() == ext) {
                debug!(path = %path.display(), "跳过（扩展名在黑名单）");
                continue;
            }
            if filters.min_file_size > 0 && file_size < filters.min_file_size {
                debug!(path = %path.display(), size = file_size, "跳过（文件过小）");
                continue;
            }
            if filters.max_file_size > 0 && file_size > filters.max_file_size {
                debug!(path = %path.display(), size = file_size, "跳过（文件过大）");
                continue;
            }
        }

        let (capture_time, time_source) = match metadata::extract_time(path) {
            Ok(t) => t,
            Err(e) => { warn!(path = %path.display(), error = %e, "无法提取时间，跳过"); continue; }
        };

        // Check Android Motion Photo and read XMP ContentIdentifier in one file read
        let (is_motion_photo, xmp_content_id) = if matches!(media_type, MediaType::Img) && livephoto_cfg.enabled {
            livephoto::read_xmp_data(path)
        } else {
            (false, None)
        };

        // Read iOS ContentIdentifier from EXIF or XMP (XMP already read above)
        let content_id = if matches!(media_type, MediaType::Img) {
            metadata::read_content_identifier(path).or(xmp_content_id)
        } else if matches!(media_type, MediaType::Vdo) {
            // MOV files can also carry ContentIdentifier
            metadata::read_content_identifier(path)
        } else {
            None
        };

        let effective_type = if is_motion_photo && livephoto_cfg.android_motion_photo {
            MediaType::Lpo
        } else {
            media_type
        };

        debug!(path = %path.display(), media_type = ?effective_type, "发现文件");

        files.push(MediaFile {
            source_path: path.to_path_buf(),
            media_type: effective_type,
            capture_time,
            time_source,
            content_id,
            is_motion_photo,
            live_pair: None,
            file_hash: None,
            file_size,
        });
    }

    // Pair iOS Live Photos
    pair_live_photos(&mut files, livephoto_cfg);

    info!(count = files.len(), "扫描完成");
    Ok(files)
}

fn pair_live_photos(files: &mut Vec<MediaFile>, cfg: &LivePhotoConfig) {
    if !cfg.enabled {
        return;
    }

    // Strategy 1: pair by ContentIdentifier
    if cfg.match_by_content_id {
    let mut by_content_id: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        if let Some(id) = &f.content_id {
            by_content_id.entry(id.clone()).or_default().push(i);
        }
    }

    for (_id, indices) in &by_content_id {
        if indices.len() < 2 { continue; }
        // Find one image and one video in the group
        let img_idx = indices.iter().find(|&&i| {
            matches!(files[i].media_type, MediaType::Img)
        });
        let vdo_idx = indices.iter().find(|&&i| {
            matches!(files[i].media_type, MediaType::Vdo)
        });
        if let (Some(&ii), Some(&vi)) = (img_idx, vdo_idx) {
            let vdo_path = files[vi].source_path.clone();
            let img_path = files[ii].source_path.clone();
            files[ii].media_type = MediaType::Lpo;
            files[ii].live_pair = Some(vdo_path);
            files[vi].media_type = MediaType::Lpo;
            files[vi].live_pair = Some(img_path);
            debug!(img = %files[ii].source_path.display(), vdo = %files[vi].source_path.display(), "LivePhoto 配对成功 (ContentIdentifier)");
        }
    }
    }

    // Strategy 2: fallback — same directory, same stem, one image + one .mov
    if cfg.match_by_filename {
    // Build a map: (parent, stem) -> indices
    let mut by_stem: HashMap<(std::path::PathBuf, String), Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        if f.live_pair.is_some() { continue; } // already paired
        if let (Some(parent), Some(stem)) = (
            f.source_path.parent(),
            f.source_path.file_stem().and_then(|s| s.to_str()),
        ) {
            by_stem
                .entry((parent.to_path_buf(), stem.to_lowercase()))
                .or_default()
                .push(i);
        }
    }

    for (_key, indices) in &by_stem {
        if indices.len() < 2 { continue; }
        let img_idx = indices.iter().find(|&&i| {
            matches!(files[i].media_type, MediaType::Img)
        });
        let vdo_idx = indices.iter().find(|&&i| {
            let ext = files[i].source_path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            ext == "mov"
        });
        if let (Some(&ii), Some(&vi)) = (img_idx, vdo_idx) {
            let vdo_path = files[vi].source_path.clone();
            let img_path = files[ii].source_path.clone();
            files[ii].media_type = MediaType::Lpo;
            files[ii].live_pair = Some(vdo_path);
            files[vi].media_type = MediaType::Lpo;
            files[vi].live_pair = Some(img_path);
            debug!(img = %files[ii].source_path.display(), vdo = %files[vi].source_path.display(), "LivePhoto 配对成功 (文件名)");
        }
    }
    }
}
