//! Configuration directory management and config file loading.
//!
//! All persistent data lives under `~/.engos/`. The single config file is
//! `~/.engos/config.yml` — a YAML document with a top-level `projects:` list
//! and room for future sections (model settings, keybindings, etc.).
//!
//! Everything here runs *before* the TUI is active so it prints directly to
//! the normal terminal rather than through ratatui.

use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::models::{ModelConfig, Orchestrator};
use crate::report::Report;

// ── Top-level config structure ────────────────────────────────────────────────

/// The complete contents of `~/.engos/config.yml`.
///
/// Designed to grow: new top-level YAML keys become new fields here.
/// `#[serde(default)]` on each field means unknown keys are silently ignored
/// and missing keys use the field's `Default` — older config files always parse.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// List of engagement projects shown on the main screen.
    #[serde(default)]
    pub reports: Vec<Report>,
}

// ── Directory helpers ─────────────────────────────────────────────────────────

/// Return the path to the engos config directory (`~/.engos/`).
///
/// Reads `$HOME` from the environment. Returns `None` if `HOME` is unset,
/// which is unusual on Linux/macOS but possible in hardened environments.
pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".engos"))
}

/// Return the path to the config file (`~/.engos/config.yml`).
pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.yml"))
}

/// Return the path to the models file (`~/.engos/models.yml`).
pub fn models_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("models.yml"))
}

// ── First-run setup ───────────────────────────────────────────────────────────

/// Ensure `~/.engos/` exists, prompting the operator to create it if not.
///
/// Called once at startup, before the TUI enters alternate-screen mode, so
/// the prompt appears in the operator's normal shell context. Returns `true`
/// if the directory is ready, `false` if the operator declined.
///
/// On first creation seeds `config.yml` with example data so the UI is never
/// empty on a first launch.
pub fn ensure_config_dir() -> io::Result<bool> {
    let Some(dir) = config_dir() else {
        eprintln!("warning: $HOME is not set; config will not be persisted this session");
        return Ok(false);
    };

    if dir.exists() {
        return Ok(true);
    }

    // Prompt before touching the filesystem — operator should always be in
    // control of what gets created on their machine.
    print!(
        "\nengos: {} does not exist.\nCreate it for config and project data? [Y/n] ",
        dir.display()
    );
    // Flush so the prompt appears before we block on read_line.
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    // Any input other than an explicit "n" accepts — Enter alone is yes,
    // matching common Unix convention for [Y/n] prompts.
    if input.trim().eq_ignore_ascii_case("n") {
        println!("Skipping — project data will not be saved this session.\n");
        return Ok(false);
    }

    fs::create_dir_all(&dir)?;
    println!("Created {}.\n", dir.display());

    // Seed both YAML files so neither is empty on the very first launch.
    let config_file = dir.join("config.yml");
    if !config_file.exists() {
        fs::write(&config_file, EXAMPLE_CONFIG_YML)?;
    }

    let models_file = dir.join("models.yml");
    if !models_file.exists() {
        fs::write(&models_file, EXAMPLE_MODELS_YML)?;
    }

    Ok(true)
}

// ── Config loading ────────────────────────────────────────────────────────────

/// Load `~/.engos/config.yml` and return the parsed [`Config`].
///
/// Returns a default (empty) `Config` if the file does not exist or cannot be
/// parsed — the application remains fully usable without persistent config.
pub fn load_config() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };

    let text = match fs::read_to_string(&path) {
        Ok(t)  => t,
        // File absent on a session where the operator declined to create the
        // config dir — not an error.
        Err(_) => return Config::default(),
    };

    serde_yml::from_str(&text).unwrap_or_else(|e| {
        // Malformed YAML is worth a warning but must not crash the tool.
        eprintln!("warning: could not parse {}: {e}", path.display());
        Config::default()
    })
}

// ── Config persistence ────────────────────────────────────────────────────────

/// Overwrite `~/.engos/config.yml` with the current project list.
///
/// Called after a new project is created so the next session sees it. Failures
/// are logged to stderr but do not crash the tool — the in-memory state is
/// already correct for the rest of this session.
pub fn persist_config(reports: &[Report]) {
    let Some(path) = config_path() else {
        eprintln!("warning: could not determine config path; project not saved");
        return;
    };
    let cfg = Config { reports: reports.to_vec() };
    match serde_yml::to_string(&cfg) {
        Ok(yaml) => {
            if let Err(e) = fs::write(&path, yaml) {
                eprintln!("warning: could not write {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("warning: could not serialise config: {e}"),
    }
}

// ── Model loading ─────────────────────────────────────────────────────────────

/// Load `~/.engos/models.yml` and return the list of configured orchestrators.
///
/// Returns an empty `Vec` if the file is absent or unparseable — the New
/// Project dropdown simply shows nothing except the "New..." entry.
pub fn load_models() -> Vec<Orchestrator> {
    let Some(path) = models_path() else {
        return vec![];
    };

    let text = match fs::read_to_string(&path) {
        Ok(t)  => t,
        Err(_) => return vec![],
    };

    let cfg: ModelConfig = serde_yml::from_str(&text).unwrap_or_else(|e| {
        eprintln!("warning: could not parse {}: {e}", path.display());
        ModelConfig::default()
    });

    cfg.orchestrators
}

// ── Seed data ─────────────────────────────────────────────────────────────────

/// Written to `models.yml` on the very first launch.
const EXAMPLE_MODELS_YML: &str = r#"# engos orchestrator models
# Add entries here to populate the Specialist Orchestrator dropdown.
#
# vendor values currently recognised: anthropic, openai, local

orchestrators:
  - name: anthropic claude-opus-4-7
    vendor: anthropic

  - name: anthropic claude-sonnet-4-5
    vendor: anthropic

  - name: qwen_7B_1.2.2
    vendor: local
"#;

/// Written to `config.yml` on the very first launch.
///
/// Demonstrates the full schema including the optional `last_opened` and
/// `last_modified` fields so operators know the complete set of supported keys.
const EXAMPLE_CONFIG_YML: &str = r#"# engos configuration
# Reference: https://github.com/your-org/engos

reports:
  - name: Acme Corp Red Team
    start_datetime: "2026-05-14T09:00:00+09:00"
    specialist_model: anthropic claude-opus-4-7
    artifacts_collected: 12
    artifacts_synthesized: 8
    last_opened: "2026-05-25T14:30:00+09:00"
    last_modified: "2026-05-26T09:15:00+09:00"

  - name: TechCo Pentest Q2
    start_datetime: "2026-04-01T08:30:00+09:00"
    specialist_model: qwen_7B_1.2.2
    artifacts_collected: 34
    artifacts_synthesized: 29
    last_opened: "2026-05-20T10:00:00+09:00"
    last_modified: "2026-05-21T16:45:00+09:00"
"#;
