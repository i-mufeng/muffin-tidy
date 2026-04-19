# Muffin-Tidy

[English](README.md)

用 Rust 编写的跨平台媒体文件整理命令行工具。

扫描源目录中的照片和视频，智能识别 iOS 实况照片与 Android 运动照片，通过内容哈希去重，并将文件按日期目录结构复制到目标位置 — **绝不修改源文件**。

## 功能特性

- 按拍摄日期整理照片和视频（`YYYY/MM/类型-YYYYMMDDHHmmss-nn.ext`）
- 从 EXIF、GPS 元数据及视频容器元数据中提取时间戳
- 通过 `ContentIdentifier` 识别并配对 **iOS 实况照片**（HEIC/JPEG + MOV）
- 通过 XMP 元数据识别 **Android 运动照片**（JPEG 内嵌视频）
- SHA-256 内容去重 — 自动跳过完全相同的文件
- 试运行模式 — 预览操作计划，不执行任何复制
- 仿 Spring Boot 风格的彩色结构化终端日志
- 进度条与汇总报告
- 基于 Rayon 的并行哈希计算

## 支持格式

**图片：** JPEG、PNG、GIF、BMP、TIFF、HEIC/HEIF、WebP，以及各品牌 RAW 格式（CR2、CR3、NEF、NRW、ARW、DNG、ORF、RW2、RAF、PEF、RWL、SRW……）

**视频：** MP4、MOV、AVI、MKV、3GP、MPEG、WebM、WMV、FLV、MTS/AVCHD……

**实况照片：** iOS 实况照片对、Android 运动照片

## 安装

```bash
cargo install --path .
```

或手动构建：

```bash
cargo build --release
# 二进制文件位于：target/release/mtidy
```

需要 Rust 1.85+。

## 使用方法

```bash
# 基本用法
mtidy --source /path/to/photos --target /path/to/output

# 多个源目录
mtidy --source /path/a --source /path/b --target /path/to/output

# 试运行（不复制文件）
mtidy --source ./photos --target ./output --dry-run

# 设置日志等级
MEDIASORT_LOG=debug mtidy --source ./photos --target ./output

# JSON 格式日志
mtidy --source ./photos --target ./output --log-format json
```

## 输出目录结构

```
output/
└── 2024/
    └── 05/
        ├── Img-20240512143022-01.jpg
        ├── Vdo-20240512150000-01.mp4
        ├── Lpo-20240512160000-01.heic   ← 实况照片对
        └── Lpo-20240512160000-01.mov    ← 与上方序号对齐
```

类型前缀：`Img`（图片）、`Vdo`（视频）、`Lpo`（实况照片 / 运动照片）

## 参数说明

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `-s, --source` | 必填 | 源目录（可重复指定多个） |
| `-t, --target` | 必填 | 目标目录 |
| `-c, --config` | `mediasort.toml` | 配置文件路径 |
| `--dry-run` | false | 仅预览，不复制 |
| `--no-dedup` | false | 跳过去重检查 |
| `--recursive` | true | 递归扫描子目录 |
| `--max-depth` | 0（不限） | 最大递归深度 |
| `--include-hidden` | false | 包含隐藏文件/目录 |
| `--threads` | 0（自动） | 哈希计算并行线程数 |
| `--log-level` | info | error / warn / info / debug / trace |
| `--log-format` | text | text / json |
| `--log-file` | — | 同时输出日志到文件 |
| `--progress` | true | 显示进度条 |
| `--summary` | true | 输出汇总报告 |

## 配置文件

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

## 汇总报告

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
