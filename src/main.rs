//! `engos` binary entry point.
//!
//! Handles CLI flags, ensures the config directory exists, loads data,
//! then hands off to the TUI — or runs a self-contained command if a
//! recognised flag is passed.

use engos::{anthropic, config, tui};

fn main() -> std::io::Result<()> {
    // Handle meta-flags before touching the terminal or the filesystem.
    let first_arg = std::env::args().nth(1);
    match first_arg.as_deref() {
        Some("--version") | Some("-V") => {
            println!("engos {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help") | Some("-h") => {
            eprintln!("Usage: engos [options]");
            eprintln!();
            eprintln!("Options:");
            eprintln!("  --check       Verify Anthropic API connectivity using configured models");
            eprintln!("  --version     Print version and exit");
            eprintln!("  --help        Print this help text");
            return Ok(());
        }
        // Run the API connectivity check and exit — no TUI involved.
        Some("--check") => {
            check_api();
            return Ok(());
        }
        _ => {}
    }

    // Resolve the watch path (unused until Phase 3 watcher wiring).
    let watch_path = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .unwrap_or_else(|| ".".to_string());

    if !std::path::Path::new(&watch_path).exists() {
        eprintln!("error: watch path does not exist: {watch_path}");
        std::process::exit(1);
    }

    // Ensure ~/.engos/ exists — prompts on first run, seeds example data.
    config::ensure_config_dir()?;

    let cfg          = config::load_config();
    let reports      = cfg.reports;
    let orchestrators = config::load_models();

    tui::install_panic_hook();

    let mut terminal = tui::enter()?;
    let result       = tui::run(&mut terminal, &watch_path, reports, orchestrators);
    tui::exit(&mut terminal)?;

    result
}

/// Verify API connectivity for every configured Anthropic orchestrator.
///
/// Runs entirely in the normal shell — no TUI, no alternate screen. Prints
/// a line-by-line status report so the operator can diagnose key or model
/// issues before starting an engagement.
fn check_api() {
    println!("engos {} — API connectivity check", env!("CARGO_PKG_VERSION"));
    println!();

    // Load orchestrators from ~/.engos/models.yml. If the file is absent the
    // config dir hasn't been created yet — tell the operator what to do.
    let orchestrators = config::load_models();

    let anthropic: Vec<_> = orchestrators
        .iter()
        .filter(|o| o.vendor == "anthropic")
        .collect();

    if anthropic.is_empty() {
        println!("  No Anthropic models found in ~/.engos/models.yml");
        println!("  Add one via  File › New Report › [ New ]  inside engos.");
        return;
    }

    for orch in &anthropic {
        println!("  ┌ {}", orch.name);

        // Derive the bare model ID the API expects from the display name.
        let model_id = anthropic::model_id(&orch.name);
        println!("  │ model id : {model_id}");

        // Check whether a key is configured — we do not print the key itself.
        let Some(ref key) = orch.api_key else {
            println!("  │ api key  : not configured");
            println!("  └ SKIP — add a key via the New Model form\n");
            continue;
        };
        if key.is_empty() {
            println!("  │ api key  : empty string");
            println!("  └ SKIP — re-enter the key via the New Model form\n");
            continue;
        }

        println!("  │ api key  : configured ({} chars)", key.len());
        print!(  "  │ testing  : ");
        // Flush so the "testing" line appears before the network call blocks.
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match anthropic::check_connection(model_id, key) {
            Ok(hint)  => println!("✓  OK  ({hint})"),
            Err(msg)  => println!("✗  FAIL\n  │            {msg}"),
        }
        println!("  └");
        println!();
    }
}
