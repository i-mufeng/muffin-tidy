#![allow(dead_code)]

use std::fmt::Write as FmtWrite;
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::Write as IoWrite;
use chrono::Local;
use indicatif::ProgressBar;
use owo_colors::{OwoColorize, Stream::Stdout};
use tracing::{Event, Level, Subscriber};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;
use crate::config::LogConfig;

/// Global progress bar reference. When set, log output is routed through
/// `pb.println()` so indicatif can keep the bar pinned to the last line.
static ACTIVE_PROGRESS: Mutex<Option<Arc<ProgressBar>>> = Mutex::new(None);

pub fn set_progress_bar(pb: Arc<ProgressBar>) {
    if let Ok(mut guard) = ACTIVE_PROGRESS.lock() {
        *guard = Some(pb);
    }
}

pub fn clear_progress_bar() {
    if let Ok(mut guard) = ACTIVE_PROGRESS.lock() {
        *guard = None;
    }
}

pub fn init_logger(config: &LogConfig) {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    // 抑制 nom-exif 内部对未知 tag 的 WARN 噪音（如 DNG 私有 tag 0xc6fc）
    // 使用 builder 确保 nom_exif=error 始终生效，不被环境变量覆盖
    let filter_str = format!("{},nom_exif=error", &config.level);
    let filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .parse_lossy(&filter_str);
    let use_color = std::io::IsTerminal::is_terminal(&std::io::stdout());

    let log_file = config.file.as_ref().map(|p| {
        Arc::new(Mutex::new(
            File::create(p).expect("无法创建日志文件"),
        ))
    });

    let spring_layer = SpringBootLayer { use_color, log_file };

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(spring_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global tracing subscriber");
}

pub struct SpringBootLayer {
    use_color: bool,
    log_file: Option<Arc<Mutex<File>>>,
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for SpringBootLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = *meta.level();
        let now = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");

        let target = meta.target();
        let short_target = target.split("::").last().unwrap_or(target);
        let module = meta.module_path().unwrap_or(target);

        // Truncate/pad fields
        let short_target = pad_or_truncate(short_target, 10);
        let module = pad_or_truncate(module, 24);

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let (level_str, icon) = match level {
            Level::ERROR => ("ERROR", "❌"),
            Level::WARN  => (" WARN", "⚠️ "),
            Level::INFO  => (" INFO", "✅"),
            Level::DEBUG => ("DEBUG", "🔍"),
            Level::TRACE => ("TRACE", "🔬"),
        };

        let line = if self.use_color {
            let ts = format!("{}", now);
            let ts = ts.if_supports_color(Stdout, |t| t.bright_black()).to_string();
            let lvl = match level {
                Level::ERROR => format!("{}", level_str.if_supports_color(Stdout, |t| t.bright_red())),
                Level::WARN  => format!("{}", level_str.if_supports_color(Stdout, |t| t.bright_yellow())),
                Level::INFO  => format!("{}", level_str.if_supports_color(Stdout, |t| t.bright_green())),
                Level::DEBUG => format!("{}", level_str.if_supports_color(Stdout, |t| t.bright_cyan())),
                Level::TRACE => format!("{}", level_str.if_supports_color(Stdout, |t| t.bright_black())),
            };
            let sep = "---".if_supports_color(Stdout, |t| t.bright_black()).to_string();
            let tgt_s = format!("[{:>10}]", short_target);
            let tgt = tgt_s.if_supports_color(Stdout, |t| t.magenta()).to_string();
            let mdl_s = format!("{:<24}", module);
            let mdl = mdl_s.if_supports_color(Stdout, |t| t.cyan()).to_string();
            let flds = visitor.fields.if_supports_color(Stdout, |t| t.bright_black()).to_string();
            format!("{}  {}  {} {} {} {} : {}  {}", ts, lvl, icon, sep, tgt, mdl, visitor.message, flds)
        } else {
            format!(
                "{}  {}  {} --- [{:>10}] {:<24} : {}  {}",
                now, level_str, icon, short_target, module, visitor.message, visitor.fields
            )
        };

        // Route through the active progress bar (if any) so indicatif keeps
        // the bar pinned to the last line; otherwise fall back to println!.
        let pb_guard = ACTIVE_PROGRESS.lock().ok();
        match pb_guard.as_ref().and_then(|g| g.as_ref()) {
            Some(pb) => pb.println(&line),
            None => println!("{}", line),
        }

        // Also write plain text to log file
        if let Some(file) = &self.log_file {
            let plain = format!(
                "{}  {}  {} --- [{:>10}] {:<24} : {}  {}\n",
                now, level_str, icon, short_target, module, visitor.message, visitor.fields
            );
            if let Ok(mut f) = file.lock() {
                let _ = f.write_all(plain.as_bytes());
            }
        }
    }
}

fn pad_or_truncate(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[s.len() - width..].to_string()
    } else {
        format!("{:width$}", s, width = width)
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: String,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value).trim_matches('"').to_string();
        } else {
            if !self.fields.is_empty() { self.fields.push(' '); }
            let _ = write!(self.fields, "{}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            if !self.fields.is_empty() { self.fields.push(' '); }
            let _ = write!(self.fields, "{}=\"{}\"", field.name(), value);
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if !self.fields.is_empty() { self.fields.push(' '); }
        let _ = write!(self.fields, "{}={}", field.name(), value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if !self.fields.is_empty() { self.fields.push(' '); }
        let _ = write!(self.fields, "{}={}", field.name(), value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if !self.fields.is_empty() { self.fields.push(' '); }
        let _ = write!(self.fields, "{}={}", field.name(), value);
    }
}
