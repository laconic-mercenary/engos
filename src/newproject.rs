//! New Project form — state, key handling, and rendering.
//!
//! Collects: project name and specialist orchestrator.
//! The project directory is derived automatically as `~/.engos/projects/<name>`
//! and is not exposed in the UI.
//!
//! Navigation: Tab / Shift+Tab cycle between fields. Enter confirms or activates.
//! Esc cancels from anywhere.
//!
//! # Name rules
//! Only alphanumeric characters, underscores, and dashes are accepted.
//! Whitespace and other characters are silently rejected at the input level.

use crate::models::Orchestrator;
use crate::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ── Focus enum ────────────────────────────────────────────────────────────────

/// Tab order: Name → Orchestrator → NewOrchestrator → Cancel → Next → Name
#[derive(Debug, Clone, PartialEq)]
pub enum NewProjectFocus {
    Name,
    Orchestrator,
    /// The `[ + New Orchestrator ]` button below the orchestrator dropdown.
    NewOrchestrator,
    Cancel,
    /// Proceed to the Capabilities screen. Only activates when the form is valid.
    Next,
}

// ── Orchestrator selection ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum OrchestratorSelection {
    Unset,
    Selected(usize),
    /// Placeholder — kept for future use but not exposed in UI.
    New,
}

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NewProjectState {
    // ── Name input ────────────────────────────────────────────────────────────
    pub name_chars:  Vec<char>,
    pub name_cursor: usize,

    // ── Orchestrator dropdown ─────────────────────────────────────────────────
    pub orch_sel:   OrchestratorSelection,
    pub orch_open:  bool,
    /// Highlighted row in the open dropdown (0..orchestrators.len()).
    pub orch_hover: usize,

    // ── Derived project directory (never shown, always auto-computed) ─────────
    /// `~/.engos/projects/<name>` — updated on every name keystroke.
    /// Consumers (capabilities screen, file writing) read this field.
    pub dir_confirmed: String,

    pub focus: NewProjectFocus,
}

pub fn new_state() -> NewProjectState {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    NewProjectState {
        name_chars:    Vec::new(),
        name_cursor:   0,
        orch_sel:      OrchestratorSelection::Unset,
        orch_open:     false,
        orch_hover:    0,
        dir_confirmed: format!("{home}/.engos/projects/"),
        focus:         NewProjectFocus::Name,
    }
}

// ── Actions ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum NewProjectAction {
    Cancel,
    OpenNewOrchestrator,
    Next,
}

/// True when the form has enough information to proceed.
///
/// Required: a non-empty name and a confirmed orchestrator selection.
pub fn is_valid(state: &NewProjectState) -> bool {
    !state.name_chars.is_empty()
        && matches!(state.orch_sel, OrchestratorSelection::Selected(_))
}

// ── Key handling ──────────────────────────────────────────────────────────────

pub fn handle_key(
    state: NewProjectState,
    orchestrators: &[Orchestrator],
    key: KeyEvent,
) -> (NewProjectState, Option<NewProjectAction>) {
    if state.orch_open {
        return handle_orch_dropdown(state, orchestrators, key);
    }

    match state.focus.clone() {
        NewProjectFocus::Name            => handle_name(state, key),
        NewProjectFocus::Orchestrator    => handle_orch_closed(state, key),
        NewProjectFocus::NewOrchestrator => handle_new_orch_btn(state, key),
        NewProjectFocus::Cancel          => handle_cancel(state, key),
        NewProjectFocus::Next            => handle_next(state, key),
    }
}

fn handle_name(
    mut state: NewProjectState,
    key: KeyEvent,
) -> (NewProjectState, Option<NewProjectAction>) {
    match key.code {
        KeyCode::Char(c) => {
            // Only alphanumeric, underscore, dash — safe as a directory name component.
            if c.is_alphanumeric() || c == '_' || c == '-' {
                state.name_chars.insert(state.name_cursor, c);
                state.name_cursor += 1;
                sync_dir(&mut state);
            }
            (state, None)
        }
        KeyCode::Backspace => {
            if state.name_cursor > 0 {
                state.name_chars.remove(state.name_cursor - 1);
                state.name_cursor -= 1;
                sync_dir(&mut state);
            }
            (state, None)
        }
        KeyCode::Left  => { state.name_cursor = state.name_cursor.saturating_sub(1); (state, None) }
        KeyCode::Right => { if state.name_cursor < state.name_chars.len() { state.name_cursor += 1; } (state, None) }
        KeyCode::Tab | KeyCode::Enter => { state.focus = NewProjectFocus::Orchestrator; (state, None) }
        KeyCode::BackTab => { (state, None) } // first field — no-op
        KeyCode::Esc => (state, Some(NewProjectAction::Cancel)),
        _ => (state, None),
    }
}

