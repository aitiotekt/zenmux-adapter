# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.0]: https://github.com/aitiotekt/zenmux-adapter/releases/tag/v0.1.0
