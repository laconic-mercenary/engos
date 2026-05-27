//! Capabilities screen — lets the operator choose which features are active
//! for the new project, then writes the project's local config files.
//!
//! # Layout
//!
//!   Orchestrator Chat      [●]  (required — always on)
//!   ╭─ Engagement Artifacts ──────────────────────────╮
//!   │  Directory Monitoring  (?)   [●]                │
//!   │  Copy and Paste  (?)         [ ]                │
//!   ╰─────────────────────────────────────────────────╯
//!                         [ Cancel ]   [ Finish ]
//!
//! Space / Enter toggles the focused capability.
//! Tab / Shift+Tab cycle through the focusable items.

use crate::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

// ── State ─────────────────────────────────────────────────────────────────────

/// Focus order: DirMonitoring → CopyPaste → Finish → Cancel → DirMonitoring
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilitiesFocus {
    DirMonitoring,
    CopyPaste,
    Finish,
    Cancel,
}

/// All mutable state for the Capabilities form.
#[derive(Debug, Clone)]
pub struct CapabilitiesState {
    /// Always `true` — Orchestrator Chat cannot be disabled for now.
    pub orchestrator_chat: bool,
    /// Whether Directory Monitoring is enabled for this project.
    pub dir_monitoring: bool,
    /// Whether Copy-and-Paste ingestion is enabled for this project.
    pub copy_paste: bool,
    pub focus: CapabilitiesFocus,
}

/// Build a fresh `CapabilitiesState` with defaults.
///
/// Both Engagement Artifact capabilities start OFF — the operator opts in
/// to the features they want rather than opting out.
pub fn new_state() -> CapabilitiesState {
    CapabilitiesState {
        orchestrator_chat: true, // always on
        // Both artifact capabilities start ON — operator opts out rather than in.
        dir_monitoring: true,
        copy_paste: true,
        focus: CapabilitiesFocus::DirMonitoring,
    }
}

/// An action the caller should perform when the form resolves.
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilitiesAction {
    /// Write local config files and create the project.
    Finish,
    /// Discard and return to the New Project form.
    Cancel,
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// Produce the next state and an optional action from a keypress.
pub fn handle_key(
    mut state: CapabilitiesState,
    key: KeyEvent,
) -> (CapabilitiesState, Option<CapabilitiesAction>) {
    match (&state.focus.clone(), key.code) {
        // ── Toggle fields ─────────────────────────────────────────────────────
        (CapabilitiesFocus::DirMonitoring, KeyCode::Char(' ') | KeyCode::Enter) => {
            state.dir_monitoring = !state.dir_monitoring;
            (state, None)
        }
        (CapabilitiesFocus::CopyPaste, KeyCode::Char(' ') | KeyCode::Enter) => {
            state.copy_paste = !state.copy_paste;
            (state, None)
        }

        // ── Button activation ─────────────────────────────────────────────────
        (CapabilitiesFocus::Finish, KeyCode::Enter | KeyCode::Char(' ')) => {
            (state, Some(CapabilitiesAction::Finish))
        }
        (CapabilitiesFocus::Cancel, KeyCode::Enter | KeyCode::Char(' ')) => {
            (state, Some(CapabilitiesAction::Cancel))
        }

        // ── Navigation — Tab/↓ forward, Shift+Tab/↑ backward ────────────────
        (_, KeyCode::Tab | KeyCode::Down) => {
            state.focus = match state.focus {
                CapabilitiesFocus::DirMonitoring => CapabilitiesFocus::CopyPaste,
                CapabilitiesFocus::CopyPaste     => CapabilitiesFocus::Finish,
                CapabilitiesFocus::Finish        => CapabilitiesFocus::Cancel,
                CapabilitiesFocus::Cancel        => CapabilitiesFocus::DirMonitoring,
            };
            (state, None)
        }
        (_, KeyCode::BackTab | KeyCode::Up) => {
            state.focus = match state.focus {
                CapabilitiesFocus::DirMonitoring => CapabilitiesFocus::Cancel,
                CapabilitiesFocus::CopyPaste     => CapabilitiesFocus::DirMonitoring,
                CapabilitiesFocus::Finish        => CapabilitiesFocus::CopyPaste,
                CapabilitiesFocus::Cancel        => CapabilitiesFocus::Finish,
            };
            (state, None)
        }

        // Esc cancels from anywhere.
        (_, KeyCode::Esc) => (state, Some(CapabilitiesAction::Cancel)),

        _ => (state, None),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render the Capabilities form centred on `area`.
pub fn render(frame: &mut Frame, area: Rect, state: &CapabilitiesState) {
    let form_w = 62_u16.min(area.width);
    let form_h = 27_u16.min(area.height); // tall enough for the description pane at full wrap
    let form_area = centered_rect(form_w, form_h, area);

    frame.render_widget(Clear, form_area);
    frame.render_widget(
        Block::default()
            .title(Span::styled(" CAPABILITIES ", theme::text_active()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::border_active()),
        form_area,
    );

    let inner = inner_rect(form_area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0]  info note
            Constraint::Length(1), // [1]  padding
            Constraint::Length(1), // [2]  Orchestrator Chat (always ON)
            Constraint::Length(1), // [3]  padding
            Constraint::Length(6), // [4]  Engagement Artifacts group box
            Constraint::Length(1), // [5]  padding
            Constraint::Length(7), // [6]  Description pane (5 inner rows — enough for wrapped text)
            Constraint::Fill(1),   // [7]  flexible space
            Constraint::Length(1), // [8]  buttons row
            Constraint::Length(1), // [9]  bottom padding
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "  Space to toggle · ↑↓/Tab to switch · Enter to confirm",
            theme::text_hint(),
        )),
        rows[0],
    );

    // Orchestrator Chat — always ON, not focusable.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  [●] Orchestrator Chat", theme::text()),
            Span::styled("  (required)", theme::text_hint()),
        ])),
        rows[2],
    );

    render_artifacts_group(frame, rows[4], state);
    render_description(frame, rows[6], state);
    render_buttons(frame, rows[8], state);
}