fn handle_orch_closed(
    mut state: NewProjectState,
    key: KeyEvent,
) -> (NewProjectState, Option<NewProjectAction>) {
    match key.code {
        KeyCode::Enter | KeyCode::Down => {
            state.orch_open  = true;
            state.orch_hover = match &state.orch_sel {
                OrchestratorSelection::Selected(i) => *i,
                _ => 0,
            };
            (state, None)
        }
        KeyCode::Tab     => { state.focus = NewProjectFocus::NewOrchestrator; (state, None) }
        KeyCode::BackTab => { state.focus = NewProjectFocus::Name;            (state, None) }
        KeyCode::Esc     => (state, Some(NewProjectAction::Cancel)),
        _ => (state, None),
    }
}

fn handle_orch_dropdown(
    mut state: NewProjectState,
    orchestrators: &[Orchestrator],
    key: KeyEvent,
) -> (NewProjectState, Option<NewProjectAction>) {
    let total = orchestrators.len();

    if total == 0 {
        if key.code == KeyCode::Esc { state.orch_open = false; }
        return (state, None);
    }

    match key.code {
        KeyCode::Down => { state.orch_hover = (state.orch_hover + 1) % total; (state, None) }
        KeyCode::Up   => { state.orch_hover = state.orch_hover.checked_sub(1).unwrap_or(total - 1); (state, None) }
        // Space and Enter both confirm the selection.
        KeyCode::Enter | KeyCode::Char(' ') => {
            state.orch_sel  = OrchestratorSelection::Selected(state.orch_hover);
            state.orch_open = false;
            // Advance focus past the dropdown after a successful selection.
            state.focus = NewProjectFocus::NewOrchestrator;
            (state, None)
        }
        KeyCode::Esc => { state.orch_open = false; (state, None) }
        _ => (state, None),
    }
}

fn handle_new_orch_btn(
    mut state: NewProjectState,
    key: KeyEvent,
) -> (NewProjectState, Option<NewProjectAction>) {
    match key.code {
        KeyCode::Enter   => (state, Some(NewProjectAction::OpenNewOrchestrator)),
        KeyCode::Tab     => { state.focus = NewProjectFocus::Cancel;          (state, None) }
        KeyCode::BackTab => { state.focus = NewProjectFocus::Orchestrator;    (state, None) }
        KeyCode::Esc     => (state, Some(NewProjectAction::Cancel)),
        _ => (state, None),
    }
}

fn handle_cancel(
    mut state: NewProjectState,
    key: KeyEvent,
) -> (NewProjectState, Option<NewProjectAction>) {
    match key.code {
        KeyCode::Enter   => (state, Some(NewProjectAction::Cancel)),
        KeyCode::Tab     => { state.focus = NewProjectFocus::Next;            (state, None) }
        KeyCode::BackTab => { state.focus = NewProjectFocus::NewOrchestrator; (state, None) }
        KeyCode::Esc     => (state, Some(NewProjectAction::Cancel)),
        _ => (state, None),
    }
}

fn handle_next(
    mut state: NewProjectState,
    key: KeyEvent,
) -> (NewProjectState, Option<NewProjectAction>) {
    match key.code {
        KeyCode::Enter => {
            // Only fire when the form is complete — Enter on a disabled Next is ignored.
            if is_valid(&state) { (state, Some(NewProjectAction::Next)) } else { (state, None) }
        }
        KeyCode::Tab     => { state.focus = NewProjectFocus::Name;   (state, None) }
        KeyCode::BackTab => { state.focus = NewProjectFocus::Cancel; (state, None) }
        KeyCode::Esc     => (state, Some(NewProjectAction::Cancel)),
        _ => (state, None),
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Keep `dir_confirmed` in sync with the name field.
///
/// This is the only place `dir_confirmed` is updated — the operator never
/// sees or edits it directly. The value is consumed by the Capabilities screen
/// when writing `local-config.yml` and `local-models.yml`.
fn sync_dir(state: &mut NewProjectState) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let name: String = state.name_chars.iter().collect();
    state.dir_confirmed = format!("{home}/.engos/projects/{name}");
}

// ── Rendering ─────────────────────────────────────────────────────────────────

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &NewProjectState,
    orchestrators: &[Orchestrator],
) {
    let form_w = 68_u16.min(area.width);
    let form_h = 18_u16.min(area.height); // shorter now that directory row is gone
    let form_area = centered_rect(form_w, form_h, area);

    frame.render_widget(Clear, form_area);
    frame.render_widget(
        Block::default()
            .title(Span::styled(" NEW PROJECT ", theme::text_active()))
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
            Constraint::Length(1), // [2]  Name label
            Constraint::Length(3), // [3]  Name input
            Constraint::Length(1), // [4]  padding
            Constraint::Length(1), // [5]  Orchestrator label
            Constraint::Length(3), // [6]  Orchestrator field
            Constraint::Length(1), // [7]  [ + New Orchestrator ] button
            Constraint::Fill(1),   // [8]  flexible space
            Constraint::Length(1), // [9]  Cancel + Next buttons
            Constraint::Length(1), // [10] bottom padding
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "  Use Tab to switch between inputs",
            theme::text_hint(),
        )),
        rows[0],
    );

    render_name_field(frame, rows[2], rows[3], state);
    render_orch_field(frame, rows[5], rows[6], state, orchestrators);
    render_new_orch_btn(frame, rows[7], state);
    render_bottom_buttons(frame, rows[9], state);

    // Dropdown overlay — rendered last so it floats above everything.
    if state.orch_open {
        render_orch_dropdown(frame, rows[6], state, orchestrators);
    }
}

