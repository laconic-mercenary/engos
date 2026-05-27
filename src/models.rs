//! Orchestrator model definitions — loaded from and serialised to
//! `~/.engos/models.yml`.

// Serialize is needed when writing a newly-created orchestrator back to disk.
use serde::{Deserialize, Serialize};

/// A single configured orchestrator (LLM backend).
///
/// Stored in `models.yml`. When a new orchestrator is created through the UI
/// it is appended to this list and the file is rewritten.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Orchestrator {
    /// Display name shown in dropdowns, e.g. `"anthropic claude-opus-4-7"`.
    pub name: String,
    /// Vendor identifier used to select the API client: `"anthropic"` or
    /// `"local"`.
    pub vendor: String,
    /// API key for cloud-hosted vendors (Anthropic, OpenAI, etc.).
    ///
    /// Omitted from the YAML when absent so the file stays clean.
    /// NOTE: storing a raw key in a config file is a known trade-off —
    /// a future version will support env-var references instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Top-level structure of `~/.engos/models.yml`.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ModelConfig {
    /// Ordered list of orchestrators shown in the New Project dropdown.
    #[serde(default)]
    pub orchestrators: Vec<Orchestrator>,
}
