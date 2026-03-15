# zenmux-adapter

[![Build](https://github.com/aitiotekt/zenmux-adapter/actions/workflows/release.yml/badge.svg)](https://github.com/aitiotekt/zenmux-adapter/actions/workflows/release.yml)
[![Release](https://img.shields.io/github/v/release/aitiotekt/zenmux-adapter)](https://github.com/aitiotekt/zenmux-adapter/releases/latest)

`zenmux-adapter` is a Rust CLI for generating AI tool configuration files from the ZenMux model catalog, now support:

- OpenClaw

It fetches the live model list from:

- `https://zenmux.ai/api/v1/models`

Then it converts the selected models into the target AI tool configuration format.

> 中文文档请见 [README_zh.md](./README_zh.md)

---

## Download (Pre-built Binaries)

Pre-built binaries for all major platforms are published on the
[Releases](https://github.com/aitiotekt/zenmux-adapter/releases) page.

| Platform | File |
|---|---|
| Linux x86_64 | `zenmux-adapter-<version>-x86_64-unknown-linux-musl.tar.gz` |
| Linux aarch64 | `zenmux-adapter-<version>-aarch64-unknown-linux-musl.tar.gz` |
| macOS Intel | `zenmux-adapter-<version>-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `zenmux-adapter-<version>-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `zenmux-adapter-<version>-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `zenmux-adapter-<version>-aarch64-pc-windows-msvc.zip` |

Download, extract, and place `zenmux-adapter` (or `zenmux-adapter.exe`) somewhere on your `PATH`.

---

## Quick Start (OpenClaw)

### 1 – Generate a config

Pick models interactively and set your API key in one step:

```bash
zenmux-adapter openclaw generate --interactive
```

You will be prompted to:
1. Select models from a searchable TUI list.
2. Optionally enter your ZenMux API key (press Enter to keep the placeholder and set it at install time).

Generate with all models and supply the key upfront:

```bash
zenmux-adapter openclaw generate --all-models --api-key sk-ss-v1-xxxx
```

### 2 – Install (merge into OpenClaw config)

Merge the ZenMux provider and model entries into your existing OpenClaw config.
If the file does not exist it will be created. You can optionally set the API key
and choose a primary model (the current primary is moved to fallback):

```bash
# Interactive – prompts for key and primary model selection
# Defaults to ~/.openclaw/openclaw.json
zenmux-adapter openclaw install zenmux-openclaw-interative.json

# Non-interactive – supply key, primary, and custom dest
zenmux-adapter openclaw install zenmux-openclaw-interative.json \
  --dest ~/.config/openclaw/config.json \
  --api-key sk-ss-v1-xxxx \
  --primary zenmux/openai/gpt-5.4 \
  --non-interactive
```

### 3 – Uninstall (remove ZenMux entries)

Remove ZenMux provider and model entries from the config. If the primary model
was provided by ZenMux the CLI will restore it from the fallback list or prompt
you to pick a replacement:

```bash
# Interactive – prompts for primary replacement (default dest)
zenmux-adapter openclaw uninstall

# Non-interactive – auto-picks first fallback or first remaining model
zenmux-adapter openclaw uninstall --dest ~/.config/openclaw/config.json --non-interactive
```

---

## Build

```bash
cargo build --release
```

The resulting binary is at `target/release/zenmux-adapter`.

## Developer Usage

Top-level help:

```bash
cargo run -- --help
```

OpenClaw subcommand help:

```bash
cargo run -- openclaw generate --help
cargo run -- openclaw install --help
cargo run -- openclaw uninstall --help
```

Current command shape:

```
zenmux-adapter openclaw generate   [OPTIONS] <--all-models|--interactive|--models <MODEL_ID>...>
zenmux-adapter openclaw install    [OPTIONS] <CONFIG_FILE> --dest <DEST>
zenmux-adapter openclaw uninstall  [OPTIONS] <DEST>
```

### Generate With All Models

```bash
cargo run -- openclaw generate --all-models
```

Default output: `zenmux-openclaw-all.json`

Custom output:

```bash
cargo run -- openclaw generate --all-models --output custom-openclaw.json
```

### Generate With Interactive Selection

```bash
cargo run -- openclaw generate --interactive
```

Interactive mode supports in-place TUI filtering. Type to filter candidates by
model ID, display name, or input modality; use space to toggle models; press
Enter to confirm. After model selection you will be prompted for an optional
API key (press Enter to keep the placeholder).

Default output: `zenmux-openclaw-interative.json`

### Generate With Explicit Model IDs

Comma-separated:

```bash
cargo run -- openclaw generate --models openai/gpt-5.4,anthropic/claude-sonnet-4.6
```

Repeated flag:

```bash
cargo run -- openclaw generate \
  --models openai/gpt-5.4 \
  --models anthropic/claude-sonnet-4.6
```

## Options – generate

| Flag | Default | Description |
|---|---|---|
| `--all-models` | – | Include all models returned by ZenMux |
| `--interactive` | – | Choose models in an interactive selector |
| `--models` | – | Include only the specified model IDs |
| `--output` / `-o` | auto | Write to a custom file path |
| `--models-api` | ZenMux endpoint | Override the model list API endpoint |
| `--base-url` | ZenMux API base | Override the OpenClaw provider `baseUrl` |
| `--api-key` | placeholder | Set the OpenClaw provider `apiKey` |
| `--max-tokens` | `8192` | Set `maxTokens` for generated models |

## Options – install

| Flag | Description |
|---|---|
| `<CONFIG_FILE>` | Path to the generated ZenMux OpenClaw JSON file |
| `--dest` / `-d` | Target OpenClaw config file (default: `~/.openclaw/openclaw.json`) |
| `--api-key` | API key to inject into the ZenMux provider (replaces placeholder) |
| `--primary` | Model key to set as primary; old primary moves to fallback |
| `--non-interactive` | Fail instead of prompting when key is missing |

## Options – uninstall

| Flag | Description |
|---|---|
| `--dest` / `-d` | Target OpenClaw config file (default: `~/.openclaw/openclaw.json`) |
| `--non-interactive` | Auto-pick replacement primary from fallback/models list |

## Notes

- One of `--all-models`, `--interactive`, or `--models` is required for `generate`.
- The CLI sorts ZenMux models by `created` descending to match the "newest" ordering.
- ZenMux pricing can contain multiple tiers. This tool collapses those tiers into single representative values for the OpenClaw `cost` fields.
- The generated provider name is `zenmux`.
