#![allow(dead_code)]

use std::path::Path;
use super::MediaType;

/// Recognized image extensions (lowercase)
const IMG_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "gif", "bmp", "tif", "tiff",
    "heic", "heif", "webp",
    "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "dng",
    "orf", "rw2", "raf", "pef", "ptx", "rwl", "srw",
];

/// Recognized video extensions (lowercase)
const VDO_EXTS: &[&str] = &[
    "mp4", "m4v", "mov", "avi", "mkv", "3gp", "3g2",
    "mpeg", "mpg", "mpe", "webm", "wmv", "flv", "mts", "m2ts",
];

pub fn detect_media_type(path: &Path) -> Option<MediaType> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if IMG_EXTS.contains(&ext.as_str()) {
        Some(MediaType::Img)
    } else if VDO_EXTS.contains(&ext.as_str()) {
        Some(MediaType::Vdo)
    } else {
        None
    }
}

pub fn is_supported(path: &Path) -> bool {
    detect_media_type(path).is_some()
}
