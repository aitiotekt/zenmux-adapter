use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use inquire::{MultiSelect, Password, Select, Text};
use reqwest::blocking::Client;

use zenmux_adapter::*;

// ── CLI structure ──────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(author, version, about = "ZenMux adapter CLI.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Openclaw(OpenClawCommand),
}

#[derive(Args, Debug)]
struct OpenClawCommand {
    #[command(subcommand)]
    command: OpenClawSubcommand,
}

#[derive(Subcommand, Debug)]
enum OpenClawSubcommand {
    Generate(GenerateOpenClawArgs),
    Install(InstallOpenClawArgs),
    Uninstall(UninstallOpenClawArgs),
}

// ── Subcommand: generate ───────────────────────────────────────────────────────

#[derive(Args, Debug)]
#[command(
    about = "Generate OpenClaw model config from the ZenMux catalog.",
    group = clap::ArgGroup::new("selection")
        .required(true)
        .args(["all_models", "interactive", "models"])
)]
struct GenerateOpenClawArgs {
    #[arg(long, help = "Generate config with all models.")]
    all_models: bool,

    #[arg(long, help = "Select models interactively.")]
    interactive: bool,

    #[arg(
        long,
        value_name = "MODEL_ID",
        num_args = 1..,
        value_delimiter = ',',
        help = "Generate config for specific model IDs. Accepts repeated values or comma-separated values."
    )]
    models: Vec<String>,

    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    #[arg(long, default_value = MODELS_API_URL)]
    models_api: String,

    #[arg(long, default_value = PROVIDER_BASE_URL)]
    base_url: String,

    /// API key written into the generated config.
    /// In --interactive mode this flag is optional; if omitted the CLI will
    /// prompt for it (pressing Enter keeps the placeholder).
    #[arg(long)]
    api_key: Option<String>,

    #[arg(long, default_value_t = DEFAULT_MAX_TOKENS)]
    max_tokens: u64,
}

// ── Subcommand: install ────────────────────────────────────────────────────────

#[derive(Args, Debug)]
#[command(about = "Merge a generated ZenMux config into an existing OpenClaw config file.")]
struct InstallOpenClawArgs {
    /// Path to the generated ZenMux OpenClaw JSON config file.
    #[arg(value_name = "CONFIG_FILE")]
    config_file: PathBuf,

    /// Path to the target OpenClaw config file.
    /// Defaults to ~/.openclaw/openclaw.json.
    /// The file will be created if it does not yet exist.
    #[arg(short, long, value_name = "DEST")]
    dest: Option<PathBuf>,

    /// API key to inject into the ZenMux provider.
    /// Required when the source config still contains the default placeholder.
    /// In non-interactive mode this flag is mandatory if the key is missing.
    /// In interactive mode the CLI will prompt if this flag is omitted.
    #[arg(long, value_name = "KEY")]
    api_key: Option<String>,

    /// Model key to set as the new primary model.
    /// The current primary is moved to the front of the fallback list.
    /// In interactive mode a single-select prompt is shown if this is omitted.
    #[arg(long, value_name = "MODEL_KEY")]
    primary: Option<String>,

    /// Run non-interactively. Fails immediately if --api-key is required but
    /// not supplied on the command line.
    #[arg(long)]
    non_interactive: bool,
}

// ── Subcommand: uninstall ──────────────────────────────────────────────────────

#[derive(Args, Debug)]
#[command(about = "Remove ZenMux entries from an OpenClaw config and restore the primary model.")]
struct UninstallOpenClawArgs {
    /// Path to the OpenClaw config file.
    /// Defaults to ~/.openclaw/openclaw.json.
    #[arg(short, long, value_name = "DEST")]
    dest: Option<PathBuf>,

    /// Run non-interactively. Uses the first fallback or first remaining
    /// model key when restoring the primary. Clears primary if nothing
    /// is available.
    #[arg(long)]
    non_interactive: bool,
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Openclaw(openclaw) => match openclaw.command {
            OpenClawSubcommand::Generate(args) => run_openclaw_generate(args),
            OpenClawSubcommand::Install(args) => run_openclaw_install(args),
            OpenClawSubcommand::Uninstall(args) => run_openclaw_uninstall(args),
        },
    }
}

// ── Command: generate ─────────────────────────────────────────────────────────

