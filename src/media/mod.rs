#![allow(dead_code)]

use std::path::PathBuf;
use chrono::{DateTime, Local};

pub mod format;
pub mod metadata;
pub mod livephoto;
#[cfg(test)]
mod format_test;

#[derive(Debug, Clone, PartialEq)]
pub enum MediaType {
    Img,
    Vdo,
    Lpo,
}

impl MediaType {
    pub fn prefix(&self) -> &'static str {
        match self {
            MediaType::Img => "Img",
            MediaType::Vdo => "Vdo",
            MediaType::Lpo => "Lpo",
        }
    }
}

#[derive(Debug, Clone)]
pub enum TimeSource {
    ExifDateTime,
    ExifGpsDate,
    FileModified,
    FileCreated,
}

#[derive(Debug, Clone)]
pub struct MediaFile {
    pub source_path: PathBuf,
    pub media_type: MediaType,
    pub capture_time: DateTime<Local>,
    pub time_source: TimeSource,
    pub content_id: Option<String>,
    pub is_motion_photo: bool,
    pub live_pair: Option<PathBuf>,
    pub file_hash: Option<String>,
    pub file_size: u64,
}
