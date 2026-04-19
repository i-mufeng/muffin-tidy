#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use chrono::{DateTime, Local};
use filetime::{set_file_times, FileTime};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{info, warn, error};
use crate::media::{MediaFile, MediaType};
use crate::dedup::DedupRegistry;
use crate::logger;

pub struct ExportPlan {
    pub src: PathBuf,
    pub dst: PathBuf,
}

pub struct ExportStats {
    pub exported_img: u32,
    pub exported_vdo: u32,
    pub exported_lpo: u32,
    pub skipped_dedup: u32,
    pub skipped_conflict: u32,
    pub skipped_error: u32,
}

impl ExportStats {
    pub fn new() -> Self {
        Self { exported_img: 0, exported_vdo: 0, exported_lpo: 0, skipped_dedup: 0, skipped_conflict: 0, skipped_error: 0 }
    }
    pub fn total_exported(&self) -> u32 {
        self.exported_img + self.exported_vdo + self.exported_lpo
    }
    pub fn total_skipped(&self) -> u32 {
        self.skipped_dedup + self.skipped_conflict + self.skipped_error
    }
}

pub fn resolve_output_path(dir: &Path, type_prefix: &str, ts: &DateTime<Local>, ext: &str) -> PathBuf {
    let base = format!("{}-{}", type_prefix, ts.format("%Y%m%d%H%M%S"));
    let mut seq = 1u32;
    loop {
        let filename = format!("{}-{:02}.{}", base, seq, ext.to_lowercase());
        let candidate = dir.join(&filename);
        if !candidate.exists() {
            return candidate;
        }
        seq += 1;
    }
}

/// Returns the resolved destination path, or `None` if the source is identical
/// to an existing file at that path (same-content conflict → skip).
pub fn resolve_output_path_dedup(
    src: &Path,
    dir: &Path,
    type_prefix: &str,
    ts: &DateTime<Local>,
    ext: &str,
) -> Result<Option<PathBuf>> {
    let base = format!("{}-{}", type_prefix, ts.format("%Y%m%d%H%M%S"));
    let mut seq = 1u32;
    loop {
        let filename = format!("{}-{:02}.{}", base, seq, ext.to_lowercase());
        let candidate = dir.join(&filename);
        if !candidate.exists() {
            return Ok(Some(candidate));
        }
        // Candidate exists — compare content hashes
        if files_identical(src, &candidate)? {
            return Ok(None); // same content, skip
        }
        seq += 1;
    }
}

fn files_identical(a: &Path, b: &Path) -> Result<bool> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let hash_file = |p: &Path| -> Result<String> {
        let mut f = std::fs::File::open(p)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    };

    Ok(hash_file(a)? == hash_file(b)?)
}