/// Render the Engagement Artifacts group box with its two toggles.
fn render_artifacts_group(frame: &mut Frame, area: Rect, state: &CapabilitiesState) {
    let block = Block::default()
        .title(Span::styled(" Engagement Artifacts ", theme::text_active()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border());

    // `Block::inner` gives us the area inside the border.
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Two rows inside the group: one per capability.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),   // top padding
            Constraint::Length(1), // DirMonitoring
            Constraint::Length(1), // CopyPaste
            Constraint::Fill(1),   // bottom padding
        ])
        .split(inner);

    render_toggle(
        frame,
        rows[1],
        "Directory Monitoring",
        state.dir_monitoring,
        state.focus == CapabilitiesFocus::DirMonitoring,
    );
    render_toggle(
        frame,
        rows[2],
        "Copy and Paste",
        state.copy_paste,
        state.focus == CapabilitiesFocus::CopyPaste,
    );
}

/// Render a single toggle row: `[●] Label  (?)` or `[ ] Label  (?)`.
///
/// The `(?)` marker is a placeholder — a future version will show context-
/// sensitive help when the operator focuses the item and presses `?`.
fn render_toggle(frame: &mut Frame, area: Rect, label: &str, on: bool, focused: bool) {
    let indicator = if on { "[●]" } else { "[ ]" };

    // When focused, invert the entire row so the selection stands out.
    let row_style = if focused { theme::selected() } else { theme::text() };

    let line = Line::from(vec![
        Span::styled(format!("  {indicator} {label}"), row_style),
        Span::styled("  (?)", if focused { row_style } else { theme::text_hint() }),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

/// Render the description pane below the Engagement Artifacts group.
///
/// Shows a short explanation of the currently focused capability so the
/// operator knows what enabling/disabling each option actually does.
/// When a button (Finish/Cancel) has focus the pane is empty but still
/// rendered, keeping the layout stable.
fn render_description(frame: &mut Frame, area: Rect, state: &CapabilitiesState) {
    let text = description_text(&state.focus);

    frame.render_widget(
        Paragraph::new(text)
            // Wrap long descriptions at word boundaries within the pane.
            .wrap(Wrap { trim: true })
            .style(theme::text_hint())
            .block(
                Block::default()
                    .title(Span::styled(" Description ", theme::text_hint()))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme::border()),
            ),
        area,
    );
}

/// Return the description string for the currently focused capability.
///
/// Returns an empty string for the button rows — the pane stays visible
/// but blank so the layout does not shift.
fn description_text(focus: &CapabilitiesFocus) -> &'static str {
    match focus {
        CapabilitiesFocus::DirMonitoring => {
            "Watches a designated folder for new files. Terminal captures, \
             scan outputs, screenshots, and tool results are ingested \
             automatically as they are written to disk during the engagement."
        }
        CapabilitiesFocus::CopyPaste => {
            "Creates artifacts from clipboard contents. Supports plain text \
             and images — paste terminal output, screenshots, or notes at \
             any time without saving to a file first."
        }
        // Buttons don't have capability descriptions.
        CapabilitiesFocus::Finish | CapabilitiesFocus::Cancel => "",
    }
}

/// Render `[ Cancel ]` and `[ Finish ]` right-aligned.
fn render_buttons(frame: &mut Frame, area: Rect, state: &CapabilitiesState) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(10), // [ Cancel ]
            Constraint::Length(2),  // gap
            Constraint::Length(10), // [ Finish ]
            Constraint::Length(2),  // right margin
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "[ Cancel ]",
            if state.focus == CapabilitiesFocus::Cancel { theme::selected() } else { theme::text_hint() },
        )),
        cols[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "[ Finish ]",
            if state.focus == CapabilitiesFocus::Finish { theme::selected() } else { theme::text_active() },
        )),
        cols[3],
    );
}

