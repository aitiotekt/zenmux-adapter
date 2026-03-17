# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1]

### Added

- One-tap install scripts (`scripts/install.sh` for Linux/macOS, `scripts/install.ps1` for Windows) – automatically detect platform and architecture, download the latest release from GitHub, and install to `~/.local/bin/zenmux-adapter` (or `%USERPROFILE%\.local\bin\zenmux-adapter.exe`).
- `filter_openclaw_input_modalities` – OpenClaw only supports `"text"` and `"image"` input types; other modalities (e.g. `"audio"`, `"video"`) are now silently filtered out when generating configs. Falls back to `["text"]` if nothing supported remains.
- Unit tests for the new modality filter (`test_filter_keeps_text_and_image`, `test_filter_removes_unknown_modalities`, `test_filter_all_unsupported_falls_back_to_text`, `test_filter_empty_falls_back_to_text`) and end-to-end integration tests via `build_openclaw_config`.

### Fixed

- CI: replaced deprecated `macos-13` (Intel) runner with `macos-latest` (Apple Silicon) for the `x86_64-apple-darwin` target – cross-compilation from ARM64 to x86_64 is natively supported by the Rust toolchain.
- CI: switched `x86_64-unknown-linux-musl` target to use `cross` (same as `aarch64-unknown-linux-musl`) to avoid linker misconfiguration with bare `cargo` + `musl-tools`.
- CI: added `ilammy/msvc-dev-cmd@v1` with `arch: amd64_arm64` for the `aarch64-pc-windows-msvc` target to activate the MSVC cross-compilation toolchain on `windows-latest` runners.
- CI: added a dedicated `test` job (runs `cargo test --all-features` on `ubuntu-latest`) that gates all platform builds, preventing wasted runner time on failing code.

## [0.1.0]

### Added

- **OpenClaw adapter** – generate, install, and uninstall ZenMux model configurations for [OpenClaw](https://openclaw.ai).
- `openclaw generate` command – fetch the live ZenMux model list from `https://zenmux.ai/api/v1/models` and convert selected models into an OpenClaw JSON config.
  - `--all-models` flag to include every model returned by the API.
  - `--interactive` flag to pick models via an in-place TUI selector (supports filtering by model ID, display name, or input modality).
  - `--models` flag to supply explicit model IDs (comma-separated or repeated).
  - `--output` / `-o` option for a custom output file path.
  - `--api-key` option to set the ZenMux provider API key at generation time.
  - `--base-url` option to override the OpenClaw provider `baseUrl`.
  - `--max-tokens` option (default `8192`) to control `maxTokens` for generated model entries.
  - `--models-api` option to override the model list endpoint.
- `openclaw install` command – merge a generated ZenMux config into an existing OpenClaw config file (creates the file if absent).
  - `--dest` / `-d` option (default: `~/.openclaw/openclaw.json`).
  - `--api-key` option to inject/replace the API key placeholder.
  - `--primary` option to promote a model to primary (old primary moves to fallback).
  - `--non-interactive` flag to fail rather than prompt when required values are missing.
- `openclaw uninstall` command – remove ZenMux provider and model entries from an OpenClaw config; restores a non-ZenMux primary model automatically.
  - `--dest` / `-d` option.
  - `--non-interactive` flag to auto-pick a replacement primary from the fallback/model list.
- Core logic extracted into `lib.rs` with an [OpenDAL](https://opendal.apache.org/) `BlockingOperator` abstraction, enabling `fs` backend in production and `memory` backend in unit tests.
- Comprehensive unit tests covering `generate`, `install`, and `uninstall` logic using the in-memory backend.
- Cross-platform release workflow – pre-built binaries for Linux (x86_64 / aarch64 musl), macOS (Intel / Apple Silicon), and Windows (x86_64 / ARM64) published via GitHub Actions.

[Unreleased]: https://github.com/aitiotekt/zenmux-adapter/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/aitiotekt/zenmux-adapter/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/aitiotekt/zenmux-adapter/releases/tag/v0.1.0
