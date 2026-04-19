#![allow(dead_code)]

use std::path::Path;
use anyhow::Result;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use nom_exif::{MediaParser, MediaSource, ExifTag, TrackInfoTag, EntryValue};
use super::TimeSource;

fn parse_exif_dt(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s.trim(), "%Y:%m:%d %H:%M:%S").ok()
}

pub fn extract_time(path: &Path) -> Result<(DateTime<Local>, TimeSource)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let is_video = matches!(
        ext.as_str(),
        "mp4" | "m4v" | "mov" | "3gp" | "3g2" | "avi" | "mkv"
            | "mpeg" | "mpg" | "mpe" | "webm" | "wmv" | "flv" | "mts" | "m2ts"
    );

    if is_video {
        if let Ok(r) = try_video_time(path) {
            return Ok(r);
        }
    } else if let Ok(r) = try_image_exif(path) {
        return Ok(r);
    }

    // Fallback: filesystem time
    let meta = std::fs::metadata(path)?;
    if let Ok(mtime) = meta.modified() {
        return Ok((DateTime::from(mtime), TimeSource::FileModified));
    }
    if let Ok(ctime) = meta.created() {
        return Ok((DateTime::from(ctime), TimeSource::FileCreated));
    }
    anyhow::bail!("无法获取文件时间: {}", path.display())
}

fn try_image_exif(path: &Path) -> Result<(DateTime<Local>, TimeSource)> {
    let ms = MediaSource::file_path(path)?;
    let mut parser = MediaParser::new();
    let iter: nom_exif::ExifIter = parser.parse(ms)?;

    let mut dt_original: Option<NaiveDateTime> = None;
    let mut dt_digitized: Option<NaiveDateTime> = None;

    for entry in iter {
        match entry.tag() {
            Some(ExifTag::DateTimeOriginal) => {
                if let Some(v) = entry.get_value() {
                    dt_original = parse_exif_dt(&v.to_string());
                }
            }
            Some(ExifTag::CreateDate) => {
                if let Some(v) = entry.get_value() {
                    dt_digitized = parse_exif_dt(&v.to_string());
                }
            }
            _ => {}
        }
    }

    if let Some(dt) = dt_original.or(dt_digitized) {
        let local = Local
            .from_local_datetime(&dt)
            .single()
            .unwrap_or_else(|| Local::now());
        return Ok((local, TimeSource::ExifDateTime));
    }

    anyhow::bail!("no EXIF datetime found")
}

fn try_video_time(path: &Path) -> Result<(DateTime<Local>, TimeSource)> {
    let ms = MediaSource::file_path(path)?;
    let mut parser = MediaParser::new();
    let info: nom_exif::TrackInfo = parser.parse(ms)?;

    if let Some(v) = info.get(TrackInfoTag::CreateDate) {
        match v {
            EntryValue::Time(dt) => {
                return Ok((dt.with_timezone(&Local), TimeSource::ExifDateTime));
            }
            EntryValue::NaiveDateTime(ndt) => {
                // Treat as UTC, convert to local
                let utc = chrono::Utc.from_utc_datetime(ndt);
                return Ok((utc.with_timezone(&Local), TimeSource::ExifDateTime));
            }
            _ => {
                // Fallback: try parsing the Display representation
                let s = v.to_string();
                if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
                    return Ok((dt.with_timezone(&Local), TimeSource::ExifDateTime));
                }
            }
        }
    }

    anyhow::bail!("no video creation_time found")
}

/// Read the iOS Live Photo ContentIdentifier from EXIF.
/// Apple stores it as tag 0x9999 in the MakerNote sub-IFD.
pub fn read_content_identifier(path: &Path) -> Option<String> {
    let ms = MediaSource::file_path(path).ok()?;
    let mut parser = MediaParser::new();
    let iter: nom_exif::ExifIter = parser.parse(ms).ok()?;

    for entry in iter {
        // ContentIdentifier is an unrecognized tag (0x9999), matched by raw code
        if entry.tag_code() == 0x9999 {
            if let Some(v) = entry.get_value() {
                let s = v.to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}
