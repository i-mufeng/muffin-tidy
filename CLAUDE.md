# CLAUDE.md — 媒体文件整理工具 `Muffin-Tidy`

## 项目概述

使用 Rust 开发一款跨平台命令行媒体文件整理工具，名为 `Muffin-Tidy`，运行命令为 mtidy 。  
工具可扫描指定源目录中的图片/视频文件，智能识别 LivePhoto（iOS 实况照片 / Android 运动照片），按日期与类型进行分类，执行哈希去重，并将文件按标准目录结构复制到目标目录。

**核心原则：绝对不对源路径的任何文件进行修改、移动或删除操作。**

---

## 目录结构

```
mediasort/
├── Cargo.toml
├── CLAUDE.md
├── README.md
├── config.example.toml          # 配置文件示例
└── src/
    ├── main.rs                  # CLI 入口，参数解析
    ├── config.rs                # 配置结构体与加载逻辑
    ├── scanner.rs               # 递归扫描源目录，收集媒体文件路径
    ├── media/
    │   ├── mod.rs               # MediaFile 统一结构体，类型枚举
    │   ├── format.rs            # 格式识别（扩展名 + Magic Bytes）
    │   ├── metadata.rs          # 时间提取（EXIF / GPS / 文件系统）
    │   └── livephoto.rs         # LivePhoto / Motion Photo 识别逻辑
    ├── dedup.rs                 # 文件哈希计算与去重集合管理
    ├── export.rs                # 目标路径生成、冲突序号递增、文件复制
    └── logger.rs                # 日志等级配置与格式化输出
```

---

## Cargo.toml 依赖

