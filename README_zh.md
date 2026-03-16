# zenmux-adapter

[![构建状态](https://github.com/aitiotekt/zenmux-adapter/actions/workflows/release.yml/badge.svg)](https://github.com/aitiotekt/zenmux-adapter/actions/workflows/release.yml)
[![最新版本](https://img.shields.io/github/v/release/aitiotekt/zenmux-adapter)](https://github.com/aitiotekt/zenmux-adapter/releases/latest)

`zenmux-adapter` 是一个 Rust CLI，用来从 ZenMux 模型目录生成 AI 工具的配置文件，目前支持:

- OpenClaw

它会从以下接口拉取最新模型列表：

- `https://zenmux.ai/api/v1/models`

然后把选中的模型转换成与目标 AI 工具的配置文件。

> For English documentation, see [README.md](./README.md)

---

## 快速安装

**Linux / macOS**

```sh
curl --proto '=https' --tlsv1.2 -sSf \
  https://raw.githubusercontent.com/aitiotekt/zenmux-adapter/main/scripts/install.sh | sh
```

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/aitiotekt/zenmux-adapter/main/scripts/install.ps1 | iex
```

以上脚本会自动检测您的平台和架构，从 GitHub 下载最新版本，并安装到 `~/.local/bin/zenmux-adapter`（在 Windows 上为 `%USERPROFILE%\.local\bin\zenmux-adapter.exe`）。如果文件已存在，则会覆盖。

---

## 下载（预构建二进制）

各平台的预构建二进制文件已发布在
[Releases](https://github.com/aitiotekt/zenmux-adapter/releases) 页面。

| 平台 | 文件名 |
|---|---|
| Linux x86_64 | `zenmux-adapter-<version>-x86_64-unknown-linux-musl.tar.gz` |
| Linux aarch64 | `zenmux-adapter-<version>-aarch64-unknown-linux-musl.tar.gz` |
| macOS Intel | `zenmux-adapter-<version>-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `zenmux-adapter-<version>-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `zenmux-adapter-<version>-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `zenmux-adapter-<version>-aarch64-pc-windows-msvc.zip` |

下载解压后，将 `zenmux-adapter`（或 `zenmux-adapter.exe`）放入 `PATH` 即可使用。

---

## 快速上手（OpenClaw）

### 第一步 – 生成配置

交互式选择模型，同时可选择输入 API key：

```bash
zenmux-adapter openclaw generate --interactive
```

交互流程：
1. 在可搜索的 TUI 列表中选择模型。
2. 可选择输入 ZenMux API key（直接回车则保留占位符，安装时再设置）。

生成全量模型配置并直接指定 key：

```bash
zenmux-adapter openclaw generate --all-models --api-key sk-ss-v1-xxxx
```

### 第二步 – 安装（合并到 OpenClaw 配置）

将 ZenMux provider 和模型条目合并到已有的 OpenClaw 配置文件中。
如果目标文件不存在则自动创建。可选设置 API key 和主模型
（原 primary 将被移入 fallback 列表首位）：

```bash
# 交互模式 – 提示输入 key 及选择主模型
# 默认目标 ~/.openclaw/openclaw.json
zenmux-adapter openclaw install zenmux-openclaw-interative.json

# 非交互模式 – 通过命令行指定 key、primary 和自定义目标路径
zenmux-adapter openclaw install zenmux-openclaw-interative.json \
  --dest ~/.config/openclaw/config.json \
  --api-key sk-ss-v1-xxxx \
  --primary zenmux/openai/gpt-5.4 \
  --non-interactive
```

### 第三步 – 卸载（移除 ZenMux 条目）

从配置文件中移除 ZenMux provider 和模型条目。如果当前主模型来自 ZenMux，
CLI 会从 fallback 列表或剩余可用模型中恢复 primary：

```bash
# 交互模式 – 提示选择替代主模型（默认目标）
zenmux-adapter openclaw uninstall

# 非交互模式 – 自动选择 fallback 中第一个或剩余 models 的第一个
zenmux-adapter openclaw uninstall --dest ~/.config/openclaw/config.json --non-interactive
```

---

## 构建

```bash
cargo build --release
```

产物位于 `target/release/zenmux-adapter`。

## 开发者用法

顶层帮助：

```bash
cargo run -- --help
```

OpenClaw 子命令帮助：

```bash
cargo run -- openclaw generate --help
cargo run -- openclaw install --help
cargo run -- openclaw uninstall --help
```

当前命令结构：

```
zenmux-adapter openclaw generate   [OPTIONS] <--all-models|--interactive|--models <MODEL_ID>...>
zenmux-adapter openclaw install    [OPTIONS] <CONFIG_FILE> --dest <DEST>
zenmux-adapter openclaw uninstall  [OPTIONS] <DEST>
```

### 生成全量模型配置

```bash
cargo run -- openclaw generate --all-models
```

默认输出文件：`zenmux-openclaw-all.json`

自定义输出文件：

```bash
cargo run -- openclaw generate --all-models --output custom-openclaw.json
```

### 交互式选择模型

```bash
cargo run -- openclaw generate --interactive
```

交互模式支持同界面 TUI 搜索过滤。停留在同一个多选界面里直接输入即可按模型 ID、显示名称或输入模态过滤候选项，按空格勾选，按回车确认。选完模型后会提示可选择输入 API key（直接回车则保留占位符）。

默认输出文件：`zenmux-openclaw-interative.json`

### 按模型 ID 指定生成

逗号分隔：

```bash
cargo run -- openclaw generate --models openai/gpt-5.4,anthropic/claude-sonnet-4.6
```

重复传参：

```bash
cargo run -- openclaw generate \
  --models openai/gpt-5.4 \
  --models anthropic/claude-sonnet-4.6
```

## 参数说明 – generate

| 参数 | 默认值 | 说明 |
|---|---|---|
| `--all-models` | – | 包含 ZenMux 返回的全部模型 |
| `--interactive` | – | 通过交互式列表选择模型 |
| `--models` | – | 只生成指定模型 ID |
| `--output` / `-o` | 自动 | 输出到自定义文件路径 |
| `--models-api` | ZenMux 接口 | 覆盖模型列表接口地址 |
| `--base-url` | ZenMux API base | 覆盖生成结果中的 OpenClaw provider `baseUrl` |
| `--api-key` | 占位符 | 设置生成结果中的 OpenClaw provider `apiKey` |
| `--max-tokens` | `8192` | 设置生成结果中的 `maxTokens` |

## 参数说明 – install

| 参数 | 说明 |
|---|---|
| `<CONFIG_FILE>` | 生成的 ZenMux OpenClaw JSON 配置文件路径 |
| `--dest` / `-d` | 目标 OpenClaw 配置文件（默认：`~/.openclaw/openclaw.json`） |
| `--api-key` | 要注入 ZenMux provider 的 API key（替换占位符） |
| `--primary` | 设置为主模型的 key；原 primary 移入 fallback |
| `--non-interactive` | key 缺失时直接报错而不提示 |

## 参数说明 – uninstall

| 参数 | 说明 |
|---|---|
| `--dest` / `-d` | 目标 OpenClaw 配置文件（默认：`~/.openclaw/openclaw.json`） |
| `--non-interactive` | 自动从 fallback/models 列表中选择替代 primary |

## 说明

- `generate` 命令中 `--all-models`、`--interactive`、`--models` 三者必须且只能选一个。
- CLI 会按 ZenMux 模型的 `created` 字段倒序排序，以对齐 "newest" 的语义。
- ZenMux 的价格字段可能有多档定价，这个工具会折叠成 OpenClaw `cost` 所需的单值。
- 生成出的 provider 名固定为 `zenmux`。
