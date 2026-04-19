# Muffin-Tidy

[中文文档](README_CN.md)

A cross-platform command-line tool for organizing media files, written in Rust.

Scans source directories for photos and videos, intelligently detects iOS Live Photos and Android Motion Photos, deduplicates by content hash, and copies files into a clean date-based directory structure — **without ever modifying the source**.

## Features

- Organizes photos and videos by capture date (`YYYY/MM/Type-YYYYMMDDHHmmss-nn.ext`)
- Extracts timestamps from EXIF, GPS metadata, and video container metadata
- Detects and pairs **iOS Live Photos** (HEIC/JPEG + MOV) via `ContentIdentifier`
- Detects **Android Motion Photos** (embedded video in JPEG via XMP)
- SHA-256 content deduplication — skips exact duplicates
- Dry-run mode — preview what would happen without copying anything
- Spring Boot-style colored terminal logs with structured fields
- Progress bar and summary report
- Parallel hash computation via Rayon

## Supported Formats

**Images:** JPEG, PNG, GIF, BMP, TIFF, HEIC/HEIF, WebP, and RAW formats (CR2, CR3, NEF, NRW, ARW, DNG, ORF, RW2, RAF, PEF, RWL, SRW, ...)

**Videos:** MP4, MOV, AVI, MKV, 3GP, MPEG, WebM, WMV, FLV, MTS/AVCHD, ...

**Live Photos:** iOS Live Photo pairs, Android Motion Photos

## Installation

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
# binary at: target/release/mtidy
```

Requires Rust 1.85+.

## Usage

```bash
# Basic usage
mtidy --source /path/to/photos --target /path/to/output

# Multiple source directories
mtidy --source /path/a --source /path/b --target /path/to/output

# Dry run (no files copied)
mtidy --source ./photos --target ./output --dry-run

# Set log level
MEDIASORT_LOG=debug mtidy --source ./photos --target ./output

# JSON log output
mtidy --source ./photos --target ./output --log-format json
```

## Output Structure

```
output/
└── 2024/
    └── 05/
        ├── Img-20240512143022-01.jpg
        ├── Vdo-20240512150000-01.mp4
        ├── Lpo-20240512160000-01.heic   ← Live Photo pair
        └── Lpo-20240512160000-01.mov    ← same sequence number
```

Types: `Img` (image), `Vdo` (video), `Lpo` (Live Photo / Motion Photo)

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-s, --source` | required | Source directory (repeatable) |
| `-t, --target` | required | Target directory |
| `-c, --config` | `mediasort.toml` | Config file path |
| `--dry-run` | false | Preview only, no copying |
| `--no-dedup` | false | Skip deduplication |
| `--recursive` | true | Recurse into subdirectories |
| `--max-depth` | 0 (unlimited) | Max recursion depth |
| `--include-hidden` | false | Include hidden files/dirs |
| `--threads` | 0 (auto) | Parallel threads for hashing |
| `--log-level` | info | error / warn / info / debug / trace |
| `--log-format` | text | text / json |
| `--log-file` | — | Also write logs to file |
| `--progress` | true | Show progress bar |
| `--summary` | true | Print summary report |

## Configuration File

```toml
[general]
recursive = true
max_depth = 0
include_hidden = false
threads = 0
dry_run = false

[logging]
level = "info"
format = "text"
file = ""
show_progress = true
show_summary = true

[export]
no_dedup = false
overwrite_existing = false

[filters]
include_extensions = []
exclude_extensions = []
min_file_size = 0
max_file_size = 0

[livephoto]
enabled = true
match_by_content_id = true
match_by_filename = true
android_motion_photo = true
```

## Summary Report

```
╔══════════════════════════════════════════════════╗
║           Muffin-Tidy 导出汇总报告                  ║
╠══════════════════════════════════════════════════╣
║ 扫描目录        /Volumes/iPhone/DCIM             ║
║ 目标目录        /Volumes/Backup/Photos           ║
║ 耗时            12.3s                            ║
╠══════════════════════════════════════════════════╣
║ 发现文件总数    1,024                            ║
║ 成功导出        891                              ║
║   - 图片 (Img)  512                              ║
║   - 视频 (Vdo)  201                              ║
║   - 实况 (Lpo)  178 (89 组)                      ║
║ 跳过（重复）    88                               ║
║ 跳过（格式）    45                               ║
║ 错误            0                                ║
╚══════════════════════════════════════════════════╝
```

## License

MIT
