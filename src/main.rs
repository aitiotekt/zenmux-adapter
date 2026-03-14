use std::{collections::BTreeMap, fmt, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use inquire::{MultiSelect, Password, Text};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

const MODELS_API_URL: &str = "https://zenmux.ai/api/v1/models";
const PROVIDER_BASE_URL: &str = "https://zenmux.ai/api/v1";
const PROVIDER_NAME: &str = "zenmux";
const API_PLACEHOLDER: &str = "sk-ss-v1-your-api-key-here";
const OPENCLAW_API_KIND: &str = "openai-completions";
const DEFAULT_MAX_TOKENS: u64 = 8192;
const DEFAULT_ALL_OUTPUT: &str = "zenmux-openclaw-all.json";
const DEFAULT_INTERACTIVE_OUTPUT: &str = "zenmux-openclaw-interative.json";

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
#[command(about = "Install (copy) a generated OpenClaw config to a target path.")]
struct InstallOpenClawArgs {
    /// Path to the generated OpenClaw JSON config file.
    #[arg(value_name = "CONFIG_FILE")]
    config_file: PathBuf,

    /// Destination path to install the config to.
    #[arg(short, long, value_name = "DEST")]
    dest: PathBuf,

    /// API key to inject into the config before installing.
    /// Required when the config still contains the default placeholder.
    /// In non-interactive mode this flag is mandatory if the key is missing.
    /// In interactive mode the CLI will prompt if this flag is omitted.
    #[arg(long, value_name = "KEY")]
    api_key: Option<String>,

    /// Run non-interactively. Fails immediately if --api-key is required but
    /// not supplied on the command line.
    #[arg(long)]
    non_interactive: bool,
}

// ── Subcommand: uninstall ──────────────────────────────────────────────────────

#[derive(Args, Debug)]
#[command(about = "Uninstall (remove) an OpenClaw config from a target path.")]
struct UninstallOpenClawArgs {
    /// Path of the installed config file to remove.
    #[arg(value_name = "DEST")]
    dest: PathBuf,
}

// ── ZenMux API types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ZenMuxModelsResponse {
    data: Vec<ZenMuxModel>,
}

