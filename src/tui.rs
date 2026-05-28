//! Terminal UI — setup, teardown, panic safety, and the render/event loop.
//!
//! All mutable session state lives in `AppState`. The loop is:
//!   render(state) → wait for key → handle_key(state, key) → state → repeat.

use crate::capabilities::{self, CapabilitiesAction, CapabilitiesState};
use crate::mainmenu::{self, MainMenuAction, MainMenuState};
use crate::options::{self, OptionsAction, OptionsState};
use crate::reportmenu::{self, ReportMenuAction, ReportMenuState};
use crate::workspace::{self, ChatMessage, MessageRole, WorkspaceAction, WorkspaceState};
use crate::config;
use crate::help::{self, HelpState};
use crate::models::{ModelConfig, Orchestrator};
use crate::neworch::{self, NewOrchAction, NewOrchState};
use crate::newreport::{self, NewReportAction, NewReportState, OrchestratorSelection};
use crate::report::{self, Report, ReportState};
use crate::theme;
use crossterm::{
    event::{self, EnableBracketedPaste, DisableBracketedPaste, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Terminal,
};
use std::{
    io::{self, Stdout, stdout},
};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

// ── Screens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Splash,
    /// Game-style landing menu (New Report / Existing Report / Options / Exit).
    MainMenu,
    /// Project list screen — reached via "Existing Report" on the main menu.
    Main,
    ReportMenu,
    NewReport,
    NewOrchestrator,
    Capabilities,
    /// Active engagement workspace — shows the project name (full layout in a later phase).
    Workspace,
    /// Application options — appearance palette selection, etc.
    Options,
    Help,
    Quit,
}

// ── Application state ─────────────────────────────────────────────────────────

/// All mutable UI state for a running session.
///
/// Owned data only — passed by value through `handle_key` so each transition
/// produces a fresh state without any hidden mutation.
pub struct AppState {
    pub screen:        Screen,
    pub main_menu:     MainMenuState,
    pub help:          HelpState,
    /// Navigation cursor for the report list on the main screen.
    pub report_nav:    ReportState,
    /// The live report list — grows when new reports are created.
    pub reports:       Vec<Report>,
    pub new_report:      NewReportState,
    pub new_orch:      NewOrchState,
    pub capabilities:  CapabilitiesState,
    /// State for the project context menu (Open / Delete).
    pub report_menu:  ReportMenuState,
    /// The live orchestrator list — grows when new orchestrators are created.
    pub orchestrators: Vec<Orchestrator>,
    /// Active engagement workspace — populated before transitioning to Screen::Workspace.
    pub workspace:     WorkspaceState,
    /// Options screen state — refreshed from the active palette each time Options opens.
    pub options:       OptionsState,
}

pub fn initial_state(reports: Vec<Report>, orchestrators: Vec<Orchestrator>) -> AppState {
    // Compute the nav cursor from the report list before moving it into AppState.
    let report_nav = ReportState::init(&reports);
    AppState {
        screen:       Screen::Splash,
        main_menu:    MainMenuState::default(),
        help:         HelpState::default(),
        report_nav,
        reports,
        new_report:   newreport::new_state(),
        new_orch:     neworch::new_state(),
        capabilities: capabilities::new_state(),
        // report_menu starts on index 0; always reset before use.
        report_menu: reportmenu::new_state(0),
        orchestrators,
        // workspace and options are populated before their screens are opened.
        workspace:    workspace::new_state(""),
        options:      options::new_state(),
    }
}

// ── Terminal lifecycle ────────────────────────────────────────────────────────

pub fn enter() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut out = stdout();
    // EnableBracketedPaste tells the terminal to wrap pasted content in escape
    // sequences so it arrives as Event::Paste(String) rather than individual
    // keystrokes. Terminals that don't support it silently ignore the sequence.
    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

pub fn exit(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableBracketedPaste)?;
    terminal.show_cursor()?;
    Ok(())
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableBracketedPaste);
        previous(info);
    }));
}