fn run_openclaw_generate(args: GenerateOpenClawArgs) -> Result<()> {
    let mut models = fetch_models(&args.models_api)?;
    sort_models_by_newest(&mut models);

    let selected_models = if args.all_models {
        models
    } else if args.interactive {
        prompt_for_models(&models)?
    } else {
        select_models_by_id(&models, &args.models)?
    };

    if selected_models.is_empty() {
        bail!("No models selected.");
    }

    let output_path = args.output.unwrap_or_else(|| {
        if args.interactive {
            PathBuf::from(DEFAULT_INTERACTIVE_OUTPUT)
        } else {
            PathBuf::from(DEFAULT_ALL_OUTPUT)
        }
    });

    // Resolve API key: flag > interactive prompt > placeholder
    let api_key = match args.api_key {
        Some(key) => key,
        None if args.interactive => prompt_optional_api_key()?,
        None => API_PLACEHOLDER.to_string(),
    };

    let config = build_openclaw_config(&selected_models, args.base_url, api_key, args.max_tokens);

    let op = fs_operator()?;
    let path_str = output_path
        .to_str()
        .context("output path is not valid UTF-8")?;
    write_openclaw_config(&op, path_str, &config)?;

    println!(
        "Wrote {} models to {}",
        selected_models.len(),
        output_path.display()
    );

    Ok(())
}

// ── Command: install ──────────────────────────────────────────────────────────

fn run_openclaw_install(args: InstallOpenClawArgs) -> Result<()> {
    let dest = resolve_openclaw_config_path(args.dest)?;
    let op = fs_operator()?;

    // Read source config to check if API key needs injection
    let src_path = args
        .config_file
        .to_str()
        .context("config_file path is not valid UTF-8")?;
    let src_raw = storage_read(&op, src_path)?.with_context(|| {
        format!(
            "source config {} does not exist",
            args.config_file.display()
        )
    })?;
    let src: OpenClawRoot =
        serde_json::from_str(&src_raw).context("failed to parse source OpenClaw config JSON")?;

    // ── API key resolution ───────────────────────────────────────────────
    let needs_key = src
        .models
        .providers
        .values()
        .any(|p| p.api_key == API_PLACEHOLDER || p.api_key.trim().is_empty());

    let api_key: Option<String> = if args.api_key.is_some() {
        args.api_key
    } else if needs_key {
        if args.non_interactive {
            bail!(
                "The source config contains the default API key placeholder.\n\
                 Supply --api-key <KEY> or run without --non-interactive to be prompted."
            );
        }
        let key = Password::new("ZenMux API key (required – source config contains placeholder):")
            .without_confirmation()
            .prompt()
            .context("failed to read API key input")?;
        if key.trim().is_empty() {
            bail!(
                "API key must not be empty when the source config still contains the placeholder."
            );
        }
        Some(key)
    } else {
        None
    };

    // ── Primary model selection ──────────────────────────────────────────
    // Read dest to get available keys for interactive selection
    let primary = if args.primary.is_some() {
        args.primary
    } else if !args.non_interactive {
        // We need to know available keys for interactive selection.
        // Read dest + source to merge them temporarily.
        let dest_raw = storage_read(&op, &dest)?;
        let mut dest_models: Vec<String> = Vec::new();

        if let Some(raw) = &dest_raw
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(raw)
            && let Some(models) = v
                .get("agents")
                .and_then(|a| a.get("defaults"))
                .and_then(|d| d.get("models"))
                .and_then(|m| m.as_object())
        {
            dest_models.extend(models.keys().cloned());
        }

        // Add source model keys
        for key in src.agents.defaults.models.keys() {
            if !dest_models.contains(key) {
                dest_models.push(key.clone());
            }
        }

        if !dest_models.is_empty() {
            Select::new(
                "Select the primary model (↑↓ to move, Enter to confirm, Esc to keep current):",
                dest_models,
            )
            .with_page_size(20)
            .prompt_skippable()
            .context("failed to read primary model selection")?
        } else {
            None
        }
    } else {
        None
    };

    install_openclaw(&op, src_path, &dest, api_key, primary)?;

    println!("Merged ZenMux config into {dest}");

    Ok(())
}

// ── Command: uninstall ────────────────────────────────────────────────────────