fn render_name_field(frame: &mut Frame, label: Rect, input: Rect, state: &NewProjectState) {
    frame.render_widget(
        Paragraph::new(Span::styled("  Name", theme::text_hint())),
        label,
    );

    let focused = state.focus == NewProjectFocus::Name;
    let before: String = state.name_chars[..state.name_cursor].iter().collect();
    let after:  String = state.name_chars[state.name_cursor..].iter().collect();

    let line = if focused {
        Line::from(vec![
            Span::styled(format!(" {before}"), theme::text()),
            Span::styled("│", theme::text_active()),
            Span::styled(after, theme::text()),
        ])
    } else {
        let text: String = state.name_chars.iter().collect();
        Line::from(Span::styled(format!(" {text}"), theme::text()))
    };

    frame.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if focused { theme::border_active() } else { theme::border() }),
        ),
        input,
    );
}

fn render_orch_field(
    frame: &mut Frame,
    label: Rect,
    field: Rect,
    state: &NewProjectState,
    orchestrators: &[Orchestrator],
) {
    frame.render_widget(
        Paragraph::new(Span::styled("  Model", theme::text_hint())),
        label,
    );

    let focused = state.focus == NewProjectFocus::Orchestrator || state.orch_open;
    let (text, style) = match &state.orch_sel {
        OrchestratorSelection::Unset => (
            " Please select a model  ▼".to_string(),
            theme::text_hint(),
        ),
        OrchestratorSelection::Selected(i) => {
            let name = orchestrators.get(*i).map(|o| o.name.as_str()).unwrap_or("?");
            (format!(" {name}  ▼"), theme::text())
        }
        OrchestratorSelection::New => (" New  ▼".to_string(), theme::text_hint()),
    };

    frame.render_widget(
        Paragraph::new(Span::styled(text, style)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if focused { theme::border_active() } else { theme::border() }),
        ),
        field,
    );
}

fn render_new_orch_btn(frame: &mut Frame, area: Rect, state: &NewProjectState) {
    let focused = state.focus == NewProjectFocus::NewOrchestrator;
    frame.render_widget(
        Paragraph::new(Span::styled(
            "  [ New ]",
            if focused { theme::selected() } else { theme::text_hint() },
        )),
        area,
    );
}

/// Render `[ Cancel ]` and `[ Next ]` side by side.
///
/// `[ Next ]` is styled active only when the form is valid — dim otherwise so
/// the operator knows something is still missing before they can proceed.
fn render_bottom_buttons(frame: &mut Frame, area: Rect, state: &NewProjectState) {
    let valid = is_valid(state);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(10), // [ Cancel ]
            Constraint::Length(2),  // gap
            Constraint::Length(8),  // [ Next ]
            Constraint::Length(2),  // right margin
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "[ Cancel ]",
            if state.focus == NewProjectFocus::Cancel { theme::selected() } else { theme::text_hint() },
        )),
        cols[1],
    );

    let next_style = match (state.focus == NewProjectFocus::Next, valid) {
        (true, true)  => theme::selected(),
        (false, true) => theme::text_active(),
        _             => theme::text_hint(), // dim when invalid regardless of focus
    };
    frame.render_widget(
        Paragraph::new(Span::styled("[ Next ]", next_style)),
        cols[3],
    );
}

fn render_orch_dropdown(
    frame: &mut Frame,
    field_area: Rect,
    state: &NewProjectState,
    orchestrators: &[Orchestrator],
) {
    if orchestrators.is_empty() {
        return;
    }

    let height = (orchestrators.len() as u16 + 2).min(
        frame.area().height.saturating_sub(field_area.y + field_area.height),
    );
    if height < 3 {
        return;
    }

    let dropdown_area = Rect {
        x: field_area.x,
        y: field_area.y + field_area.height,
        width: field_area.width,
        height,
    };

    frame.render_widget(Clear, dropdown_area);

    let lines: Vec<Line> = orchestrators
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let style = if i == state.orch_hover { theme::selected() } else { theme::text() };
            Line::from(Span::styled(format!("  {}", o.name), style))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border_active()),
        ),
        dropdown_area,
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