// ── Event loop ────────────────────────────────────────────────────────────────

pub fn run(
    terminal: &mut Tui,
    _watch_path: &str, // Phase 3: launch the file watcher from here
    reports: Vec<Report>,
    orchestrators: Vec<Orchestrator>,
) -> io::Result<()> {
    let mut state = initial_state(reports, orchestrators);

    // Draw the initial frame before blocking on input.
    terminal.draw(|frame| render(frame, &state))?;

    while !matches!(state.screen, Screen::Quit) {
        // Block until a terminal event arrives — no timeout, near-zero CPU idle.
        //
        // The previous approach (event::poll with a 16ms timeout) caused
        // terminal.draw to fire ~60 times per second even when nothing changed,
        // burning CPU continuously. A keyboard-only TUI has no reason to redraw
        // faster than the operator's keystroke rate.
        //
        // When the file watcher is wired in (Phase 3) this becomes a
        // tokio::select! on both the input stream and the watcher channel so
        // file events also trigger a redraw.
        match event::read()? {
            Event::Key(key) => {
                state = handle_key(state, key);
                // Only redraw after state actually changes — skip the draw
                // if we're about to exit anyway.
                if !matches!(state.screen, Screen::Quit) {
                    terminal.draw(|frame| render(frame, &state))?;
                }
            }
            // Bracketed paste — only processed when the workspace is open.
            // Other screens ignore the event so paste text doesn't leak into
            // menus or forms unexpectedly.
            Event::Paste(text) => {
                state = handle_paste(state, text);
                terminal.draw(|frame| render(frame, &state))?;
            }
            // Resize events require a forced redraw so the layout recalculates
            // for the new terminal dimensions.
            Event::Resize(_, _) => {
                terminal.draw(|frame| render(frame, &state))?;
            }
            _ => {}
        }
    }

    Ok(())
}

// ── Key handling ──────────────────────────────────────────────────────────────

fn handle_key(state: AppState, key: KeyEvent) -> AppState {
    // Global hard-quit: Ctrl+C exits from any screen without going through menus.
    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
        let mut s = state;
        s.screen = Screen::Quit;
        return s;
    }

    match state.screen.clone() {
        Screen::Splash          => handle_splash(state, key),
        Screen::MainMenu        => handle_main_menu(state, key),
        Screen::Main            => handle_main(state, key),
        Screen::ReportMenu     => handle_project_menu(state, key),
        Screen::NewReport      => handle_new_report(state, key),
        Screen::NewOrchestrator => handle_new_orch(state, key),
        Screen::Capabilities    => handle_capabilities(state, key),
        Screen::Workspace       => handle_workspace(state, key),
        Screen::Options         => handle_options(state, key),
        Screen::Help            => handle_help(state, key),
        Screen::Quit            => state,
    }
}

fn handle_splash(mut state: AppState, key: KeyEvent) -> AppState {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
            state.screen = Screen::Quit;
        }
        _ => state.screen = Screen::MainMenu,
    }
    state
}

fn handle_main_menu(mut state: AppState, key: KeyEvent) -> AppState {
    let (next, action) = mainmenu::handle_key(state.main_menu, key);
    state.main_menu = next;

    match action {
        Some(MainMenuAction::NewReport) => {
            state.new_report = newreport::new_state();
            state.screen   = Screen::NewReport;
        }
        Some(MainMenuAction::ExistingReport) => {
            // Reset the project list cursor before entering so it is always
            // in a valid position regardless of deletions in a prior session.
            state.report_nav = report::ReportState::init(&state.reports);
            state.screen = Screen::Main;
        }
        Some(MainMenuAction::Options) => {
            // Refresh options state from the current palette before opening.
            state.options = options::new_state();
            state.screen  = Screen::Options;
        }
        Some(MainMenuAction::Help) => {
            state.help   = HelpState::default();
            state.screen = Screen::Help;
        }
        Some(MainMenuAction::Quit) => state.screen = Screen::Quit,
        None => {}
    }

    state
}

