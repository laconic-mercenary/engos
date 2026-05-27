//! Terminal UI — setup, teardown, panic safety, and the render/event loop.
//!
//! All mutable session state lives in `AppState`. The loop is:
//!   render(state) → wait for key → handle_key(state, key) → state → repeat.

use crate::capabilities::{self, CapabilitiesAction, CapabilitiesState};
use crate::projectmenu::{self, ProjectMenuAction, ProjectMenuState};
use crate::config;
use crate::help::{self, HelpState};
use crate::menu::{self, MenuAction, MenuState};
use crate::models::{ModelConfig, Orchestrator};
use crate::neworch::{self, NewOrchAction, NewOrchState};
use crate::newproject::{self, NewProjectAction, NewProjectState, OrchestratorSelection};
use crate::project::{self, Project, ProjectState};
use crate::theme;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
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
    Main,
    ProjectMenu,
    NewProject,
    NewOrchestrator,
    Capabilities,
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
    pub menu:          MenuState,
    pub help:          HelpState,
    /// Navigation cursor for the project list on the main screen.
    pub proj:          ProjectState,
    /// The live project list — grows when new projects are created.
    pub projects:      Vec<Project>,
    pub new_proj:      NewProjectState,
    pub new_orch:      NewOrchState,
    pub capabilities:  CapabilitiesState,
    /// State for the project context menu (Open / Delete).
    pub project_menu:  ProjectMenuState,
    /// The live orchestrator list — grows when new orchestrators are created.
    pub orchestrators: Vec<Orchestrator>,
}

pub fn initial_state(projects: Vec<Project>, orchestrators: Vec<Orchestrator>) -> AppState {
    // Compute the nav cursor from the project list before moving it into AppState.
    let proj = ProjectState::init(&projects);
    AppState {
        screen:       Screen::Splash,
        menu:         MenuState::default(),
        help:         HelpState::default(),
        proj,
        projects,
        new_proj:     newproject::new_state(),
        new_orch:     neworch::new_state(),
        capabilities: capabilities::new_state(),
        // project_menu starts on index 0; it is always reset before use.
        project_menu: projectmenu::new_state(0),
        orchestrators,
    }
}

// ── Terminal lifecycle ────────────────────────────────────────────────────────

pub fn enter() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

pub fn exit(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        previous(info);
    }));
}

// ── Event loop ────────────────────────────────────────────────────────────────

pub fn run(
    terminal: &mut Tui,
    watch_path: &str,
    projects: Vec<Project>,
    orchestrators: Vec<Orchestrator>,
) -> io::Result<()> {
    let mut state = initial_state(projects, orchestrators);

    // Draw the initial frame before blocking on input.
    terminal.draw(|frame| render(frame, &state, watch_path))?;

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
                    terminal.draw(|frame| render(frame, &state, watch_path))?;
                }
            }
            // Resize events require a forced redraw so the layout recalculates
            // for the new terminal dimensions.
            Event::Resize(_, _) => {
                terminal.draw(|frame| render(frame, &state, watch_path))?;
            }
            // Mouse, focus, paste, and other events are not yet handled.
            _ => {}
        }
    }

    Ok(())
}

// ── Key handling ──────────────────────────────────────────────────────────────

fn handle_key(state: AppState, key: KeyEvent) -> AppState {
    match state.screen.clone() {
        Screen::Splash          => handle_splash(state, key),
        Screen::Main            => handle_main(state, key),
        Screen::ProjectMenu     => handle_project_menu(state, key),
        Screen::NewProject      => handle_new_project(state, key),
        Screen::NewOrchestrator => handle_new_orch(state, key),
        Screen::Capabilities    => handle_capabilities(state, key),
        Screen::Help            => handle_help(state, key),
        Screen::Quit            => state,
    }
}

fn handle_splash(mut state: AppState, key: KeyEvent) -> AppState {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
            state.screen = Screen::Quit;
        }
        _ => state.screen = Screen::Main,
    }
    state
}

fn handle_main(mut state: AppState, key: KeyEvent) -> AppState {
    if state.menu.focused {
        let (next_menu, action) = menu::handle_key(state.menu, key);
        state.menu = next_menu;

        match action {
            Some(MenuAction::Quit) => state.screen = Screen::Quit,
            Some(MenuAction::NewProject) => {
                state.new_proj = newproject::new_state();
                state.screen   = Screen::NewProject;
            }
            Some(MenuAction::Help(topic)) => {
                state.help   = HelpState { selected: topic };
                state.screen = Screen::Help;
            }
            None => {}
        }
    } else {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                state.screen = Screen::Quit;
            }
            (KeyCode::Char('m'), _) => state.menu = menu::focus(state.menu),
            (KeyCode::Up | KeyCode::Down, _) => {
                state.proj = project::handle_key(state.proj, &state.projects, key);
            }
            // Enter on a highlighted project opens the context menu.
            (KeyCode::Enter, _) => {
                if let Some(idx) = state.proj.selected {
                    state.project_menu = projectmenu::new_state(idx);
                    state.screen       = Screen::ProjectMenu;
                }
            }
            _ => {}
        }
    }
    state
}