#[derive(Debug, Clone, Deserialize)]
struct ZenMuxModel {
    id: String,
    display_name: String,
    created: i64,
    #[serde(default)]
    input_modalities: Vec<String>,
    #[serde(default)]
    capabilities: ModelCapabilities,
    #[serde(default)]
    context_length: u64,
    #[serde(default)]
    pricings: ModelPricings,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ModelCapabilities {
    #[serde(default)]
    reasoning: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ModelPricings {
    #[serde(default)]
    prompt: Vec<PricingTier>,
    #[serde(default)]
    completion: Vec<PricingTier>,
    #[serde(default)]
    input_cache_read: Vec<PricingTier>,
    #[serde(default)]
    input_cache_write: Vec<PricingTier>,
    #[serde(default)]
    input_cache_write_1_h: Vec<PricingTier>,
    #[serde(default)]
    input_cache_write_5_min: Vec<PricingTier>,
}

#[derive(Debug, Clone, Deserialize)]
struct PricingTier {
    value: f64,
}

// ── OpenClaw config types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawRoot {
    models: OpenClawModelsSection,
    agents: OpenClawAgentsSection,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawModelsSection {
    mode: String,
    providers: BTreeMap<String, OpenClawProvider>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawProvider {
    #[serde(rename = "baseUrl")]
    base_url: String,
    #[serde(rename = "apiKey")]
    api_key: String,
    api: String,
    models: Vec<OpenClawModel>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawModel {
    id: String,
    name: String,
    reasoning: bool,
    input: Vec<String>,
    cost: OpenClawCost,
    #[serde(rename = "contextWindow")]
    context_window: u64,
    #[serde(rename = "maxTokens")]
    max_tokens: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawCost {
    input: f64,
    output: f64,
    #[serde(rename = "cacheRead")]
    cache_read: f64,
    #[serde(rename = "cacheWrite")]
    cache_write: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawAgentsSection {
    defaults: OpenClawAgentDefaults,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawAgentDefaults {
    model: OpenClawPrimaryModel,
    models: BTreeMap<String, EmptyModelConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawPrimaryModel {
    primary: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct EmptyModelConfig {}

// ── Interactive model selector display ────────────────────────────────────────

#[derive(Debug, Clone)]
struct SelectableModel {
    model: ZenMuxModel,
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
    let json = serde_json::to_string_pretty(&config)?;
    fs::write(&output_path, json)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    println!(
        "Wrote {} models to {}",
        selected_models.len(),
        output_path.display()
    );

    Ok(())
}

// ── Command: install ──────────────────────────────────────────────────────────

fn run_openclaw_install(args: InstallOpenClawArgs) -> Result<()> {
    // Read and parse the source config
    let raw = fs::read_to_string(&args.config_file)
        .with_context(|| format!("failed to read {}", args.config_file.display()))?;
    let mut config: OpenClawRoot =
        serde_json::from_str(&raw).context("failed to parse OpenClaw config JSON")?;

    // Check whether any provider still has the placeholder key
    let needs_key = config
        .models
        .providers
        .values()
        .any(|p| p.api_key == API_PLACEHOLDER || p.api_key.trim().is_empty());

    let api_key: Option<String> = if args.api_key.is_some() {
        args.api_key
    } else if needs_key {
        if args.non_interactive {
            bail!(
                "The config file contains the default API key placeholder.\n\
                 Supply --api-key <KEY> or run without --non-interactive to be prompted."
            );
        }
        // Interactive prompt – mandatory (empty input is not accepted)
        let key = Password::new("ZenMux API key (required – config contains placeholder):")
            .without_confirmation()
            .prompt()
            .context("failed to read API key input")?;
        if key.trim().is_empty() {
            bail!("API key must not be empty when the config still contains the placeholder.");
        }
        Some(key)
    } else {
        None
    };

    // Inject the key into every provider
    if let Some(key) = api_key {
        for provider in config.models.providers.values_mut() {
            provider.api_key = key.clone();
        }
    }

    // Create parent dirs and write the config
    if let Some(parent) = args.dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(&config)?;
    fs::write(&args.dest, json)
        .with_context(|| format!("failed to write {}", args.dest.display()))?;

    println!(
        "Installed OpenClaw config to {}",
        args.dest.display()
    );

    Ok(())
}

// ── Command: uninstall ────────────────────────────────────────────────────────

fn run_openclaw_uninstall(args: UninstallOpenClawArgs) -> Result<()> {
    if !args.dest.exists() {
        bail!(
            "Config file {} does not exist – nothing to uninstall.",
            args.dest.display()
        );
    }

    fs::remove_file(&args.dest)
        .with_context(|| format!("failed to remove {}", args.dest.display()))?;

    println!("Uninstalled OpenClaw config from {}", args.dest.display());

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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

fn sort_models_by_newest(models: &mut [ZenMuxModel]) {
    models.sort_by(|left, right| {
        right
            .created
            .cmp(&left.created)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn select_models_by_id(
    models: &[ZenMuxModel],
    requested_ids: &[String],
) -> Result<Vec<ZenMuxModel>> {
    let mut selected = Vec::new();
    let mut missing = Vec::new();

    for requested_id in requested_ids {
        match models.iter().find(|model| model.id == *requested_id) {
            Some(model) => selected.push(model.clone()),
            None => missing.push(requested_id.clone()),
        }
    }

    if !missing.is_empty() {
        bail!("Unknown model ids: {}", missing.join(", "));
    }

    Ok(selected)
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

fn format_selector_label(model: &ZenMuxModel) -> String {
    let reasoning = if model.capabilities.reasoning {
        "reasoning"
    } else {
        "standard"
    };

    format!(
        "{} | {} | ctx={} | inputs={}",
        model.display_name,
        reasoning,
        format_context_window(model.context_length),
        model.input_modalities.join(",")
    )
}

impl fmt::Display for SelectableModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&format_selector_label(&self.model))
    }
}

fn build_openclaw_config(
    models: &[ZenMuxModel],
    base_url: String,
    api_key: String,
    max_tokens: u64,
) -> OpenClawRoot {
    let provider_models = models
        .iter()
        .map(|model| OpenClawModel {
            id: model.id.clone(),
            name: format!("{} via ZenMux", trimmed_display_name(&model.display_name)),
            reasoning: model.capabilities.reasoning,
            input: if model.input_modalities.is_empty() {
                vec!["text".to_string()]
            } else {
                model.input_modalities.clone()
            },
            cost: OpenClawCost {
                input: representative_price(&model.pricings.prompt),
                output: representative_price(&model.pricings.completion),
                cache_read: representative_price(&model.pricings.input_cache_read),
                cache_write: representative_cache_write_price(&model.pricings),
            },
            context_window: model.context_length,
            max_tokens: max_tokens.min(model.context_length.max(1)),
        })
        .collect::<Vec<_>>();

    let mut providers = BTreeMap::new();
    providers.insert(
        PROVIDER_NAME.to_string(),
        OpenClawProvider {
            base_url,
            api_key,
            api: OPENCLAW_API_KIND.to_string(),
            models: provider_models,
        },
    );

    let agent_models = models
        .iter()
        .map(|model| (agent_model_key(&model.id), EmptyModelConfig::default()))
        .collect::<BTreeMap<_, _>>();

    OpenClawRoot {
        models: OpenClawModelsSection {
            mode: "merge".to_string(),
            providers,
        },
        agents: OpenClawAgentsSection {
            defaults: OpenClawAgentDefaults {
                model: OpenClawPrimaryModel {
                    primary: agent_model_key(&models[0].id),
                },
                models: agent_models,
            },
        },
    }
}

fn representative_price(tiers: &[PricingTier]) -> f64 {
    tiers
        .iter()
        .map(|tier| tier.value)
        .reduce(f64::min)
        .unwrap_or(0.0)
}

fn representative_cache_write_price(pricings: &ModelPricings) -> f64 {
    let candidates = [
        representative_price(&pricings.input_cache_write),
        representative_price(&pricings.input_cache_write_1_h),
        representative_price(&pricings.input_cache_write_5_min),
    ];

    candidates
        .into_iter()
        .filter(|value| *value > 0.0)
        .reduce(f64::min)
        .unwrap_or(0.0)
}

fn trimmed_display_name(display_name: &str) -> &str {
    display_name
        .split_once(':')
        .map(|(_, name)| name.trim())
        .unwrap_or(display_name)
}

fn agent_model_key(model_id: &str) -> String {
    format!("{PROVIDER_NAME}/{model_id}")
}

fn format_context_window(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.2}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.2}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}
