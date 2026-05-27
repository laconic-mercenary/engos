//! `engos` binary entry point.
//!
//! Handles CLI flags, ensures the config directory exists, loads project data,
//! installs the panic hook, then hands off to the TUI.
//! Nothing else lives here — logic belongs in library modules.

use engos::{config, tui};

fn main() -> std::io::Result<()> {
    // Handle meta-flags before touching the terminal or the filesystem so
    // --version and --help print to the normal shell without side effects.
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-V") => {
            println!("engos {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help") | Some("-h") => {
            eprintln!("Usage: engos [path]");
            eprintln!("  path   directory to watch (default: current directory)");
            eprintln!("  q      quit");
            return Ok(());
        }
        _ => {}
    }

    // Resolve the watch path before any terminal work so a missing path shows
    // an error in the normal shell, not on a blank alternate screen.
    let watch_path = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .unwrap_or_else(|| ".".to_string());

    if !std::path::Path::new(&watch_path).exists() {
        eprintln!("error: watch path does not exist: {watch_path}");
        std::process::exit(1);
    }

    // Ensure ~/.engos/ exists. This prompts the operator if the directory is
    // absent and seeds example data on first creation. Runs before the TUI
    // enters alternate-screen mode so the prompt appears in the normal shell.
    config::ensure_config_dir()?;

    // Load ~/.engos/config.yml. Returns a default (empty) Config if the file
    // is absent or the operator declined to create the config dir — the TUI
    // handles an empty project list gracefully.
    let cfg          = config::load_config();
    let projects     = cfg.projects;
    let orchestrators = config::load_models();

    // Install the panic hook so any panic restores the terminal before printing.
    tui::install_panic_hook();

    // Enter TUI mode: raw input + alternate screen.
    let mut terminal = tui::enter()?;

    // Run the render/event loop. Returns Ok(()) on clean exit, Err on I/O failure.
    // orchestrators is passed by value — AppState owns the list so new entries
    // created during the session can be appended and persisted.
    // Both lists are passed by value — AppState owns them so new entries created
    // during the session can be appended and persisted.
    let result = tui::run(&mut terminal, &watch_path, projects, orchestrators);

    // Always restore the terminal, even if run() returned an error.
    tui::exit(&mut terminal)?;

    result
}
