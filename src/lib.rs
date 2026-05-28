//! `engos` — Engagement OS library crate.
//!
//! All application logic lives here so it can be imported by integration tests
//! in `tests/`. The binary in `src/main.rs` is a thin entry point that wires
//! the pieces together; it contains no logic worth testing on its own.
//!
//! Modules are added one per build phase. Uncomment each as it is implemented.

/// Filesystem watcher — translates raw OS events into typed [`watcher::WatchEvent`]s.
pub mod watcher;

/// Terminal UI — setup, teardown, panic safety, and the render loop.
pub mod tui;

/// Menu bar and dropdown — data model, key handling, and rendering.
pub mod menu;

/// Help screen — navigable topics list and content pane.
pub mod help;

/// Visual theme — colour palette and style constructors.
pub mod theme;

/// Configuration directory management and project data loading.
pub mod config;

/// Project data model, navigation state, and rendering.
pub mod project;

/// Orchestrator model definitions loaded from `models.yml`.
pub mod models;

/// New Project form — state, key handling, and rendering.
pub mod newproject;

/// New Orchestrator form — configure and save a new LLM backend.
pub mod neworch;

/// Capabilities screen — project feature selection and local file writing.
pub mod capabilities;

/// Project context menu — Open / Delete popup triggered from the project list.
pub mod projectmenu;

/// Game-style main menu — centred four-item landing screen.
pub mod mainmenu;

/// Project workspace — active engagement screen (project name placeholder for now).
pub mod workspace;

/// Options screen — appearance settings and future preferences.
pub mod options;

// Phase 3+: uncomment as each module is built.
// pub mod ingest;   // artifact ingestion (text + image parsing)
// pub mod model;    // ModelBackend trait + AnthropicBackend
// pub mod pipeline; // timeline extraction, report generation
// pub mod config;   // config.toml parsing
