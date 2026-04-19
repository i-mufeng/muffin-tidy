mod config;
mod scanner;
mod dedup;
mod export;
mod logger;
mod media;

use std::time::Instant;
use anyhow::Result;
use clap::Parser;
use config::{Cli, RunConfig};
use tracing::info;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = RunConfig::resolve(cli)?;

    logger::init_logger(&cfg.log);

    let start = Instant::now();
    let mut all_files = Vec::new();

    // 1. Scan
    for source in &cfg.sources {
        let mut files = scanner::scan(source, cfg.recursive, cfg.max_depth, cfg.include_hidden, &cfg.filters, &cfg.livephoto)?;
        all_files.append(&mut files);
    }

    let total_found = all_files.len();

    // 2. Compute hashes (parallel)
    if !cfg.no_dedup {
        info!("开始计算文件哈希");
        dedup::compute_hashes_parallel(&mut all_files, cfg.threads);
    }

    // 3. Export
    let stats = export::run(&all_files, &cfg.target, cfg.dry_run, cfg.no_dedup, cfg.no_conflict_check)?;

    // 4. Summary
    if cfg.summary {
        let elapsed = start.elapsed();
        let secs = elapsed.as_secs_f64();
        let elapsed_str = if secs < 60.0 {
            format!("{:.1}s", secs)
        } else {
            format!("{}m {:.0}s", secs as u64 / 60, secs % 60.0)
        };

        // helper: left label, right value, total width 40 (between the margins)
        fn row(label: &str, value: &str) -> String {
            let dots = 40usize.saturating_sub(label.len() + value.len());
            format!("  {}{}{}",
                label,
                "·".repeat(dots.max(1)),
                value)
        }
        fn divider() -> &'static str { "  ─────────────────────────────────────────" }

        println!();
        println!("  🧁 Muffin-Tidy");
        println!("{}", divider());
        for src in &cfg.sources {
            println!("{}", row("源目录", &src.display().to_string()));
        }
        println!("{}", row("目标目录", &cfg.target.display().to_string()));
        println!("{}", row("耗时", &elapsed_str));
        println!("{}", divider());
        println!("{}", row("扫描文件", &total_found.to_string()));
        println!("{}", divider());
        println!("{}", row("✅ 导出", &stats.total_exported().to_string()));
        println!("{}", row("   🖼  Img 图片", &stats.exported_img.to_string()));
        println!("{}", row("   🎬 Vdo 视频", &stats.exported_vdo.to_string()));
        println!("{}", row("   ✨ Lpo 实况", &stats.exported_lpo.to_string()));
        println!("{}", divider());
        println!("{}", row("⏭  跳过", &stats.total_skipped().to_string()));
        println!("{}", row("   ♻  内容重复", &stats.skipped_dedup.to_string()));
        println!("{}", row("   ⚠  目标冲突", &stats.skipped_conflict.to_string()));
        println!("{}", row("   ✗  写入失败", &stats.skipped_error.to_string()));
        println!("{}", divider());
        println!();
    }

    Ok(())
}