/// Run the full export pipeline.
pub fn run(
    files: &[MediaFile],
    target: &Path,
    dry_run: bool,
    no_dedup: bool,
    no_conflict_check: bool,
) -> Result<ExportStats> {
    let mut registry = DedupRegistry::new();
    let mut stats = ExportStats::new();

    let pb = Arc::new(ProgressBar::new(files.len() as u64));
    pb.set_style(
        ProgressStyle::with_template(
            "  {spinner:.cyan} [{elapsed_precise}] [{bar:38.cyan/blue}] {pos}/{len} ({percent}%) {msg}"
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-"),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    logger::set_progress_bar(pb.clone());

    // Live Photo pairs: track which files have already been assigned a dst path
    // so the paired file can reuse the same sequence number.
    // Key: source_path of the "primary" file → assigned dst base (without ext)
    let mut lpo_pair_base: std::collections::HashMap<PathBuf, PathBuf> = std::collections::HashMap::new();

    for file in files {
        let filename = file.source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        pb.set_message(filename.to_string());

        // Dedup check
        if !no_dedup {
            if let Some(hash) = &file.file_hash {
                if registry.is_duplicate(hash) {
                    warn!(path = %file.source_path.display(), hash = %hash, "跳过重复文件");
                    stats.skipped_dedup += 1;
                    pb.inc(1);
                    continue;
                }
                registry.register(hash);
            }
        }

        let ext = file.source_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
            .to_lowercase();

        let year = file.capture_time.format("%Y").to_string();
        let month = file.capture_time.format("%m").to_string();
        let dir = target.join(&year).join(&month);

        // Resolve destination path
        let dst_opt = if matches!(file.media_type, MediaType::Lpo) {
            if let Some(pair_path) = &file.live_pair {
                if let Some(base) = lpo_pair_base.get(pair_path) {
                    let candidate = base.with_extension(&ext);
                    if candidate.exists() && !no_conflict_check {
                        match files_identical(&file.source_path, &candidate) {
                            Ok(true) => None,
                            Ok(false) => Some(candidate),
                            Err(e) => {
                                error!(src = %file.source_path.display(), error = %e, "文件写入失败");
                                stats.skipped_error += 1;
                                pb.inc(1);
                                continue;
                            }
                        }
                    } else {
                        Some(candidate)
                    }
                } else {
                    let opt = if no_conflict_check {
                        Ok(Some(resolve_output_path(&dir, file.media_type.prefix(), &file.capture_time, &ext)))
                    } else {
                        resolve_output_path_dedup(&file.source_path, &dir, file.media_type.prefix(), &file.capture_time, &ext)
                    };
                    match opt {
                        Ok(Some(ref p)) => {
                            lpo_pair_base.insert(file.source_path.clone(), p.with_extension(""));
                            opt.unwrap()
                        }
                        Ok(None) => None,
                        Err(e) => {
                            error!(src = %file.source_path.display(), error = %e, "文件写入失败");
                            stats.skipped_error += 1;
                            pb.inc(1);
                            continue;
                        }
                    }
                }
            } else if no_conflict_check {
                Some(resolve_output_path(&dir, file.media_type.prefix(), &file.capture_time, &ext))
            } else {
                match resolve_output_path_dedup(&file.source_path, &dir, file.media_type.prefix(), &file.capture_time, &ext) {
                    Ok(opt) => opt,
                    Err(e) => {
                        error!(src = %file.source_path.display(), error = %e, "文件写入失败");
                        stats.skipped_error += 1;
                        pb.inc(1);
                        continue;
                    }
                }
            }
        } else if no_conflict_check {
            Some(resolve_output_path(&dir, file.media_type.prefix(), &file.capture_time, &ext))
        } else {
            match resolve_output_path_dedup(&file.source_path, &dir, file.media_type.prefix(), &file.capture_time, &ext) {
                Ok(opt) => opt,
                Err(e) => {
                    error!(src = %file.source_path.display(), error = %e, "文件写入失败");
                    stats.skipped_error += 1;
                    pb.inc(1);
                    continue;
                }
            }
        };

        let dst = match dst_opt {
            None => {
                warn!(src = %file.source_path.display(), "目标已存在相同文件，跳过");
                stats.skipped_conflict += 1;
                pb.inc(1);
                continue;
            }
            Some(p) => p,
        };

        if dry_run {
            info!(src = %file.source_path.display(), dst = %dst.display(), "【试运行】导出文件");
        } else {
            if let Err(e) = copy_file(&file.source_path, &dst) {
                error!(src = %file.source_path.display(), error = %e, "文件写入失败");
                stats.skipped_error += 1;
                pb.inc(1);
                continue;
            }
            info!(src = %file.source_path.display(), dst = %dst.display(), "导出文件");
        }

        match file.media_type {
            MediaType::Img => stats.exported_img += 1,
            MediaType::Vdo => stats.exported_vdo += 1,
            MediaType::Lpo => stats.exported_lpo += 1,
        }
        pb.inc(1);
    }

    pb.finish_and_clear();
    logger::clear_progress_bar();
    Ok(stats)
}

fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, dst)?;

    // Preserve source timestamps
    let meta = std::fs::metadata(src)?;
    let mtime = FileTime::from_last_modification_time(&meta);
    let atime = FileTime::from_last_access_time(&meta);
    set_file_times(dst, atime, mtime)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn fixed_time() -> DateTime<Local> {
        Local.with_ymd_and_hms(2024, 5, 12, 14, 30, 22).unwrap()
    }

    #[test]
    fn resolve_path_format() {
        let dir = TempDir::new().unwrap();
        let p = resolve_output_path(dir.path(), "Img", &fixed_time(), "jpg");
        let name = p.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, "Img-20240512143022-01.jpg");
    }

    #[test]
    fn resolve_path_ext_lowercase() {
        let dir = TempDir::new().unwrap();
        let p = resolve_output_path(dir.path(), "Vdo", &fixed_time(), "MOV");
        let name = p.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, "Vdo-20240512143022-01.mov");
    }

    #[test]
    fn resolve_path_seq_increments() {
        let dir = TempDir::new().unwrap();
        let ts = fixed_time();
        // Create the first candidate so seq must increment
        let first = resolve_output_path(dir.path(), "Img", &ts, "jpg");
        std::fs::write(&first, b"").unwrap();
        let second = resolve_output_path(dir.path(), "Img", &ts, "jpg");
        let name = second.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, "Img-20240512143022-02.jpg");
    }

    #[test]
    fn export_stats_total() {
        let mut s = ExportStats::new();
        s.exported_img = 3;
        s.exported_vdo = 2;
        s.exported_lpo = 4;
        assert_eq!(s.total_exported(), 9);
    }
}