fn handle_project_menu(mut state: AppState, key: KeyEvent) -> AppState {
    let (next_menu, action) = projectmenu::handle_key(state.project_menu, key);
    state.project_menu = next_menu;

    match action {
        Some(ProjectMenuAction::Open(_idx)) => {
            // Workspace screen not yet built — close the menu and return to Main.
            state.screen = Screen::Main;
        }
        Some(ProjectMenuAction::Delete(idx)) => {
            if idx < state.projects.len() {
                // Capture the directory path BEFORE removing from the list —
                // once removed the name is gone.
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                let project_dir = format!(
                    "{home}/.engos/projects/{}",
                    state.projects[idx].name
                );

                state.projects.remove(idx);
                config::persist_config(&state.projects);

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
            let new_selected = if state.projects.is_empty() {
                None
            } else if idx >= state.projects.len() {
                // Removed the last item — move cursor to the new last.
                Some(state.projects.len() - 1)
            } else {
                // Item removed from the middle — cursor stays at same index
                // which now points to the next project.
                Some(idx)
            };
            state.proj   = ProjectState { selected: new_selected };
            state.screen = Screen::Main;
        }
        Some(ProjectMenuAction::Close) => {
            state.screen = Screen::Main;
        }
        None => {}
    }

    state
}

fn handle_new_project(mut state: AppState, key: KeyEvent) -> AppState {
    let (next_form, action) = newproject::handle_key(state.new_proj, &state.orchestrators, key);
    state.new_proj = next_form;

    match action {
        Some(NewProjectAction::Cancel) => {
            state.new_proj = newproject::new_state();
            state.screen   = Screen::Main;
        }
        Some(NewProjectAction::OpenNewOrchestrator) => {
            state.new_orch = neworch::new_state();
            state.screen   = Screen::NewOrchestrator;
        }
        Some(NewProjectAction::Next) => {
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
            state.screen = Screen::NewProject;
        }
        Some(NewOrchAction::Cancel) => {
            state.screen = Screen::NewProject;
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
                &state.new_proj.dir_confirmed,
                &state.capabilities,
                &orch_name,
                &orch_vendor,
            );

            // Build the new project entry and append it to the in-memory list.
            let project_name: String = state.new_proj.name_chars.iter().collect();
            let new_project = Project {
                name:                 project_name,
                start_datetime:       current_timestamp(),
                specialist_model:     orch_name,
                artifacts_collected:  0,
                artifacts_synthesized: 0,
                last_opened:          None,
                last_modified:        None,
            };
            state.projects.push(new_project);

            // Select the newly created project in the main screen list.
            state.proj = ProjectState { selected: Some(state.projects.len() - 1) };

            // Persist the updated project list to ~/.engos/config.yml.
            config::persist_config(&state.projects);

            // Reset both creation forms and return to Main.
            state.new_proj    = newproject::new_state();
            state.capabilities = capabilities::new_state();
            state.screen       = Screen::Main;
        }
        Some(CapabilitiesAction::Cancel) => {
            // Return to the New Project form without discarding its state —
            // the operator may want to change their orchestrator or name.
            state.screen = Screen::NewProject;
        }
        None => {}
    }

    state
}

fn handle_help(mut state: AppState, key: KeyEvent) -> AppState {
    let (next_help, action) = help::handle_key(state.help, key);
    state.help = next_help;
    if let Some(help::HelpAction::Close) = action {
        state.screen = Screen::Main;
    }
    state
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Resolve the selected orchestrator's name and vendor from `AppState`.
fn resolve_orchestrator(state: &AppState) -> (String, String) {
    match &state.new_proj.orch_sel {
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

fn render(frame: &mut ratatui::Frame, state: &AppState, watch_path: &str) {
    match &state.screen {
        Screen::Splash => render_splash(frame, watch_path),
        Screen::Main   => render_main(frame, state),
        Screen::ProjectMenu => {
            render_main(frame, state);
            let name = state.projects
                .get(state.project_menu.project_idx)
                .map(|p| p.name.as_str())
                .unwrap_or("Project");
            projectmenu::render(frame, frame.area(), &state.project_menu, name);
        }
        Screen::NewProject => {
            render_main(frame, state);
            newproject::render(frame, frame.area(), &state.new_proj, &state.orchestrators);
        }
        Screen::NewOrchestrator => {
            render_main(frame, state);
            newproject::render(frame, frame.area(), &state.new_proj, &state.orchestrators);
            neworch::render(frame, frame.area(), &state.new_orch);
        }
        Screen::Capabilities => {
            render_main(frame, state);
            capabilities::render(frame, frame.area(), &state.capabilities);
        }
        Screen::Help => help::render(frame, frame.area(), &state.help),
        Screen::Quit => {}
    }
}

fn render_splash(frame: &mut ratatui::Frame, watch_path: &str) {
    let area  = frame.area();
    let box_w = 52_u16.min(area.width);
    let box_h = 14_u16.min(area.height);
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
            Line::from(vec![
                Span::styled("  Watching  ", theme::text_hint()),
                Span::styled(watch_path, theme::text_active()),
            ]),
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
    let area = frame.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)])
        .split(area);

    menu::render_bar(frame, rows[0], &state.menu);
    project::render(frame, rows[1], &state.projects, &state.proj);
    menu::render_dropdown(frame, rows[0], &state.menu);
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
