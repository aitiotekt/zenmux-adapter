use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Context, Result, bail};
use opendal::blocking;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Public constants ──────────────────────────────────────────────────────────

pub const MODELS_API_URL: &str = "https://zenmux.ai/api/v1/models";
pub const PROVIDER_BASE_URL: &str = "https://zenmux.ai/api/v1";
pub const PROVIDER_NAME: &str = "zenmux";
pub const API_PLACEHOLDER: &str = "sk-ss-v1-your-api-key-here";
pub const ZENMUX_KEY_PREFIX: &str = "zenmux/";
pub const OPENCLAW_API_KIND: &str = "openai-completions";
pub const DEFAULT_MAX_TOKENS: u64 = 8192;
pub const DEFAULT_ALL_OUTPUT: &str = "zenmux-openclaw-all.json";
pub const DEFAULT_INTERACTIVE_OUTPUT: &str = "zenmux-openclaw-interative.json";
pub const DEFAULT_OPENCLAW_CONFIG: &str = ".openclaw/openclaw.json";

// ── ZenMux API types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ZenMuxModelsResponse {
    pub data: Vec<ZenMuxModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZenMuxModel {
    pub id: String,
    pub display_name: String,
    pub created: i64,
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    pub context_length: u64,
    #[serde(default)]
    pub pricings: ModelPricings,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelCapabilities {
    #[serde(default)]
    pub reasoning: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelPricings {
    #[serde(default)]
    pub prompt: Vec<PricingTier>,
    #[serde(default)]
    pub completion: Vec<PricingTier>,
    #[serde(default)]
    pub input_cache_read: Vec<PricingTier>,
    #[serde(default)]
    pub input_cache_write: Vec<PricingTier>,
    #[serde(default)]
    pub input_cache_write_1_h: Vec<PricingTier>,
    #[serde(default)]
    pub input_cache_write_5_min: Vec<PricingTier>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PricingTier {
    pub value: f64,
}

// ── OpenClaw config types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenClawRoot {
    pub models: OpenClawModelsSection,
    pub agents: OpenClawAgentsSection,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenClawModelsSection {
    pub mode: String,
    pub providers: BTreeMap<String, OpenClawProvider>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenClawProvider {
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub api: String,
    pub models: Vec<OpenClawModel>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenClawModel {
    pub id: String,
    pub name: String,
    pub reasoning: bool,
    pub input: Vec<String>,
    pub cost: OpenClawCost,
    #[serde(rename = "contextWindow")]
    pub context_window: u64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenClawCost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenClawAgentsSection {
    pub defaults: OpenClawAgentDefaults,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenClawAgentDefaults {
    pub model: OpenClawPrimaryModel,
    pub models: BTreeMap<String, EmptyModelConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenClawPrimaryModel {
    pub primary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EmptyModelConfig {}

// ── Interactive model selector display ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SelectableModel {
    pub model: ZenMuxModel,
}

pub fn format_selector_label(model: &ZenMuxModel) -> String {
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

// ── Storage helpers (OpenDAL) ─────────────────────────────────────────────────

/// Read a UTF-8 file from the operator. Returns `None` if the path does not
/// exist (stat returns `NotFound`).
pub fn storage_read(op: &blocking::Operator, path: &str) -> Result<Option<String>> {
    match op.stat(path) {
        Ok(_) => {
            let bytes = op
                .read(path)
                .with_context(|| format!("failed to read {path}"))?;
            let content = String::from_utf8(bytes.to_vec())
                .with_context(|| format!("{path} is not valid UTF-8"))?;
            Ok(Some(content))
        }
        Err(e) if e.kind() == opendal::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("failed to stat {path}")),
    }
}

/// Write UTF-8 content to the operator. Parent directories are created
/// automatically by most OpenDAL backends.
pub fn storage_write(op: &blocking::Operator, path: &str, content: &str) -> Result<()> {
    op.write(path, content.as_bytes().to_vec())
        .with_context(|| format!("failed to write {path}"))?;
    Ok(())
}

/// Global tokio runtime for OpenDAL blocking operators.
/// In async contexts (e.g., within `#[tokio::main]`) the current handle is
/// used automatically.  In pure blocking contexts (CLI main, tests) we fall
/// back to this lazily-initialised runtime.
static RUNTIME: std::sync::LazyLock<tokio::runtime::Runtime> = std::sync::LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime for OpenDAL blocking operator")
});

fn make_blocking(async_op: opendal::Operator) -> Result<blocking::Operator> {
    // Try current async handle first; fall back to global runtime.
    let _guard = match tokio::runtime::Handle::try_current() {
        Ok(_) => None,
        Err(_) => Some(RUNTIME.enter()),
    };
    Ok(blocking::Operator::new(async_op)?)
}

/// Create an `Fs`-backed blocking operator rooted at `/`.
pub fn fs_operator() -> Result<blocking::Operator> {
    let builder = opendal::services::Fs::default().root("/");
    let async_op = opendal::Operator::new(builder)?.finish();
    make_blocking(async_op)
}

/// Create an in-memory blocking operator (for tests).
pub fn memory_operator() -> Result<blocking::Operator> {
    let builder = opendal::services::Memory::default();
    let async_op = opendal::Operator::new(builder)?.finish();
    make_blocking(async_op)
}

// ── Core logic: generate ──────────────────────────────────────────────────────

pub fn write_openclaw_config(
    op: &blocking::Operator,
    path: &str,
    config: &OpenClawRoot,
) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    storage_write(op, path, &json)
}

// ── Core logic: install (merge) ───────────────────────────────────────────────

pub fn install_openclaw(
    op: &blocking::Operator,
    source_path: &str,
    dest_path: &str,
    api_key: Option<String>,
    primary: Option<String>,
) -> Result<()> {
    // 1. Read the generated zenmux source config
    let src_raw = storage_read(op, source_path)?
        .with_context(|| format!("source config {source_path} does not exist"))?;
    let mut src: OpenClawRoot =
        serde_json::from_str(&src_raw).context("failed to parse source OpenClaw config JSON")?;

    // 2. API key injection
    if let Some(key) = api_key {
        for provider in src.models.providers.values_mut() {
            provider.api_key = key.clone();
        }
    }

    // 3. Read (or create) the target OpenClaw config as Value
    let mut target: Value = match storage_read(op, dest_path)? {
        Some(raw) => serde_json::from_str(&raw)
            .context("failed to parse target OpenClaw config JSON")?,
        None => serde_json::json!({}),
    };

    // 4. Merge zenmux provider into target
    let target_obj = target
        .as_object_mut()
        .context("target OpenClaw config is not a JSON object")?;

    let models_section = target_obj
        .entry("models")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("models is not an object")?;

    let providers = models_section
        .entry("providers")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("providers is not an object")?;

    if let Some(zenmux_provider) = src.models.providers.get(PROVIDER_NAME) {
        providers.insert(
            PROVIDER_NAME.to_string(),
            serde_json::to_value(zenmux_provider)?,
        );
    }

    // 5. Merge zenmux model keys into agents.defaults.models
    let agents_section = target_obj
        .entry("agents")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("agents is not an object")?;

    let defaults = agents_section
        .entry("defaults")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("defaults is not an object")?;

    let available_keys: Vec<String> = {
        let models_map = defaults
            .entry("models")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .context("models is not an object")?;

        for (key, val) in &src.agents.defaults.models {
            models_map.insert(key.clone(), serde_json::to_value(val)?);
        }

        models_map.keys().cloned().collect()
    };

    // 6. Primary model selection
    let model_section = defaults
        .entry("model")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("model is not an object")?;

    let current_primary = model_section
        .get("primary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(new_key) = primary {
        if !available_keys.contains(&new_key) {
            bail!(
                "Model key '{}' is not in the models list.\nAvailable: {}",
                new_key,
                available_keys.join(", ")
            );
        }

        let fallbacks = model_section
            .entry("fallbacks")
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .context("fallbacks is not an array")?;

        fallbacks.retain(|v| v.as_str() != Some(&new_key));

        if !current_primary.is_empty() && current_primary != new_key {
            fallbacks.retain(|v| v.as_str() != Some(&current_primary));
            fallbacks.insert(0, Value::String(current_primary));
        }

        model_section.insert("primary".to_string(), Value::String(new_key));
    }

    // 7. Write output
    let json = serde_json::to_string_pretty(&target)?;
    storage_write(op, dest_path, &json)?;

    Ok(())
}

// ── Core logic: uninstall (remove) ────────────────────────────────────────────

/// Result of an uninstall operation, indicating whether primary recovery is
/// needed.
pub enum UninstallPrimaryAction {
    /// Primary was not a zenmux model – no action needed.
    Unchanged,
    /// Primary was a zenmux model and was automatically restored.
    Restored(String),
    /// Primary was a zenmux model but no candidates are available.
    Cleared,
}

pub fn uninstall_openclaw(
    op: &blocking::Operator,
    dest_path: &str,
    primary_choice: Option<String>,
) -> Result<(UninstallPrimaryAction, Vec<String>)> {
    let raw = storage_read(op, dest_path)?
        .with_context(|| format!("config {dest_path} does not exist – nothing to uninstall"))?;
    let mut target: Value =
        serde_json::from_str(&raw).context("failed to parse OpenClaw config JSON")?;

    let target_obj = target
        .as_object_mut()
        .context("OpenClaw config is not a JSON object")?;

    // 1. Remove zenmux provider
    if let Some(providers) = target_obj
        .get_mut("models")
        .and_then(|m| m.get_mut("providers"))
        .and_then(|p| p.as_object_mut())
    {
        providers.remove(PROVIDER_NAME);
    }

    // 2. Remove zenmux model keys from agents.defaults.models
    let defaults = target_obj
        .get_mut("agents")
        .and_then(|a| a.get_mut("defaults"));

    let mut action = UninstallPrimaryAction::Unchanged;
    let mut candidates: Vec<String> = Vec::new();

    if let Some(defaults) = defaults {
        if let Some(models_map) = defaults.get_mut("models").and_then(|m| m.as_object_mut()) {
            let zenmux_keys: Vec<String> = models_map
                .keys()
                .filter(|k| k.starts_with(ZENMUX_KEY_PREFIX))
                .cloned()
                .collect();
            for key in &zenmux_keys {
                models_map.remove(key);
            }
        }

        let remaining_model_keys: Vec<String> = defaults
            .get("models")
            .and_then(|m| m.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        if let Some(model_section) = defaults.get_mut("model").and_then(|m| m.as_object_mut()) {
            if let Some(fallbacks) =
                model_section.get_mut("fallbacks").and_then(|f| f.as_array_mut())
            {
                fallbacks.retain(|v| {
                    v.as_str()
                        .map(|s| !s.starts_with(ZENMUX_KEY_PREFIX))
                        .unwrap_or(true)
                });
            }

            let primary_is_zenmux = model_section
                .get("primary")
                .and_then(|v| v.as_str())
                .map(|s| s.starts_with(ZENMUX_KEY_PREFIX))
                .unwrap_or(false);

            if primary_is_zenmux {
                // Build candidate list
                candidates = model_section
                    .get("fallbacks")
                    .and_then(|f| f.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                for key in &remaining_model_keys {
                    if !candidates.contains(key) {
                        candidates.push(key.clone());
                    }
                }

                // Determine the new primary
                let new_primary = primary_choice.or_else(|| candidates.first().cloned());

                match new_primary {
                    Some(key) => {
                        if let Some(fallbacks) =
                            model_section.get_mut("fallbacks").and_then(|f| f.as_array_mut())
                        {
                            fallbacks.retain(|v| v.as_str() != Some(&key));
                        }
                        model_section.insert("primary".to_string(), Value::String(key.clone()));
                        action = UninstallPrimaryAction::Restored(key);
                    }
                    None => {
                        model_section.remove("primary");
                        action = UninstallPrimaryAction::Cleared;
                    }
                }
            }
        }
    }

    // 4. Write back
    let json = serde_json::to_string_pretty(&target)?;
    storage_write(op, dest_path, &json)?;

    Ok((action, candidates))
}

// ── Pure helpers ──────────────────────────────────────────────────────────────

pub fn sort_models_by_newest(models: &mut [ZenMuxModel]) {
    models.sort_by(|left, right| {
        right
            .created
            .cmp(&left.created)
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub fn select_models_by_id(
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

pub fn build_openclaw_config(
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
                    fallbacks: Vec::new(),
                },
                models: agent_models,
            },
        },
    }
}

pub fn representative_price(tiers: &[PricingTier]) -> f64 {
    tiers
        .iter()
        .map(|tier| tier.value)
        .reduce(f64::min)
        .unwrap_or(0.0)
}

pub fn representative_cache_write_price(pricings: &ModelPricings) -> f64 {
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

pub fn trimmed_display_name(display_name: &str) -> &str {
    display_name
        .split_once(':')
        .map(|(_, name)| name.trim())
        .unwrap_or(display_name)
}

pub fn agent_model_key(model_id: &str) -> String {
    format!("{PROVIDER_NAME}/{model_id}")
}

pub fn format_context_window(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.2}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.2}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_model(id: &str, display_name: &str, created: i64) -> ZenMuxModel {
        ZenMuxModel {
            id: id.to_string(),
            display_name: display_name.to_string(),
            created,
            input_modalities: vec!["text".to_string()],
            capabilities: ModelCapabilities { reasoning: true },
            context_length: 128_000,
            pricings: ModelPricings {
                prompt: vec![PricingTier { value: 1.0 }],
                completion: vec![PricingTier { value: 2.0 }],
                input_cache_read: vec![PricingTier { value: 0.1 }],
                input_cache_write: vec![PricingTier { value: 0.5 }],
                ..Default::default()
            },
        }
    }

    fn make_two_models() -> Vec<ZenMuxModel> {
        vec![
            make_test_model("openai/gpt-5", "OpenAI: GPT-5", 1000),
            make_test_model("anthropic/claude-5", "Anthropic: Claude 5", 900),
        ]
    }

    // ── build_openclaw_config tests ───────────────────────────────────────

    #[test]
    fn test_build_config_produces_correct_provider() {
        let models = make_two_models();
        let config = build_openclaw_config(
            &models,
            "https://example.com/v1".to_string(),
            "sk-test".to_string(),
            4096,
        );

        assert_eq!(config.models.mode, "merge");
        let provider = config.models.providers.get(PROVIDER_NAME).unwrap();
        assert_eq!(provider.base_url, "https://example.com/v1");
        assert_eq!(provider.api_key, "sk-test");
        assert_eq!(provider.api, OPENCLAW_API_KIND);
        assert_eq!(provider.models.len(), 2);
    }

    #[test]
    fn test_build_config_sets_primary_to_first_model() {
        let models = make_two_models();
        let config = build_openclaw_config(
            &models,
            PROVIDER_BASE_URL.to_string(),
            API_PLACEHOLDER.to_string(),
            8192,
        );

        assert_eq!(
            config.agents.defaults.model.primary,
            "zenmux/openai/gpt-5"
        );
        assert!(config.agents.defaults.model.fallbacks.is_empty());
    }

    #[test]
    fn test_build_config_model_keys_match() {
        let models = make_two_models();
        let config = build_openclaw_config(
            &models,
            PROVIDER_BASE_URL.to_string(),
            API_PLACEHOLDER.to_string(),
            8192,
        );

        let keys: Vec<&String> = config.agents.defaults.models.keys().collect();
        assert!(keys.contains(&&"zenmux/openai/gpt-5".to_string()));
        assert!(keys.contains(&&"zenmux/anthropic/claude-5".to_string()));
    }

    #[test]
    fn test_max_tokens_capped_by_context_length() {
        let models = vec![make_test_model("test/model", "Test", 1000)];
        let config = build_openclaw_config(
            &models,
            PROVIDER_BASE_URL.to_string(),
            API_PLACEHOLDER.to_string(),
            999_999, // bigger than context_length
        );

        let m = &config.models.providers[PROVIDER_NAME].models[0];
        assert_eq!(m.max_tokens, 128_000); // capped to context_length
    }

    // ── sort_models_by_newest tests ───────────────────────────────────────

    #[test]
    fn test_sort_models_by_newest() {
        let mut models = vec![
            make_test_model("a", "A", 100),
            make_test_model("c", "C", 300),
            make_test_model("b", "B", 200),
        ];
        sort_models_by_newest(&mut models);
        assert_eq!(models[0].id, "c");
        assert_eq!(models[1].id, "b");
        assert_eq!(models[2].id, "a");
    }

    #[test]
    fn test_sort_models_tiebreak_by_id() {
        let mut models = vec![
            make_test_model("zoo", "Zoo", 100),
            make_test_model("alpha", "Alpha", 100),
        ];
        sort_models_by_newest(&mut models);
        assert_eq!(models[0].id, "alpha");
        assert_eq!(models[1].id, "zoo");
    }

    // ── select_models_by_id tests ────────────────────────────────────────

    #[test]
    fn test_select_models_by_id_ok() {
        let models = make_two_models();
        let selected = select_models_by_id(
            &models,
            &["anthropic/claude-5".to_string()],
        )
        .unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "anthropic/claude-5");
    }

    #[test]
    fn test_select_models_by_id_missing() {
        let models = make_two_models();
        let result = select_models_by_id(
            &models,
            &["nonexistent/model".to_string()],
        );
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("nonexistent/model")
        );
    }

    // ── Pure helper tests ────────────────────────────────────────────────

    #[test]
    fn test_trimmed_display_name() {
        assert_eq!(trimmed_display_name("OpenAI: GPT-5"), "GPT-5");
        assert_eq!(trimmed_display_name("NoColon"), "NoColon");
        assert_eq!(trimmed_display_name("Prefix:  Spaced  "), "Spaced");
    }

    #[test]
    fn test_agent_model_key() {
        assert_eq!(agent_model_key("openai/gpt-5"), "zenmux/openai/gpt-5");
    }

    #[test]
    fn test_format_context_window() {
        assert_eq!(format_context_window(500), "500");
        assert_eq!(format_context_window(128_000), "128.00K");
        assert_eq!(format_context_window(1_000_000), "1.00M");
        assert_eq!(format_context_window(2_500_000), "2.50M");
    }

    #[test]
    fn test_representative_price() {
        assert_eq!(representative_price(&[]), 0.0);
        assert_eq!(
            representative_price(&[PricingTier { value: 3.0 }, PricingTier { value: 1.5 }]),
            1.5
        );
    }

    // ── Storage-backed tests (in-memory) ─────────────────────────────────

    #[test]
    fn test_storage_read_write_roundtrip() {
        let op = memory_operator().unwrap();
        storage_write(&op, "test.txt", "hello world").unwrap();
        let content = storage_read(&op, "test.txt").unwrap();
        assert_eq!(content.unwrap(), "hello world");
    }

    #[test]
    fn test_storage_read_nonexistent() {
        let op = memory_operator().unwrap();
        let content = storage_read(&op, "nonexistent.txt").unwrap();
        assert!(content.is_none());
    }

    #[test]
    fn test_write_and_read_openclaw_config() {
        let op = memory_operator().unwrap();
        let models = make_two_models();
        let config = build_openclaw_config(
            &models,
            PROVIDER_BASE_URL.to_string(),
            "sk-test-key".to_string(),
            8192,
        );

        write_openclaw_config(&op, "output.json", &config).unwrap();

        let raw = storage_read(&op, "output.json").unwrap().unwrap();
        let parsed: OpenClawRoot = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.agents.defaults.model.primary, "zenmux/openai/gpt-5");
        assert_eq!(
            parsed.models.providers[PROVIDER_NAME].api_key,
            "sk-test-key"
        );
    }

    // ── install_openclaw tests ───────────────────────────────────────────

    #[test]
    fn test_install_into_empty_dest() {
        let op = memory_operator().unwrap();
        let models = make_two_models();
        let config = build_openclaw_config(
            &models,
            PROVIDER_BASE_URL.to_string(),
            API_PLACEHOLDER.to_string(),
            8192,
        );

        // Write source config
        let src_json = serde_json::to_string_pretty(&config).unwrap();
        storage_write(&op, "src.json", &src_json).unwrap();

        // Install with API key injection and primary selection
        install_openclaw(
            &op,
            "src.json",
            "dest.json",
            Some("sk-real-key".to_string()),
            Some("zenmux/anthropic/claude-5".to_string()),
        )
        .unwrap();

        let raw = storage_read(&op, "dest.json").unwrap().unwrap();
        let dest: Value = serde_json::from_str(&raw).unwrap();

        // Verify provider was merged with real key
        let provider = &dest["models"]["providers"]["zenmux"];
        assert_eq!(provider["apiKey"].as_str().unwrap(), "sk-real-key");

        // Verify primary was changed
        let primary = dest["agents"]["defaults"]["model"]["primary"]
            .as_str()
            .unwrap();
        assert_eq!(primary, "zenmux/anthropic/claude-5");

        // When dest starts empty, there's no old primary to move into fallback
        let fallbacks = dest["agents"]["defaults"]["model"]["fallbacks"]
            .as_array()
            .unwrap();
        assert!(fallbacks.is_empty());
    }

    #[test]
    fn test_install_merges_into_existing_config() {
        let op = memory_operator().unwrap();

        // Write an existing config with a different provider
        let existing = serde_json::json!({
            "models": {
                "mode": "merge",
                "providers": {
                    "other-provider": {
                        "baseUrl": "https://other.com",
                        "apiKey": "other-key",
                        "api": "openai-completions",
                        "models": []
                    }
                }
            },
            "agents": {
                "defaults": {
                    "model": {
                        "primary": "other-provider/model-a",
                        "fallbacks": ["other-provider/model-b"]
                    },
                    "models": {
                        "other-provider/model-a": {},
                        "other-provider/model-b": {}
                    }
                }
            }
        });
        storage_write(&op, "dest.json", &serde_json::to_string_pretty(&existing).unwrap())
            .unwrap();

        // Create source config
        let models = make_two_models();
        let config = build_openclaw_config(
            &models,
            PROVIDER_BASE_URL.to_string(),
            "sk-test".to_string(),
            8192,
        );
        storage_write(
            &op,
            "src.json",
            &serde_json::to_string_pretty(&config).unwrap(),
        )
        .unwrap();

        // Install without changing primary
        install_openclaw(&op, "src.json", "dest.json", None, None).unwrap();

        let raw = storage_read(&op, "dest.json").unwrap().unwrap();
        let dest: Value = serde_json::from_str(&raw).unwrap();

        // other-provider should still exist
        assert!(dest["models"]["providers"]["other-provider"].is_object());
        // zenmux provider should be added
        assert!(dest["models"]["providers"]["zenmux"].is_object());
        // Original primary should be unchanged (no --primary passed)
        assert_eq!(
            dest["agents"]["defaults"]["model"]["primary"].as_str().unwrap(),
            "other-provider/model-a"
        );
        // zenmux model keys should be merged
        assert!(dest["agents"]["defaults"]["models"]["zenmux/openai/gpt-5"].is_object());
        assert!(dest["agents"]["defaults"]["models"]["other-provider/model-a"].is_object());
    }

    #[test]
    fn test_install_rejects_invalid_primary() {
        let op = memory_operator().unwrap();
        let models = make_two_models();
        let config = build_openclaw_config(
            &models,
            PROVIDER_BASE_URL.to_string(),
            "sk-test".to_string(),
            8192,
        );
        storage_write(
            &op,
            "src.json",
            &serde_json::to_string_pretty(&config).unwrap(),
        )
        .unwrap();

        let result = install_openclaw(
            &op,
            "src.json",
            "dest.json",
            None,
            Some("nonexistent/model".to_string()),
        );
        assert!(result.is_err());
    }

    // ── uninstall_openclaw tests ─────────────────────────────────────────

    #[test]
    fn test_uninstall_removes_zenmux_entries() {
        let op = memory_operator().unwrap();

        let config = serde_json::json!({
            "models": {
                "mode": "merge",
                "providers": {
                    "zenmux": {
                        "baseUrl": "https://zenmux.ai/api/v1",
                        "apiKey": "sk-test",
                        "api": "openai-completions",
                        "models": [{"id": "openai/gpt-5"}]
                    },
                    "other": {
                        "baseUrl": "https://other.com",
                        "apiKey": "other-key",
                        "api": "openai-completions",
                        "models": []
                    }
                }
            },
            "agents": {
                "defaults": {
                    "model": {
                        "primary": "other/model-a",
                        "fallbacks": ["zenmux/openai/gpt-5", "other/model-b"]
                    },
                    "models": {
                        "zenmux/openai/gpt-5": {},
                        "other/model-a": {},
                        "other/model-b": {}
                    }
                }
            }
        });
        storage_write(&op, "config.json", &serde_json::to_string_pretty(&config).unwrap())
            .unwrap();

        let (action, _) = uninstall_openclaw(&op, "config.json", None).unwrap();
        assert!(matches!(action, UninstallPrimaryAction::Unchanged));

        let raw = storage_read(&op, "config.json").unwrap().unwrap();
        let result: Value = serde_json::from_str(&raw).unwrap();

        // zenmux provider removed
        assert!(result["models"]["providers"]["zenmux"].is_null());
        // other provider preserved
        assert!(result["models"]["providers"]["other"].is_object());
        // zenmux model key removed
        assert!(result["agents"]["defaults"]["models"]["zenmux/openai/gpt-5"].is_null());
        // other model keys preserved
        assert!(result["agents"]["defaults"]["models"]["other/model-a"].is_object());
        // zenmux fallback entry removed
        let fallbacks = result["agents"]["defaults"]["model"]["fallbacks"]
            .as_array()
            .unwrap();
        assert_eq!(fallbacks.len(), 1);
        assert_eq!(fallbacks[0].as_str().unwrap(), "other/model-b");
    }

    #[test]
    fn test_uninstall_restores_primary_from_fallback() {
        let op = memory_operator().unwrap();

        let config = serde_json::json!({
            "models": {
                "mode": "merge",
                "providers": {
                    "zenmux": {
                        "baseUrl": "https://zenmux.ai/api/v1",
                        "apiKey": "sk-test",
                        "api": "openai-completions",
                        "models": []
                    }
                }
            },
            "agents": {
                "defaults": {
                    "model": {
                        "primary": "zenmux/openai/gpt-5",
                        "fallbacks": ["github-copilot/gpt-5", "github-copilot/claude-5"]
                    },
                    "models": {
                        "zenmux/openai/gpt-5": {}
                    }
                }
            }
        });
        storage_write(&op, "config.json", &serde_json::to_string_pretty(&config).unwrap())
            .unwrap();

        // Non-interactive: should auto-pick first fallback
        let (action, _) = uninstall_openclaw(&op, "config.json", None).unwrap();
        match action {
            UninstallPrimaryAction::Restored(key) => {
                assert_eq!(key, "github-copilot/gpt-5");
            }
            _ => panic!("expected Restored"),
        }

        let raw = storage_read(&op, "config.json").unwrap().unwrap();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            result["agents"]["defaults"]["model"]["primary"].as_str().unwrap(),
            "github-copilot/gpt-5"
        );
        // First fallback promoted – should be removed from the list
        let fallbacks = result["agents"]["defaults"]["model"]["fallbacks"]
            .as_array()
            .unwrap();
        assert_eq!(fallbacks.len(), 1);
        assert_eq!(fallbacks[0].as_str().unwrap(), "github-copilot/claude-5");
    }

    #[test]
    fn test_uninstall_clears_primary_when_no_candidates() {
        let op = memory_operator().unwrap();

        let config = serde_json::json!({
            "models": {
                "mode": "merge",
                "providers": {
                    "zenmux": {
                        "baseUrl": "https://zenmux.ai/api/v1",
                        "apiKey": "sk-test",
                        "api": "openai-completions",
                        "models": []
                    }
                }
            },
            "agents": {
                "defaults": {
                    "model": {
                        "primary": "zenmux/openai/gpt-5"
                    },
                    "models": {
                        "zenmux/openai/gpt-5": {}
                    }
                }
            }
        });
        storage_write(&op, "config.json", &serde_json::to_string_pretty(&config).unwrap())
            .unwrap();

        let (action, _) = uninstall_openclaw(&op, "config.json", None).unwrap();
        assert!(matches!(action, UninstallPrimaryAction::Cleared));

        let raw = storage_read(&op, "config.json").unwrap().unwrap();
        let result: Value = serde_json::from_str(&raw).unwrap();
        // primary should be removed entirely
        assert!(result["agents"]["defaults"]["model"]["primary"].is_null());
    }

    #[test]
    fn test_uninstall_with_explicit_primary_choice() {
        let op = memory_operator().unwrap();

        let config = serde_json::json!({
            "models": {
                "mode": "merge",
                "providers": {
                    "zenmux": {
                        "baseUrl": "https://zenmux.ai/api/v1",
                        "apiKey": "sk-test",
                        "api": "openai-completions",
                        "models": []
                    }
                }
            },
            "agents": {
                "defaults": {
                    "model": {
                        "primary": "zenmux/openai/gpt-5",
                        "fallbacks": ["other/model-a", "other/model-b"]
                    },
                    "models": {
                        "zenmux/openai/gpt-5": {},
                        "other/model-a": {},
                        "other/model-b": {}
                    }
                }
            }
        });
        storage_write(&op, "config.json", &serde_json::to_string_pretty(&config).unwrap())
            .unwrap();

        // Explicit choice: pick model-b instead of auto first fallback
        let (action, _) = uninstall_openclaw(
            &op,
            "config.json",
            Some("other/model-b".to_string()),
        )
        .unwrap();
        match action {
            UninstallPrimaryAction::Restored(key) => assert_eq!(key, "other/model-b"),
            _ => panic!("expected Restored"),
        }

        let raw = storage_read(&op, "config.json").unwrap().unwrap();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            result["agents"]["defaults"]["model"]["primary"].as_str().unwrap(),
            "other/model-b"
        );
    }

    #[test]
    fn test_uninstall_nonexistent_file_fails() {
        let op = memory_operator().unwrap();
        let result = uninstall_openclaw(&op, "nonexistent.json", None);
        assert!(result.is_err());
    }
}