// ── Layout helpers ────────────────────────────────────────────────────────────

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

fn inner_rect(area: Rect) -> Rect {
    Rect {
        x:      area.x + 1,
        y:      area.y + 1,
        width:  area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

// ── File writing ──────────────────────────────────────────────────────────────

/// Write `local-config.yml` and `local-models.yml` into the project directory.
///
/// Creates the directory if it does not yet exist. Both files are always
/// written so the project directory has a complete, self-contained config even
/// if some capabilities are disabled.
pub fn write_project_files(
    project_dir: &str,
    state: &CapabilitiesState,
    orchestrator_name: &str,
    orchestrator_vendor: &str,
) {
    let dir = std::path::Path::new(project_dir);

    // Create the project directory tree. Failure here is logged and we bail —
    // no point writing files into a directory we couldn't create.
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("warning: could not create project directory {project_dir}: {e}");
        return;
    }

    write_local_config(dir, state);
    write_local_models(dir, orchestrator_name, orchestrator_vendor);
}

fn write_local_config(dir: &std::path::Path, state: &CapabilitiesState) {
    let content = format!(
        "# engos project capabilities configuration\n\
         # Edit this file to enable or disable features for this project.\n\
         \n\
         capabilities:\n\
         \n\
         # Orchestrator Chat is always required and cannot be disabled.\n\
           orchestrator_chat: {chat}\n\
         \n\
         # Engagement Artifacts — choose which ingestion methods are active.\n\
           engagement_artifacts:\n\
             directory_monitoring: {dir_mon}\n\
             copy_paste: {cp}\n",
        chat    = state.orchestrator_chat,
        dir_mon = state.dir_monitoring,
        cp      = state.copy_paste,
    );

    let path = dir.join("local-config.yml");
    if let Err(e) = std::fs::write(&path, content) {
        eprintln!("warning: could not write {}: {e}", path.display());
    }
}

fn write_local_models(dir: &std::path::Path, name: &str, vendor: &str) {
    let content = format!(
        "# engos project model configuration\n\
         # The orchestrator selected for this project.\n\
         # To override globally, edit ~/.engos/models.yml instead.\n\
         \n\
         orchestrator:\n\
           name: \"{name}\"\n\
           vendor: {vendor}\n",
    );

    let path = dir.join("local-models.yml");
    if let Err(e) = std::fs::write(&path, content) {
        eprintln!("warning: could not write {}: {e}", path.display());
    }
}