fn handle_workspace(mut state: AppState, key: KeyEvent) -> AppState {
    let (next, action) = workspace::handle_key(state.workspace, key);
    state.workspace = next;
    if let Some(WorkspaceAction::Close) = action {
        state.screen = Screen::MainMenu;
    }
    state
}

/// Handle a bracketed-paste event.
///
/// Only active when the workspace is open. The pasted content becomes a new
/// artifact: the counter increments, a log entry is written, and a system
/// message appears in the chat history so the operator has visual confirmation.
/// Any other screen silently drops the event.
fn handle_paste(mut state: AppState, text: String) -> AppState {
    if matches!(state.screen, Screen::Workspace) {
        let char_count = text.chars().count();
        state.workspace.artifacts_processed += 1;
        state.workspace.logs.push(format!("paste artifact  {char_count} chars"));
        state.workspace.chat_history.push(ChatMessage {
            role: MessageRole::Model,
            text: format!("paste artifact received \u{2014} {char_count} chars"),
        });
        // Bring the confirmation message into view.
        state.workspace.chat_scroll = 0;
    }
    state
}

fn handle_options(mut state: AppState, key: KeyEvent) -> AppState {
    let (next, action) = options::handle_key(state.options, key);
    state.options = next;
    if let Some(OptionsAction::Close) = action {
        state.screen = Screen::MainMenu;
    }
    state
}

fn handle_main(mut state: AppState, key: KeyEvent) -> AppState {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => state.screen = Screen::Quit,
        // Esc returns to the main menu from the project list.
        (KeyCode::Esc, _) => state.screen = Screen::MainMenu,
        (KeyCode::Up | KeyCode::Down, _) => {
            state.report_nav = report::handle_key(state.report_nav, &state.reports, key);
        }
        // Enter on a highlighted project opens the context menu.
        (KeyCode::Enter, _) => {
            if let Some(idx) = state.report_nav.selected {
                state.report_menu = reportmenu::new_state(idx);
                state.screen       = Screen::ReportMenu;
            }
        }
        _ => {}
    }
    state
}

fn handle_project_menu(mut state: AppState, key: KeyEvent) -> AppState {
    let (next_menu, action) = reportmenu::handle_key(state.report_menu, key);
    state.report_menu = next_menu;

    match action {
        Some(ReportMenuAction::Open(idx)) => {
            let name = state.reports
                .get(idx)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            state.workspace = workspace::new_state(&name);
            state.screen    = Screen::Workspace;
        }
        Some(ReportMenuAction::Delete(idx)) => {
            if idx < state.reports.len() {
                // Capture the directory path BEFORE removing from the list —
                // once removed the name is gone.
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                let project_dir = format!(
                    "{home}/.engos/reports/{}",
                    state.reports[idx].name
                );

                state.reports.remove(idx);
                config::persist_config(&state.reports);

                // Remove the project directory and all its contents.
                // Failure is logged but does not abort — the project is
                // already removed from the list and config.
                let dir_path = std::path::Path::new(&project_dir);
                if dir_path.exists() {
                    if let Err(e) = std::fs::remove_dir_all(dir_path) {
                        eprintln!(
                            "warning: could not delete project directory \
                             {project_dir}: {e}"
                        );
                    }
                }
            }
            // Update the nav cursor so it stays in bounds after removal.
            let new_selected = if state.reports.is_empty() {
                None
            } else if idx >= state.reports.len() {
                // Removed the last item — move cursor to the new last.
                Some(state.reports.len() - 1)
            } else {
                // Item removed from the middle — cursor stays at same index
                // which now points to the next project.
                Some(idx)
            };
            state.report_nav = ReportState { selected: new_selected };
            state.screen = Screen::Main;
        }
        Some(ReportMenuAction::Close) => {
            state.screen = Screen::Main;
        }
        None => {}
    }

    state
}