fn run_openclaw_uninstall(args: UninstallOpenClawArgs) -> Result<()> {
    let dest = resolve_openclaw_config_path(args.dest)?;
    let op = fs_operator()?;

    let primary_choice = if args.non_interactive {
        // Non-interactive: the core function auto-picks first fallback
        None
    } else {
        // Read the config first to know if we need to prompt
        let raw = storage_read(&op, &dest)?
            .with_context(|| format!("config {dest} does not exist – nothing to uninstall"))?;
        let target: serde_json::Value = serde_json::from_str(&raw)?;

        let primary_is_zenmux = target
            .get("agents")
            .and_then(|a| a.get("defaults"))
            .and_then(|d| d.get("model"))
            .and_then(|m| m.get("primary"))
            .and_then(|p| p.as_str())
            .map(|s| s.starts_with(ZENMUX_KEY_PREFIX))
            .unwrap_or(false);

        if primary_is_zenmux {
            // Build candidate list for interactive selection
            let model_section = target
                .get("agents")
                .and_then(|a| a.get("defaults"))
                .and_then(|d| d.get("model"));

            let mut candidates: Vec<String> = model_section
                .and_then(|m| m.get("fallbacks"))
                .and_then(|f| f.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .filter(|s| !s.starts_with(ZENMUX_KEY_PREFIX))
                        .collect()
                })
                .unwrap_or_default();

            let remaining_keys: Vec<String> = target
                .get("agents")
                .and_then(|a| a.get("defaults"))
                .and_then(|d| d.get("models"))
                .and_then(|m| m.as_object())
                .map(|m| {
                    m.keys()
                        .filter(|k| !k.starts_with(ZENMUX_KEY_PREFIX))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();

            for key in &remaining_keys {
                if !candidates.contains(key) {
                    candidates.push(key.clone());
                }
            }

            if candidates.is_empty() {
                None
            } else {
                Select::new(
                    "The primary model was a ZenMux model. Select a replacement:",
                    candidates,
                )
                .with_page_size(20)
                .prompt_skippable()
                .context("failed to read primary model selection")?
            }
        } else {
            None
        }
    };

    let (action, _) = uninstall_openclaw(&op, &dest, primary_choice)?;

    match action {
        UninstallPrimaryAction::Restored(key) => {
            println!("Primary model restored to: {key}");
        }
        UninstallPrimaryAction::Cleared => {
            println!("No alternative models available – primary field removed.");
        }
        UninstallPrimaryAction::Unchanged => {}
    }

    println!("Removed ZenMux entries from {dest}");

    Ok(())
}

// ── CLI helpers ───────────────────────────────────────────────────────────────

fn resolve_openclaw_config_path(dest: Option<PathBuf>) -> Result<String> {
    if let Some(path) = dest {
        return path
            .to_str()
            .map(|s| s.to_string())
            .context("dest path is not valid UTF-8");
    }
    let home = env::var("HOME").context("HOME environment variable is not set")?;
    Ok(format!("{home}/{DEFAULT_OPENCLAW_CONFIG}"))
}

fn fetch_models(models_api: &str) -> Result<Vec<ZenMuxModel>> {
    let client = Client::builder()
        .user_agent(format!(
            "{}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(models_api)
        .send()
        .with_context(|| format!("failed to fetch {models_api}"))?
        .error_for_status()
        .with_context(|| format!("ZenMux returned an error for {models_api}"))?;

    let payload: ZenMuxModelsResponse = response
        .json()
        .context("failed to decode ZenMux models payload")?;

    Ok(payload.data)
}

fn prompt_for_models(models: &[ZenMuxModel]) -> Result<Vec<ZenMuxModel>> {
    let options = models
        .iter()
        .cloned()
        .map(|model| SelectableModel { model })
        .collect::<Vec<SelectableModel>>();

    let selected = MultiSelect::new("Select ZenMux models to include", options)
        .with_help_message("Type to filter. Use arrows to move, space to toggle, enter to confirm.")
        .with_page_size(20)
        .prompt()
        .context("failed to read interactive selection")?;

    if selected.is_empty() {
        bail!("No models selected.");
    }

    Ok(selected.into_iter().map(|item| item.model).collect())
}

/// Prompt for an optional API key in interactive generate mode.
/// Pressing Enter (empty input) keeps the placeholder.
fn prompt_optional_api_key() -> Result<String> {
    let key = Text::new("ZenMux API key (optional – press Enter to keep placeholder):")
        .prompt()
        .context("failed to read API key input")?;

    if key.trim().is_empty() {
        Ok(API_PLACEHOLDER.to_string())
    } else {
        Ok(key)
    }
}
