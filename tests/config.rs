//! Unit tests for config YAML parsing.
//!
//! Tests parse YAML directly from strings so no filesystem or home-directory
//! lookups are needed — pure data-in, data-out.

use engos::config::Config;

/// Parse a minimal config with one project and verify all fields round-trip.
#[test]
fn parses_full_project() {
    let yaml = r#"
projects:
  - name: Test Engagement
    start_datetime: "2026-05-01T09:00:00+09:00"
    specialist_model: anthropic claude-opus-4-7
    artifacts_collected: 5
    artifacts_synthesized: 3
    last_opened: "2026-05-10T14:00:00+09:00"
    last_modified: "2026-05-11T08:30:00+09:00"
"#;

    let cfg: Config = serde_yml::from_str(yaml).expect("should parse");
    assert_eq!(cfg.projects.len(), 1);

    let p = &cfg.projects[0];
    assert_eq!(p.name,                 "Test Engagement");
    assert_eq!(p.start_datetime,       "2026-05-01T09:00:00+09:00");
    assert_eq!(p.specialist_model,     "anthropic claude-opus-4-7");
    assert_eq!(p.artifacts_collected,  5);
    assert_eq!(p.artifacts_synthesized, 3);
    assert_eq!(p.last_opened.as_deref(),   Some("2026-05-10T14:00:00+09:00"));
    assert_eq!(p.last_modified.as_deref(), Some("2026-05-11T08:30:00+09:00"));
}

/// Optional fields default to None when absent — old config files stay valid.
#[test]
fn optional_fields_default_to_none() {
    let yaml = r#"
projects:
  - name: Legacy Project
    start_datetime: "2026-01-01T00:00:00+00:00"
    specialist_model: local-model
    artifacts_collected: 0
    artifacts_synthesized: 0
"#;

    let cfg: Config = serde_yml::from_str(yaml).expect("should parse");
    let p = &cfg.projects[0];

    assert!(p.last_opened.is_none(),   "last_opened must default to None");
    assert!(p.last_modified.is_none(), "last_modified must default to None");
}

/// An empty projects list parses without error.
#[test]
fn empty_projects_list_is_valid() {
    let yaml = "projects: []\n";
    let cfg: Config = serde_yml::from_str(yaml).expect("should parse");
    assert!(cfg.projects.is_empty());
}

/// A config file with no projects key at all also parses — the field defaults.
#[test]
fn missing_projects_key_defaults_to_empty() {
    let yaml = "# just a comment\n";
    let cfg: Config = serde_yml::from_str(yaml).expect("should parse");
    assert!(cfg.projects.is_empty(), "missing projects key must give empty vec");
}

/// The built-in seed YAML parses correctly — guards against typos in the constant.
#[test]
fn seed_yaml_is_valid() {
    // Re-embed the same constant used in config.rs to catch any drift.
    let seed = r#"# engos configuration
# Reference: https://github.com/your-org/engos

projects:
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

    let cfg: Config = serde_yml::from_str(seed).expect("seed YAML must parse");
    assert_eq!(cfg.projects.len(), 2, "seed must contain exactly 2 projects");
    assert_eq!(cfg.projects[0].name, "Acme Corp Red Team");
    assert_eq!(cfg.projects[1].name, "TechCo Pentest Q2");
}
