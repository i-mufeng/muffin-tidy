# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Muffin-Tidy (`mtidy`) 是一个 Rust 跨平台 CLI 媒体文件整理工具。扫描源目录中的图片/视频，智能识别 LivePhoto，按日期分类，SHA-256 去重，复制到目标目录。

**核心原则：绝对不修改、移动或删除源路径的任何文件。**

## 常用命令

```bash
# 构建
cargo build                          # 开发构建
cargo build --release                # 发布构建（LTO + strip）

# 测试
cargo test                           # 运行所有测试
cargo test -- --nocapture            # 带标准输出
cargo test format_test               # 运行单个模块测试

# 运行
cargo run -- <目标目录>               # 最简用法（源=当前目录）
cargo run -- <源目录> <目标目录>       # 指定源和目标
cargo run -- -s <源> -t <目标>        # flag 方式
cargo run -- <源> <目标> --dry-run    # 试运行
MTIDY_LOG=debug cargo run -- <源> <目标>   # 调试日志

# 集成测试需要先构建
cargo build && cargo test
```

## 架构

数据流：`main → config → scanner → dedup → export`

```
src/
├── main.rs          CLI 入口，流程编排，汇总报告输出
├── config.rs        CLI 参数(clap derive) + TOML 配置合并，RunConfig 为最终运行参数
├── scanner.rs       walkdir 递归扫描 → 格式过滤 → 时间提取 → LivePhoto 配对
├── media/
│   ├── mod.rs       MediaFile/MediaType/TimeSource 核心类型定义
│   ├── format.rs    扩展名匹配识别（IMG_EXTS/VDO_EXTS 常量表）
│   ├── metadata.rs  nom-exif 提取拍摄时间（EXIF > GPS > 文件时间）
│   └── livephoto.rs XMP 解析：Android Motion Photo 识别 + iOS ContentIdentifier 读取
├── dedup.rs         rayon 并行 SHA-256 哈希计算 + DedupRegistry 去重
├── export.rs        目标路径生成、冲突检测、fs::copy + filetime 保留时间戳
└── logger.rs        自定义 tracing Layer，Spring Boot 风格终端彩色日志
```

### 关键设计决策

- **CLI 用法**：支持位置参数（`mtidy <target>` 或 `mtidy <source> <target>`）和 flag 方式（`-s`/`-t`），flag 优先
- **配置文件**：默认 `mtidy.toml`，CLI 参数优先级高于配置文件
- **时间提取**：`nom-exif` 统一处理图片 EXIF 和视频容器元数据，EXIF 时间视为本地时间，视频 `creation_time` 为 UTC 需转换
- **LivePhoto 配对**：两策略 — ① ContentIdentifier（EXIF tag 0x9999 + XMP apple-fi:Identifier）② 文件名回退（同目录同名 图片+.mov）
- **去重**：导出前计算 SHA-256，哈希线程数硬上限 4（I/O bound 限制）
- **目标路径格式**：`{target}/{yyyy}/{MM}/{Type}-{yyyyMMddHHmmss}-{nn}.{ext}`，扩展名强制小写
- **Live Photo 序号对齐**：配对的图片和视频共享同一序号
- **路径处理**：使用 `dunce::canonicalize()` 避免 Windows UNC 路径问题
- **进度条**：日志通过 `pb.println()` 输出以配合 indicatif 进度条
- **nom_exif 噪音抑制**：logger filter 中硬编码 `nom_exif=error` 抑制未知 tag 的 WARN

### 错误处理

- 单文件错误 → `ERROR` 日志后跳过，继续处理
- 不可恢复错误 → `anyhow::bail!` 退出
- 退出码：`0` 成功，`1` 部分成功，`2` 完全失败

### 依赖要点

| 依赖 | 用途 |
|------|------|
| `nom-exif` | 图片 EXIF + 视频容器元数据统一读取 |
| `quick-xml` | 目前仅用于 XMP 解析（Motion Photo 识别） |
| `rayon` | 并行哈希计算 |
| `filetime` | 复制后保留源文件 mtime/atime |
| `dunce` | Windows 路径规范化 |
| `owo-colors` | 终端彩色输出（supports-colors feature） |
| `indicatif` | 进度条 |

### 测试

- 单元测试：各模块内 `#[cfg(test)]`，`format_test.rs` 作为独立测试模块
- 集成测试：`tests/integration_test.rs` 通过运行编译好的 `mtidy` 二进制验证端到端行为
- 测试覆盖：格式识别、路径生成、序号递增、去重、干运行、源文件不变性
- CI：GitHub Actions 在 tag `v*` 时自动构建 7 个平台（linux gnu/musl/arm64、macos x86/arm64、windows x86/arm64）

### 日志格式

```
{timestamp}  {LEVEL}  {icon} --- [{target}] {module} : {message}  {key=value}
```

使用 `tracing` 自定义 Layer 实现，颜色通过 `owo-colors` 渲染。日志级别图标：`✅ INFO` `⚠️ WARN` `❌ ERROR` `🔍 DEBUG` `🔬 TRACE`