```toml
[package]
name = "mediasort"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[[bin]]
name = "mediasort"
path = "src/main.rs"

[dependencies]
# CLI 参数解析
clap = { version = "4.6.1", features = ["derive", "env"] }

# 错误处理
anyhow = "1.0.102"
thiserror = "2.0.18"

# EXIF / 元数据读取
# nom-exif 同时支持图片（JPEG/HEIC/TIFF/RAW）和视频（MOV/MP4/3GP）的元数据提取
# 替代 kamadak-exif，无需额外处理视频时间戳
nom-exif = "2.7.0"

# XMP 解析（Android Motion Photo 识别）
quick-xml = "0.39.2"

# 文件哈希（SHA-256）
sha2 = "0.11.0"
hex = "0.4.3"

# 目录递归遍历
walkdir = "2.5.0"

# 时间处理
chrono = { version = "0.4.44", features = ["serde"] }

# 序列化/反序列化（配置文件）
serde = { version = "1.0.228", features = ["derive"] }
toml = "1.1.2"

# 日志框架
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "fmt", "json"] }

# 终端颜色输出（用于自定义 Spring Boot 风格日志器）
owo-colors = { version = "4.3.0", features = ["supports-colors"] }

# 进度条
indicatif = "0.18.4"

# 并发（用于并行 hash 计算）
rayon = "1.12.0"

# 跨平台路径处理（Windows UNC 路径规范化）
dunce = "1.0.5"

# 文件时间戳设置（保留源文件 mtime/atime）
filetime = "0.2.27"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

---

## 支持的媒体格式

### 图片（Type = `Img`）

| 格式 | 扩展名 |
|------|--------|
| JPEG | `.jpg` `.jpeg` `.jpe` |
| PNG  | `.png` |
| GIF  | `.gif` |
| BMP  | `.bmp` |
| TIFF | `.tif` `.tiff` |
| HEIF/HEIC | `.heic` `.heif` |
| WebP | `.webp` |
| RAW — Canon | `.cr2` `.cr3` |
| RAW — Nikon | `.nef` `.nrw` |
| RAW — Sony  | `.arw` `.srf` `.sr2` |
| RAW — Adobe DNG | `.dng` |
| RAW — Olympus | `.orf` |
| RAW — Panasonic | `.rw2` |
| RAW — Fuji | `.raf` |
| RAW — Pentax | `.pef` `.ptx` |
| RAW — Leica | `.rwl` |
| RAW — Samsung | `.srw` |

### 视频（Type = `Vdo`）

| 格式 | 扩展名 |
|------|--------|
| MP4 | `.mp4` `.m4v` |
| QuickTime | `.mov` |
| AVI | `.avi` |
| MKV | `.mkv` |
| 3GPP | `.3gp` `.3g2` |
| MPEG | `.mpeg` `.mpg` `.mpe` |
| WebM | `.webm` |
| WMV | `.wmv` |
| FLV | `.flv` |
| MTS/AVCHD | `.mts` `.m2ts` |

### 实况照片（Type = `Lpo`）

- iOS Live Photo：一对 JPEG（或 HEIC）+ MOV，通过 EXIF `ContentIdentifier` 字段匹配
- Android Motion Photo：单个 JPEG 文件内嵌视频，通过 XMP 元数据字段 `Camera:MotionPhoto=1` 或 `GCamera:MotionPhoto=1` 识别

---

## 核心数据结构

### `MediaType` 枚举

```rust
pub enum MediaType {
    Img,  // 图片
    Vdo,  // 视频
    Lpo,  // 实况照片（LivePhoto / Motion Photo）
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
```

### `MediaFile` 结构体

```rust
pub struct MediaFile {
    pub source_path: PathBuf,        // 原始文件绝对路径
    pub media_type: MediaType,       // 媒体类型
    pub capture_time: DateTime<Local>, // 最终确定的拍摄时间
    pub time_source: TimeSource,     // 时间来源（用于日志）
    pub content_id: Option<String>,  // iOS Live Photo ContentIdentifier
    pub is_motion_photo: bool,       // Android Motion Photo 标记
    pub live_pair: Option<PathBuf>,  // 与之配对的另一个文件路径
    pub file_hash: Option<String>,   // SHA-256 哈希（延迟计算）
    pub file_size: u64,
}

pub enum TimeSource {
    ExifDateTime,
    ExifGpsDate,
    FileModified,
    FileCreated,
}
```

---

## 时间提取优先级

对每个文件，按以下顺序尝试获取时间，取第一个成功的结果：

1. **EXIF `DateTimeOriginal`**（拍摄时间，最优先）
2. **EXIF `DateTimeDigitized`**（数字化时间）
3. **EXIF GPS `GPSDateStamp` + `GPSTimeStamp`** 组合
4. **视频容器元数据** `creation_time`（MOV/MP4，由 `nom-exif` 从 moov/mvhd box 提取）
5. **文件系统修改时间**（`fs::metadata().modified()`）
6. **文件系统创建时间**（`fs::metadata().created()`，仅 Windows/macOS 可靠）

使用 `nom-exif` 统一处理图片与视频的元数据：
- 图片：`MediaParser` → `ExifIter`，读取标准 EXIF tag
- 视频：`MediaParser` → `TrackInfo`，读取容器级 `creation_time`（UTC）

需处理以下时区问题：
- 若 EXIF 时间不含时区信息，视为**本地时间**
- 若存在 `OffsetTimeOriginal` 字段，则应用对应时区偏移
- 视频容器的 `creation_time` 通常为 UTC，需转换为本地时间显示

---

## LivePhoto 识别逻辑

### iOS Live Photo

**识别方式：** 读取 JPEG/HEIC 文件的 EXIF，查找苹果专有字段 `ContentIdentifier`（Tag ID `0x9999` 位于 Apple MakerNote，或通过 XMP `apple-fi:Identifier`）。

**配对逻辑：**
1. 扫描阶段，建立 `HashMap<String, Vec<MediaFile>>`，key 为 `ContentIdentifier`
2. 同一 ContentIdentifier 下，若同时存在图片文件和 `.mov` 文件，则确认为 Live Photo 对
3. 两个文件均标记为 `MediaType::Lpo`，并互相记录 `live_pair` 路径
4. 导出时，两个文件使用**完全相同的基础文件名**（序号对齐），仅扩展名不同

**备用配对逻辑（ContentIdentifier 不可用时）：**  
同目录下文件名完全相同（不含扩展名），一个为图片格式，一个为 `.mov`，则视为 Live Photo 对。

### Android Motion Photo

**识别方式：** 读取 JPEG 文件的 XMP 元数据，检查是否包含以下任一字段：
- `Camera:MotionPhoto = "1"` 或 `= 1`
- `GCamera:MotionPhoto = "1"` 或 `= 1`
- `MicroVideo:MicroVideo = "1"`

**处理方式：**
- Android Motion Photo **保持原文件不拆分**，整体作为一个文件处理
- 类型标记为 `MediaType::Lpo`
- 导出时完整复制该 JPEG 文件（内嵌视频随之保留）

---

## 去重逻辑

使用完整文件内容的 **SHA-256 哈希**进行去重。

```rust
pub struct DedupRegistry {
    seen_hashes: HashSet<String>,
}

impl DedupRegistry {
    pub fn is_duplicate(&mut self, hash: &str) -> bool;
    pub fn register(&mut self, hash: &str);
}
```

**执行时机：**
- 在确认导出一个文件之前，计算其 SHA-256 哈希
- 若哈希已存在于 `seen_hashes`，跳过该文件，记录 `WARN` 日志
- 若不存在，则注册后执行复制

**注意：** 哈希计算为 IO 密集操作，可使用 `rayon` 并行计算，但需避免重复读取（建议在扫描阶段延迟计算，在导出队列排序后统一计算）。

---

## 导出路径生成规则

### 目录结构格式

```
{target}/{yyyy}/{MM}/{type}-{yyyyMMddHHmmss}-{nn}.{ext}
```

- `{yyyy}` — 4 位年份（基于拍摄时间）
- `{MM}` — 2 位月份，补零（01–12）
- `{type}` — `Img` / `Vdo` / `Lpo`
- `{yyyyMMddHHmmss}` — 14 位时间戳，本地时间
- `{nn}` — 2 位序号，从 `01` 开始，若目标路径已存在同名文件则递增
- `{ext}` — 原始文件扩展名（**统一转换为小写**）

**示例：**
```
D:\Photos\2024\05\Img-20240512143022-01.jpg
D:\Photos\2024\05\Lpo-20240512143022-01.heic
D:\Photos\2024\05\Lpo-20240512143022-01.mov   ← 与上方 heic 同名序号对齐
```

### 序号分配算法

```rust
fn resolve_output_path(dir: &Path, type_prefix: &str, ts: &DateTime<Local>, ext: &str) -> PathBuf {
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
```

**Live Photo 对的序号对齐：**  
处理 iOS Live Photo 时，图片和视频文件共享同一个序号（先为图片文件申请序号，视频文件使用相同序号，直接检查是否存在对应的 `.mov` 文件）。

### 路径分隔符

使用 `std::path::PathBuf`/`Path` 处理所有路径，禁止硬编码 `\` 或 `/`。  
使用 `dunce::canonicalize()` 处理 Windows UNC 路径，确保跨平台兼容。

---

## 文件复制与元数据保留

**复制方式：** 使用 `std::fs::copy(src, dst)` 进行字节级复制，该函数在所有平台上保留文件内容的完整性。

**元数据保留要求：**
- 复制完成后，使用 `filetime` crate 将目标文件的**修改时间**和**访问时间**设置为与源文件一致
- EXIF/XMP 等嵌入式元数据由于是文件内容的一部分，`fs::copy` 自动保留，无需额外处理

```toml
# 添加到 Cargo.toml
filetime = "0.2"
```

```rust
use filetime::{set_file_times, FileTime};

let meta = fs::metadata(&src)?;
let mtime = FileTime::from_last_modification_time(&meta);
let atime = FileTime::from_last_access_time(&meta);
set_file_times(&dst, atime, mtime)?;
```

---

## CLI 接口设计

### 基础用法

```bash
mediasort [OPTIONS] --source <SOURCE> --target <TARGET>
```

### 参数定义

```rust
#[derive(Parser)]
#[command(name = "mediasort", version, about = "智能媒体文件整理工具")]
pub struct Cli {
    /// 源目录路径（支持多个）
    #[arg(short, long, required = true, num_args = 1..)]
    pub source: Vec<PathBuf>,

    /// 目标目录路径
    #[arg(short, long)]
    pub target: PathBuf,

    /// 配置文件路径（TOML 格式）
    #[arg(short, long, default_value = "mediasort.toml")]
    pub config: PathBuf,

    /// 日志等级：error | warn | info | debug | trace
    #[arg(long, default_value = "info", env = "MEDIASORT_LOG")]
    pub log_level: String,

    /// 日志格式：text | json
    #[arg(long, default_value = "text")]
    pub log_format: LogFormat,

    /// 日志输出文件路径（可选，同时输出到文件和终端）
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    /// 试运行模式：只扫描和输出计划，不执行任何复制
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// 跳过去重检查
    #[arg(long, default_value_t = false)]
    pub no_dedup: bool,

    /// 递归扫描子目录（默认开启）
    #[arg(long, default_value_t = true)]
    pub recursive: bool,

    /// 最大递归深度（0 = 不限制）
    #[arg(long, default_value_t = 0)]
    pub max_depth: usize,

    /// 是否包含隐藏文件/目录（以 . 开头）
    #[arg(long, default_value_t = false)]
    pub include_hidden: bool,

    /// 并行线程数（用于哈希计算，0 = 自动）
    #[arg(long, default_value_t = 0)]
    pub threads: usize,

    /// 显示进度条
    #[arg(long, default_value_t = true)]
    pub progress: bool,

    /// 导出后输出详细的汇总报告
    #[arg(long, default_value_t = true)]
    pub summary: bool,
}
```

### 子命令（可选扩展）

```bash
mediasort scan   --source <SOURCE>             # 仅扫描，输出发现的文件列表
mediasort export --source <SOURCE> --target <TARGET>  # 扫描并导出
mediasort dedup  --source <SOURCE>             # 仅检测重复文件
```

---

## 配置文件（`mediasort.toml`）

```toml
[general]
recursive = true
max_depth = 0                   # 0 = 不限制
include_hidden = false
threads = 0                     # 0 = 自动检测 CPU 核数
dry_run = false

[logging]
level = "info"                  # error | warn | info | debug | trace
format = "text"                 # text | json
file = ""                       # 留空则不输出到文件
show_progress = true
show_summary = true

[export]
no_dedup = false
# 以下为细粒度控制，覆盖 CLI 默认行为
overwrite_existing = false      # 若目标文件已存在且非同名冲突，是否覆盖

[filters]
# 指定要包含的扩展名白名单（留空则全部支持的格式）
include_extensions = []
# 指定要排除的扩展名
exclude_extensions = []
# 文件大小过滤（字节），0 = 不限制
min_file_size = 0
max_file_size = 0

[livephoto]
enabled = true                  # 是否启用 LivePhoto 识别
match_by_content_id = true      # 优先通过 ContentIdentifier 匹配
match_by_filename = true        # 备用：通过同名文件匹配
android_motion_photo = true     # 是否识别 Android Motion Photo
```

---

## 日志规范

使用 `tracing` + 自定义 `Layer` 实现仿 Spring Boot 风格的结构化日志。  
颜色渲染使用 `owo-colors`，自动检测终端是否支持颜色（`supports-colors` feature），重定向到文件时自动降级为纯文本。

---

### Spring Boot 风格输出格式

```
{timestamp}  {LEVEL}  {icon} --- [{target}] {module} : {message}  {fields}
```

**完整示例（终端彩色输出）：**

```
2024-05-12 14:30:22.123  INFO  ✅ --- [   scanner] mediasort::scanner      : 扫描开始 source="/Volumes/Photos"
2024-05-12 14:30:22.456  INFO  ✅ --- [   scanner] mediasort::scanner      : 发现文件 path="IMG_0001.HEIC" size=3842048
2024-05-12 14:30:22.457 DEBUG  🔍 --- [  metadata] mediasort::media::metadata : EXIF 时间提取 path="IMG_0001.HEIC" time="2024-05-12 14:30:22" source=ExifDateTimeOriginal
2024-05-12 14:30:22.458 DEBUG  🔍 --- [  livephoto] mediasort::media::livephoto: LivePhoto 配对成功 image="IMG_0001.HEIC" video="IMG_0001.MOV"
2024-05-12 14:30:22.459  INFO  ✅ --- [    export] mediasort::export       : 导出文件 src="IMG_0001.HEIC" dst="2024/05/Lpo-20240512143022-01.heic"
2024-05-12 14:30:22.460  WARN  ⚠️  --- [     dedup] mediasort::dedup        : 跳过重复文件 path="IMG_0001_copy.HEIC" hash="a1b2c3..."
2024-05-12 14:30:22.461 ERROR  ❌ --- [    export] mediasort::export       : 文件写入失败 src="IMG_9999.jpg" error="Permission denied"
```

**各列颜色规范（终端输出）：**

| 列 | 颜色 |
|---|---|
| 时间戳 | 暗灰色（`bright_black`） |
| `INFO` | 亮绿色（`bright_green`）、加粗 |
| `WARN` | 亮黄色（`bright_yellow`）、加粗 |
| `ERROR` | 亮红色（`bright_red`）、加粗 |
| `DEBUG` | 亮蓝色（`bright_cyan`）、加粗 |
| `TRACE` | 暗灰色（`bright_black`） |
| `---` 分隔符 | 暗灰色 |
| `[target]` | 紫色（`magenta`） |
| 模块路径 | 青色（`cyan`） |
| 消息正文 | 默认（白色） |
| 字段键值对 | 暗灰色（`bright_black`） |

**日志级别图标：**

| 级别 | 图标 |
|---|---|
| `TRACE` | `🔬` |
| `DEBUG` | `🔍` |
| `INFO`  | `✅` |
| `WARN`  | `⚠️ ` |
| `ERROR` | `❌` |

---

### `logger.rs` 实现规范

在 `src/logger.rs` 中实现自定义 `tracing_subscriber::Layer`。

```rust
use owo_colors::{OwoColorize, Stream::Stdout};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{layer::Context, Layer};

pub struct SpringBootLayer {
    pub use_color: bool,    // 是否启用颜色（自动检测或强制关闭）
    pub log_file: Option<Arc<Mutex<File>>>,  // 可选文件输出（纯文本）
}

impl<S: Subscriber> Layer<S> for SpringBootLayer {
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) { ... }
}
```

**格式化逻辑要点：**

1. **时间戳**：`chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")`
2. **Level 字段**：固定 5 字符宽度右对齐（`{:>5}`），配合颜色和图标
3. **target 字段**：从 `event.metadata().target()` 取最后一段（模块名），固定 10 字符宽度，不足补空格，超出截断
4. **module 字段**：`event.metadata().module_path()` 取完整路径，固定 24 字符宽度
5. **字段键值对**：通过实现 `tracing::field::Visit` 收集所有字段，以 `key=value` 格式追加到消息末尾
6. **文件输出**：写入文件时剥离所有 ANSI 转义码，输出纯文本

**初始化方式：**

```rust
// src/main.rs
pub fn init_logger(config: &LogConfig) {
    let use_color = config.format == LogFormat::Text
        && std::io::IsTerminal::is_terminal(&std::io::stdout());

    let spring_layer = SpringBootLayer {
        use_color,
        log_file: config.file.as_ref().map(|p| {
            Arc::new(Mutex::new(File::create(p).expect("无法创建日志文件")))
        }),
    };

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new(&config.level))
        .with(spring_layer);

    // JSON 模式：跳过自定义层，使用标准 JSON 格式
    if config.format == LogFormat::Json {
        let subscriber = tracing_subscriber::registry()
            .with(EnvFilter::new(&config.level))
            .with(tracing_subscriber::fmt::layer().json());
        tracing::subscriber::set_global_default(subscriber).unwrap();
    } else {
        tracing::subscriber::set_global_default(subscriber).unwrap();
    }
}
```

---

### 日志等级使用规范

| 等级 | 使用场景 |
|------|----------|
| `ERROR` | 无法恢复的错误（文件读取失败、磁盘满） |
| `WARN` | 跳过的文件（重复、格式不支持、时间无法确定） |
| `INFO` | 扫描开始/结束、每个文件的导出操作、汇总统计 |
| `DEBUG` | 时间来源判断、LivePhoto 配对过程、路径解析 |
| `TRACE` | EXIF 字段读取详情、哈希计算过程 |

### JSON 格式（`--log-format json`）

```json
{"timestamp":"2024-05-12T14:30:22.123+08:00","level":"INFO","target":"scanner","message":"导出文件","src":"IMG_0001.HEIC","dst":"2024/05/Lpo-20240512143022-01.heic"}
```

---

## 执行流程

```
main()
  │
  ├─ 1. 解析 CLI 参数 + 加载配置文件（CLI 参数优先级高于配置文件）
  │
  ├─ 2. 初始化日志系统（等级、格式、输出目标）
  │
  ├─ 3. 扫描阶段 scanner::scan()
  │      ├─ walkdir 递归遍历源目录
  │      ├─ 过滤不支持格式、隐藏文件、大小限制
  │      ├─ 对每个文件：
  │      │    ├─ 读取文件元数据（大小、文件时间）
  │      │    ├─ 按优先级提取拍摄时间（EXIF > GPS > 文件时间）
  │      │    ├─ 识别 Android Motion Photo（XMP 解析）
  │      │    └─ 收集 iOS Live Photo ContentIdentifier
  │      └─ LivePhoto 配对（ContentIdentifier → HashMap 聚合）
  │
  ├─ 4. 去重阶段 dedup::compute_hashes()
  │      ├─ 使用 rayon 并行计算 SHA-256
  │      └─ 标记重复文件（保留首次出现，后续跳过）
  │
  ├─ 5. 导出计划生成 export::plan()
  │      ├─ 对每个待导出文件生成目标路径
  │      ├─ 处理同名冲突（序号递增）
  │      └─ dry_run 模式下仅打印计划，不执行复制
  │
  ├─ 6. 执行导出 export::execute()
  │      ├─ 创建目标目录（{target}/yyyy/MM/）
  │      ├─ fs::copy(src, dst)
  │      ├─ 设置目标文件时间戳与源文件一致（filetime）
  │      └─ 记录每步日志
  │
  └─ 7. 输出汇总报告
         ├─ 总文件数、成功导出数、跳过数（重复/格式/错误）
         └─ 耗时统计
```

---

## 错误处理规范

- 所有可恢复错误（单文件处理失败）：记录 `ERROR` 日志后**跳过该文件，继续处理其他文件**
- 不可恢复错误（目标磁盘无空间、源目录不存在）：返回 `Err`，退出并打印错误信息
- 使用 `anyhow::Result` 作为统一错误类型
- 使用 `thiserror` 定义领域特定错误类型（`MediaError`, `ExifError`, `ExportError`）
- 程序退出码：`0` = 成功，`1` = 有错误但部分成功，`2` = 完全失败

---

## 测试规范

在 `src/` 各模块中编写单元测试，并在 `tests/` 目录下编写集成测试。

### 必须覆盖的测试场景

- `format.rs`：各格式扩展名识别、大小写不敏感
- `metadata.rs`：EXIF 时间提取优先级、缺失 EXIF 时回退逻辑
- `livephoto.rs`：iOS ContentIdentifier 配对、Android XMP 识别、备用文件名配对
- `dedup.rs`：相同内容文件哈希一致、不同内容哈希不同
- `export.rs`：路径生成格式正确性、序号冲突递增、跨平台路径分隔符

### 测试用例说明

- 准备测试用媒体文件（可使用极小尺寸的合法格式文件）放在 `tests/fixtures/`
- `dry_run` 模式集成测试：验证不创建任何目标文件
- 源文件不变性验证：导出后对源文件 hash 再次验证与初始一致

---

## 实现注意事项

1. **统一使用 `nom-exif` 处理图片和视频**：该库同时支持 JPEG/HEIC/TIFF/RAW（EXIF）和 MOV/MP4/3GP（容器元数据），无需为视频额外引入解析库。视频的 `creation_time` 从 moov/mvhd box 提取，注意其为 UTC 时间（从 1904-01-01 起的秒数），需转换为本地时间。

2. **HEIC/HEIF 的 EXIF 读取**：`nom-exif` 对 HEIF 容器原生支持，可直接提取内嵌 EXIF，无需 libheif。

3. **RAW 格式**：大多数 RAW 格式（CR2、NEF、ARW、DNG）本质是带 EXIF 的 TIFF 结构，`nom-exif` 可直接读取，无需特殊处理。

4. **iOS ContentIdentifier 位置**：  
   - 首选：Apple MakerNote EXIF 中（需解析 Apple MakerNote 结构）  
   - 备选：文件 XMP 元数据中 `apple-fi:Identifier` 字段（使用 `quick-xml` 解析）  
   - 若两者都无法获取，退回文件名匹配策略

5. **Windows 长路径**：在 Windows 上使用 `dunce::canonicalize()` 而非标准 `fs::canonicalize()`，避免 `\\?\` 前缀导致某些 API 失败。

6. **并发安全**：`DedupRegistry` 在并行哈希阶段需使用 `Mutex<HashSet>` 或 `dashmap::DashMap` 保护。

7. **大文件哈希**：使用流式读取（`BufReader`）计算 SHA-256，避免将整个大文件载入内存。

8. **时区处理**：所有时间统一使用 `chrono::DateTime<Local>` 存储和格式化，导出路径中的时间戳使用本地时间。

9. **颜色输出兼容性**：`owo-colors` 的 `supports-colors` feature 会在非 TTY 环境（如重定向到文件、CI 环境）下自动禁用 ANSI 颜色码。也可通过检测 `NO_COLOR` 环境变量强制关闭。

---

## 构建与运行

```bash
# 开发构建
cargo build

# 发布构建（优化）
cargo build --release

# 运行（开发）
cargo run -- --source /Volumes/iPhone/DCIM --target /Volumes/Backup/Photos

# 试运行（不复制文件）
cargo run -- --source ./test_photos --target ./output --dry-run

# 指定日志等级
MEDIASORT_LOG=debug cargo run -- --source ./test_photos --target ./output

# 运行测试
cargo test

# 运行测试（含输出）
cargo test -- --nocapture
```

---

## 汇总报告格式示例

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
