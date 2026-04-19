#![allow(dead_code)]

use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::{Sha256, Digest};
use tracing::warn;
use crate::logger;
use crate::media::MediaFile;

pub struct DedupRegistry {
    seen_hashes: HashSet<String>,
}

impl DedupRegistry {
    pub fn new() -> Self {
        Self { seen_hashes: HashSet::new() }
    }

    pub fn is_duplicate(&self, hash: &str) -> bool {
        self.seen_hashes.contains(hash)
    }

    pub fn register(&mut self, hash: &str) {
        self.seen_hashes.insert(hash.to_string());
    }
}

pub fn compute_hash(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Compute hashes for all files in parallel using rayon, with a progress bar.
/// `threads`: 0 = auto (capped at 4 to avoid I/O flooding), otherwise use specified count.
pub fn compute_hashes_parallel(files: &mut Vec<MediaFile>, threads: usize) {
    if files.is_empty() {
        return;
    }

    // Hash computation is I/O-bound; cap at 4 to avoid disk flooding.
    let num_threads = match threads {
        0 => 4.min(rayon::current_num_threads()),
        n => n,
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap());

    let total_files = files.len();
    let total_bytes: u64 = files.iter().map(|f| f.file_size).sum();

    let pb = Arc::new(ProgressBar::new(total_bytes));
    pb.set_style(
        ProgressStyle::with_template(
            "  {spinner:.cyan} [{elapsed_precise}] [{bar:38.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}"
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-"),
    );
    pb.set_message(format!("0/{} 文件", total_files));
    pb.enable_steady_tick(Duration::from_millis(80));
    logger::set_progress_bar(pb.clone());

    let completed = Arc::new(AtomicUsize::new(0));

    let hashes: Vec<Option<String>> = pool.install(|| {
        files
            .par_iter()
            .map(|f| {
                let result = match compute_hash(&f.source_path) {
                    Ok(h) => Some(h),
                    Err(e) => {
                        warn!(path = %f.source_path.display(), error = %e, "哈希计算失败");
                        None
                    }
                };
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                pb.inc(f.file_size);
                pb.set_message(format!("{}/{} 文件", done, total_files));
                result
            })
            .collect()
    });

    pb.finish_and_clear();
    logger::clear_progress_bar();

    for (f, h) in files.iter_mut().zip(hashes) {
        f.file_hash = h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp(content: &[u8]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content).unwrap();
        f
    }

    #[test]
    fn same_content_same_hash() {
        let a = write_temp(b"hello world");
        let b = write_temp(b"hello world");
        assert_eq!(compute_hash(a.path()).unwrap(), compute_hash(b.path()).unwrap());
    }

    #[test]
    fn different_content_different_hash() {
        let a = write_temp(b"hello");
        let b = write_temp(b"world");
        assert_ne!(compute_hash(a.path()).unwrap(), compute_hash(b.path()).unwrap());
    }

    #[test]
    fn dedup_registry() {
        let mut reg = DedupRegistry::new();
        assert!(!reg.is_duplicate("abc123"));
        reg.register("abc123");
        assert!(reg.is_duplicate("abc123"));
        assert!(!reg.is_duplicate("def456"));
    }
}
