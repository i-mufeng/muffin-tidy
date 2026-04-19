#![allow(dead_code)]

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use clap::Parser;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Parser, Debug)]
#[command(
    name = "mtidy",
    version,
    about = "智能媒体文件整理工具",
    long_about = "智能媒体文件整理工具\n\n用法示例:\n  mtidy <目标目录>              # 整理当前目录到目标目录\n  mtidy <源目录> <目标目录>      # 指定源目录和目标目录\n  mtidy -s <源> -t <目标>       # 使用 flag 方式"
)]
pub struct Cli {
    /// 位置参数：[源目录] 目标目录
    /// 若只传一个参数，视为目标目录，源目录默认为当前工作目录
    #[arg(value_name = "PATHS", num_args = 1..=2)]
    pub positional: Vec<PathBuf>,

    /// 源目录路径（支持多个，与位置参数互斥）
    #[arg(short, long, num_args = 1..)]
    pub source: Vec<PathBuf>,

    /// 目标目录路径（与位置参数互斥）
    #[arg(short, long)]
    pub target: Option<PathBuf>,

    /// 配置文件路径（TOML 格式）
    #[arg(short, long, default_value = "mtidy.toml")]
    pub config: PathBuf,

    /// 日志等级
    #[arg(long, default_value = "info", env = "MEDIASORT_LOG")]
    pub log_level: String,

    /// 日志格式：text | json
    #[arg(long, default_value = "text")]
    pub log_format: LogFormat,

    /// 日志输出文件路径
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    /// 试运行模式
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// 跳过去重检查
    #[arg(long, default_value_t = false)]
    pub no_dedup: bool,

    /// 跳过目标冲突检测（目标存在同名文件时直接加序号，不比较内容）
    #[arg(long, default_value_t = false)]
    pub no_conflict_check: bool,

    /// 递归扫描子目录
    #[arg(long, default_value_t = true)]
    pub recursive: bool,

    /// 最大递归深度（0 = 不限制）
    #[arg(long, default_value_t = 0)]
    pub max_depth: usize,

    /// 是否包含隐藏文件/目录
    #[arg(long, default_value_t = false)]
    pub include_hidden: bool,

    /// 并行线程数（0 = 自动）
    #[arg(long, default_value_t = 0)]
    pub threads: usize,

    /// 显示进度条
    #[arg(long, default_value_t = true)]
    pub progress: bool,

    /// 导出后输出汇总报告
    #[arg(long, default_value_t = true)]
    pub summary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String,
    pub format: LogFormat,
    pub file: Option<PathBuf>,
    pub show_progress: bool,
    pub show_summary: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: LogFormat::Text,
            file: None,
            show_progress: true,
            show_summary: true,
        }
    }
}

// ── TOML 配置文件结构 ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub export: ExportConfig,
    #[serde(default)]
    pub filters: FiltersConfig,
    #[serde(default)]
    pub livephoto: LivePhotoConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub recursive: bool,
    pub max_depth: usize,
    pub include_hidden: bool,
    pub threads: usize,
    pub dry_run: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { recursive: true, max_depth: 0, include_hidden: false, threads: 0, dry_run: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
    pub file: Option<PathBuf>,
    pub show_progress: bool,
    pub show_summary: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: LogFormat::Text,
            file: None,
            show_progress: true,
            show_summary: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    pub no_dedup: bool,
    pub no_conflict_check: bool,
    pub overwrite_existing: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self { no_dedup: false, no_conflict_check: false, overwrite_existing: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FiltersConfig {
    pub include_extensions: Vec<String>,
    pub exclude_extensions: Vec<String>,
    pub min_file_size: u64,
    pub max_file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePhotoConfig {
    pub enabled: bool,
    pub match_by_content_id: bool,
    pub match_by_filename: bool,
    pub android_motion_photo: bool,
}

impl Default for LivePhotoConfig {
    fn default() -> Self {
        Self { enabled: true, match_by_content_id: true, match_by_filename: true, android_motion_photo: true }
    }
}

impl FileConfig {
    /// 从 TOML 文件加载配置，文件不存在时返回默认值
    pub fn load(path: &PathBuf) -> Self {
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("警告：配置文件解析失败（{}），使用默认配置", e);
                Self::default()
            }),
            Err(e) => {
                eprintln!("警告：无法读取配置文件（{}），使用默认配置", e);
                Self::default()
            }
        }
    }
}

/// 解析后的最终运行参数（CLI 覆盖配置文件）
pub struct RunConfig {
    pub sources: Vec<PathBuf>,
    pub target: PathBuf,
    pub log: LogConfig,
    pub dry_run: bool,
    pub no_dedup: bool,
    pub no_conflict_check: bool,
    pub recursive: bool,
    pub max_depth: usize,
    pub include_hidden: bool,
    pub threads: usize,
    pub summary: bool,
    pub filters: FiltersConfig,
    pub livephoto: LivePhotoConfig,
}

impl RunConfig {
    /// 将 CLI 参数与配置文件合并，CLI 优先
    pub fn resolve(cli: Cli) -> anyhow::Result<Self> {
        // 解析位置参数
        let (sources, target) = match (cli.positional.len(), cli.source.is_empty(), cli.target.as_ref()) {
            // 位置参数：1 个 → target，source = cwd
            (1, true, None) => {
                let target = cli.positional[0].clone();
                let cwd = std::env::current_dir()?;
                (vec![cwd], target)
            }
            // 位置参数：2 个 → source target
            (2, true, None) => {
                let source = cli.positional[0].clone();
                let target = cli.positional[1].clone();
                (vec![source], target)
            }
            // flag 方式
            (0, false, Some(t)) => (cli.source, t.clone()),
            // 混合：有 -s 但没有 -t，且有 1 个位置参数作为 target
            (1, false, None) => (cli.source, cli.positional[0].clone()),
            _ => {
                anyhow::bail!(
                    "请指定目标目录。用法：\n  mtidy <目标目录>\n  mtidy <源目录> <目标目录>\n  mtidy -s <源> -t <目标>"
                );
            }
        };

        let file_cfg = FileConfig::load(&cli.config);

        Ok(RunConfig {
            sources,
            target,
            log: LogConfig {
                level: cli.log_level,
                format: cli.log_format,
                file: cli.log_file,
                show_progress: cli.progress,
                show_summary: cli.summary,
            },
            dry_run: if cli.dry_run { true } else { file_cfg.general.dry_run },
            no_dedup: if cli.no_dedup { true } else { file_cfg.export.no_dedup },
            no_conflict_check: if cli.no_conflict_check { true } else { file_cfg.export.no_conflict_check },
            recursive: cli.recursive,
            max_depth: cli.max_depth,
            include_hidden: cli.include_hidden,
            threads: if cli.threads > 0 { cli.threads } else { file_cfg.general.threads },
            summary: cli.summary,
            filters: file_cfg.filters,
            livephoto: file_cfg.livephoto,
        })
    }
}