fn handle_new_report(mut state: AppState, key: KeyEvent) -> AppState {
    let (next_form, action) = newreport::handle_key(state.new_report, &state.orchestrators, key);
    state.new_report = next_form;

    match action {
        Some(NewReportAction::Cancel) => {
            state.new_report = newreport::new_state();
            state.screen   = Screen::MainMenu;
        }
        Some(NewReportAction::OpenNewOrchestrator) => {
            state.new_orch = neworch::new_state();
            state.screen   = Screen::NewOrchestrator;
        }
        Some(NewReportAction::Next) => {
            // Reset capabilities to defaults before showing the screen.
            state.capabilities = capabilities::new_state();
            state.screen       = Screen::Capabilities;
        }
        None => {}
    }

    state
}

fn handle_new_orch(mut state: AppState, key: KeyEvent) -> AppState {
    let (next_orch, action) = neworch::handle_key(state.new_orch, &state.orchestrators, key);
    state.new_orch = next_orch;

    match action {
        Some(NewOrchAction::Save(orch)) => {
            state.orchestrators.push(orch);
            persist_models(&state.orchestrators);
            state.screen = Screen::NewReport;
        }
        Some(NewOrchAction::Cancel) => {
            state.screen = Screen::NewReport;
        }
        None => {}
    }

    state
}

fn handle_capabilities(mut state: AppState, key: KeyEvent) -> AppState {
    let (next_caps, action) = capabilities::handle_key(state.capabilities, key);
    state.capabilities = next_caps;

    match action {
        Some(CapabilitiesAction::Finish) => {
            // Resolve the selected orchestrator name and vendor.
            let (orch_name, orch_vendor) = resolve_orchestrator(&state);

            // Write project-local config files.
            capabilities::write_project_files(
                &state.new_report.dir_confirmed,
                &state.capabilities,
                &orch_name,
                &orch_vendor,
            );

            // Capture the name before it is moved into the Project struct.
            let project_name: String = state.new_report.name_chars.iter().collect();
            let new_report = Report {
                name:                  project_name.clone(),
                start_datetime:        current_timestamp(),
                specialist_model:      orch_name,
                artifacts_collected:   0,
                artifacts_synthesized: 0,
                last_opened:           None,
                last_modified:         None,
            };
            state.reports.push(new_report);

            // Select the newly created project in the main screen list.
            state.report_nav = ReportState { selected: Some(state.reports.len() - 1) };

            // Persist the updated project list to ~/.engos/config.yml.
            config::persist_config(&state.reports);

            // Open the workspace for the newly created project.
            state.workspace    = workspace::new_state(&project_name);
            state.new_report     = newreport::new_state();
            state.capabilities = capabilities::new_state();
            state.screen       = Screen::Workspace;
        }
        Some(CapabilitiesAction::Cancel) => {
            // Return to the New Project form without discarding its state —
            // the operator may want to change their orchestrator or name.
            state.screen = Screen::NewReport;
        }
        None => {}
    }

    state
}

