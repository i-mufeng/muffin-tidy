use std::fs;
use std::path::Path;
use tempfile::TempDir;

// Minimal valid JPEG (1x1 pixel, no EXIF — falls back to file mtime for timestamp)
fn minimal_jpeg() -> Vec<u8> {
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
        0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
        0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
        0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
        0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
        0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0A, 0x0B, 0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03,
        0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
        0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06,
        0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
        0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72,
        0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
        0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
        0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
        0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75,
        0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3,
        0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
        0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9,
        0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
        0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
        0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01,
        0x00, 0x00, 0x3F, 0x00, 0xFB, 0xD2, 0x8A, 0x28, 0x03, 0xFF, 0xD9,
    ]
}

fn mtidy_bin() -> std::path::PathBuf {
    // Prefer the debug build produced by `cargo test`
    let mut p = std::env::current_exe().unwrap();
    // current_exe is something like target/debug/deps/integration_test-xxxx
    // Walk up to target/debug/
    p.pop(); // deps/
    p.pop(); // debug/
    p.push("mtidy");
    if cfg!(windows) { p.set_extension("exe"); }
    p
}

fn count_files(dir: &Path) -> usize {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count()
}

fn sha256_file(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = fs::File::open(path).unwrap();
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = f.read(&mut buf).unwrap();
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    hex::encode(hasher.finalize())
}

#[test]
fn dry_run_creates_no_files() {
    let src_dir = TempDir::new().unwrap();
    fs::write(src_dir.path().join("photo.jpg"), minimal_jpeg()).unwrap();
    let dst_dir = TempDir::new().unwrap();

    let status = std::process::Command::new(mtidy_bin())
        .args([
            src_dir.path().to_str().unwrap(),
            dst_dir.path().to_str().unwrap(),
            "--dry-run",
            "--log-level", "error",
        ])
        .status()
        .expect("failed to run mtidy");

    assert!(status.success(), "mtidy exited with non-zero status");
    assert_eq!(count_files(dst_dir.path()), 0, "dry_run must not create any files");
}

#[test]
fn source_files_unchanged_after_export() {
    let src_dir = TempDir::new().unwrap();
    let img_path = src_dir.path().join("snap.jpg");
    fs::write(&img_path, minimal_jpeg()).unwrap();
    let dst_dir = TempDir::new().unwrap();

    let hash_before = sha256_file(&img_path);

    let status = std::process::Command::new(mtidy_bin())
        .args([
            src_dir.path().to_str().unwrap(),
            dst_dir.path().to_str().unwrap(),
            "--log-level", "error",
        ])
        .status()
        .expect("failed to run mtidy");

    assert!(status.success());
    assert_eq!(sha256_file(&img_path), hash_before, "source file must not be modified");
}

#[test]
fn exported_filename_matches_format() {
    let src_dir = TempDir::new().unwrap();
    fs::write(src_dir.path().join("img.jpg"), minimal_jpeg()).unwrap();
    let dst_dir = TempDir::new().unwrap();

    let status = std::process::Command::new(mtidy_bin())
        .args([
            src_dir.path().to_str().unwrap(),
            dst_dir.path().to_str().unwrap(),
            "--log-level", "error",
        ])
        .status()
        .expect("failed to run mtidy");

    assert!(status.success());

    let exported: Vec<_> = walkdir::WalkDir::new(dst_dir.path())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    assert_eq!(exported.len(), 1, "expected exactly one exported file");
    let name = exported[0].file_name().to_str().unwrap();
    // Format: {Type}-{yyyyMMddHHmmss}-{nn}.{ext}
    let valid = name.starts_with("Img-") || name.starts_with("Vdo-") || name.starts_with("Lpo-");
    assert!(valid, "filename '{}' must start with Img-/Vdo-/Lpo-", name);
    // Check timestamp portion: 14 digits
    let parts: Vec<&str> = name.splitn(3, '-').collect();
    assert_eq!(parts.len(), 3, "filename '{}' must have 3 dash-separated parts", name);
    assert_eq!(parts[1].len(), 14, "timestamp in '{}' must be 14 digits", name);
    assert!(parts[1].chars().all(|c| c.is_ascii_digit()), "timestamp must be all digits");
}

#[test]
fn dedup_skips_identical_files() {
    let src_dir = TempDir::new().unwrap();
    // Two files with identical content
    fs::write(src_dir.path().join("a.jpg"), minimal_jpeg()).unwrap();
    fs::write(src_dir.path().join("b.jpg"), minimal_jpeg()).unwrap();
    let dst_dir = TempDir::new().unwrap();

    let status = std::process::Command::new(mtidy_bin())
        .args([
            src_dir.path().to_str().unwrap(),
            dst_dir.path().to_str().unwrap(),
            "--log-level", "error",
        ])
        .status()
        .expect("failed to run mtidy");

    assert!(status.success());
    // Only one of the two identical files should be exported
    assert_eq!(count_files(dst_dir.path()), 1, "dedup should export only one of two identical files");
}