fn handle_help(mut state: AppState, key: KeyEvent) -> AppState {
    let (next_help, action) = help::handle_key(state.help, key);
    state.help = next_help;
    if let Some(help::HelpAction::Close) = action {
        state.screen = Screen::MainMenu;
    }
    state
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Resolve the selected orchestrator's name and vendor from `AppState`.
fn resolve_orchestrator(state: &AppState) -> (String, String) {
    match &state.new_report.orch_sel {
        OrchestratorSelection::Selected(i) => {
            if let Some(o) = state.orchestrators.get(*i) {
                return (o.name.clone(), o.vendor.clone());
            }
        }
        _ => {}
    }
    ("unknown".to_string(), "unknown".to_string())
}

/// Return the current UTC time as an ISO 8601 string: `2026-05-27T14:30:00Z`.
///
/// Uses only `std::time` — no `chrono` dependency. Calendar arithmetic uses
/// Howard Hinnant's civil_from_days algorithm, which correctly handles all
/// Gregorian leap years without special-casing.
fn current_timestamp() -> String {
    let total_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Time-of-day components (UTC).
    let h  = (total_secs % 86_400) / 3_600;
    let mi = (total_secs %  3_600) /    60;
    let s  =  total_secs %     60;

    // Calendar date via Hinnant's civil_from_days.
    let (yr, mo, dy) = unix_days_to_ymd((total_secs / 86_400) as i64);

    format!("{yr:04}-{mo:02}-{dy:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert days since the Unix epoch (1970-01-01) to (year, month, day) UTC.
///
/// Algorithm: Howard Hinnant's civil_from_days
/// <https://howardhinnant.github.io/date_algorithms.html>
fn unix_days_to_ymd(days: i64) -> (i64, u64, u64) {
    // Shift epoch to 1 Mar, year 0 (a purely mathematical convenience).
    let z   = days + 719_468_i64;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64; // day-of-era [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // year-of-era
    let y   = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day-of-year [0, 365]
    let mp  = (5 * doy + 2) / 153;        // month-of-year, Mar-based [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1;
    let mon = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr  = if mon <= 2 { y + 1 } else { y };
    (yr, mon, day)
}

// ── Persistence helpers ───────────────────────────────────────────────────────

fn persist_models(orchestrators: &[Orchestrator]) {
    let Some(path) = config::models_path() else {
        eprintln!("warning: could not determine models path; orchestrator not saved");
        return;
    };
    let cfg = ModelConfig { orchestrators: orchestrators.to_vec() };
    match serde_yml::to_string(&cfg) {
        Ok(yaml) => {
            if let Err(e) = std::fs::write(&path, yaml) {
                eprintln!("warning: could not write {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("warning: could not serialise models: {e}"),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render(frame: &mut ratatui::Frame, state: &AppState) {
    match &state.screen {
        Screen::Splash   => render_splash(frame),
        Screen::MainMenu => mainmenu::render(frame, frame.area(), &state.main_menu),
        Screen::Main     => render_main(frame, state),
        Screen::ReportMenu => {
            render_main(frame, state);
            let name = state.reports
                .get(state.report_menu.report_idx)
                .map(|p| p.name.as_str())
                .unwrap_or("Report");
            reportmenu::render(frame, frame.area(), &state.report_menu, name);
        }
        Screen::NewReport => {
            render_main(frame, state);
            newreport::render(frame, frame.area(), &state.new_report, &state.orchestrators);
        }
        Screen::NewOrchestrator => {
            render_main(frame, state);
            newreport::render(frame, frame.area(), &state.new_report, &state.orchestrators);
            neworch::render(frame, frame.area(), &state.new_orch);
        }
        Screen::Capabilities => {
            render_main(frame, state);
            capabilities::render(frame, frame.area(), &state.capabilities);
        }
        Screen::Workspace => workspace::render(frame, frame.area(), &state.workspace),
        Screen::Options   => options::render(frame, frame.area(), &state.options),
        Screen::Help => help::render(frame, frame.area(), &state.help),
        Screen::Quit => {}
    }
}

fn render_splash(frame: &mut ratatui::Frame) {
    let area  = frame.area();
    let box_w = 52_u16.min(area.width);
    let box_h = 12_u16.min(area.height);
    let sa    = centered_rect(box_w, box_h, area);

    frame.render_widget(Clear, sa);

    let version = env!("CARGO_PKG_VERSION");
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("  engos", theme::text_active())),
            Line::from(Span::styled(format!("  Engagement OS  v{version}"), theme::text_hint())),
            Line::from(""),
            Line::from(Span::styled("  Red team reporting assistant", theme::text())),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "  press any key to begin  ·  q to quit",
                theme::text_hint(),
            )),
            Line::from(""),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border_splash()),
        ),
        sa,
    );
}

fn render_main(frame: &mut ratatui::Frame, state: &AppState) {
    report::render(frame, frame.area(), &state.reports, &state.report_nav);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(height), Constraint::Fill(1)])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(width), Constraint::Fill(1)])
        .split(v[1])[1]
}
