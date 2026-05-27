//! New Orchestrator form — configure and save a new LLM backend.
//!
//! The form asks for three pieces of information:
//!   1. Vendor  (Anthropic | Local)
//!   2. Name    — display label used in the project dropdown
//!   3. API Key — masked input; only meaningful for cloud vendors
//!
//! Local vendor is stubbed: the fields render but a "coming soon" notice
//! replaces the API Key input so the form is consistent without being misleading.

use crate::models::Orchestrator;
use crate::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ── Vendor enum ───────────────────────────────────────────────────────────────

/// The two supported vendor types.
///
/// Displayed as human-readable strings in the dropdown — variants map directly
/// to what the user sees and selects.
#[derive(Debug, Clone, PartialEq)]
pub enum OrchVendor {
    Anthropic,
    Local,
}

impl OrchVendor {
    /// The label shown in the dropdown list.
    pub fn label(&self) -> &'static str {
        match self {
            OrchVendor::Anthropic => "Anthropic",
            OrchVendor::Local     => "Local",
        }
    }

    /// A suggested default display name for a newly-created orchestrator of
    /// this vendor type.
    fn default_name(&self) -> &'static str {
        match self {
            OrchVendor::Anthropic => "anthropic claude-opus-4-7",
            OrchVendor::Local     => "local-model",
        }
    }

    /// The `vendor` string written to `models.yml`.
    pub fn to_str(&self) -> &'static str {
        match self {
            OrchVendor::Anthropic => "anthropic",
            OrchVendor::Local     => "local",
        }
    }
}

/// All vendor options in display order.
pub const VENDORS: &[OrchVendor] = &[OrchVendor::Anthropic, OrchVendor::Local];

// ── Focus and state ───────────────────────────────────────────────────────────

/// Which field has keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum NewOrchFocus {
    Vendor,
    Name,
    ApiKey,
    Save,
    Cancel,
}

/// All mutable state for the New Orchestrator form.
#[derive(Debug, Clone)]
pub struct NewOrchState {
    // Vendor dropdown
    pub vendor:        OrchVendor,
    pub vendor_open:   bool,
    pub vendor_hover:  usize, // index into VENDORS

    // Name input (display label)
    pub name_chars:   Vec<char>,
    pub name_cursor:  usize,

    // API Key input (masked on screen)
    pub api_key_chars:  Vec<char>,
    pub api_key_cursor: usize,

    /// Validation error shown below the Name field (e.g. duplicate name).
    /// `None` when the name is acceptable.
    pub name_error: Option<String>,

    pub focus: NewOrchFocus,
}

/// Build a fresh state, defaulting to Anthropic with a suggested name.
pub fn new_state() -> NewOrchState {
    let default_name = OrchVendor::Anthropic.default_name();
    NewOrchState {
        vendor:        OrchVendor::Anthropic,
        vendor_open:   false,
        vendor_hover:  0,
        name_chars:    default_name.chars().collect(),
        name_cursor:   default_name.len(),
        api_key_chars:  Vec::new(),
        api_key_cursor: 0,
        name_error: None,
        focus: NewOrchFocus::Vendor,
    }
}

/// An action the caller should perform after the form resolves.
#[derive(Debug, Clone)]
pub enum NewOrchAction {
    /// Operator confirmed — caller should add this orchestrator to the list
    /// and write `models.yml`.
    Save(Orchestrator),
    /// Operator cancelled — discard and return to the previous screen.
    Cancel,
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// Produce the next state and an optional action from a keypress.
///
/// `existing` is the current list of saved orchestrators — used to prevent
/// duplicate names when the operator tries to save.
pub fn handle_key(
    state: NewOrchState,
    existing: &[crate::models::Orchestrator],
    key: KeyEvent,
) -> (NewOrchState, Option<NewOrchAction>) {
    if state.vendor_open {
        return handle_vendor_dropdown(state, key);
    }

    match &state.focus.clone() {
        NewOrchFocus::Vendor  => handle_vendor_closed(state, key),
        NewOrchFocus::Name    => handle_name(state, key),
        NewOrchFocus::ApiKey  => handle_api_key(state, key),
        NewOrchFocus::Save    => handle_save(state, existing, key),
        NewOrchFocus::Cancel  => handle_cancel_btn(state, key),
    }
}

fn handle_vendor_dropdown(
    mut state: NewOrchState,
    key: KeyEvent,
) -> (NewOrchState, Option<NewOrchAction>) {
    match key.code {
        KeyCode::Down  => {
            state.vendor_hover = (state.vendor_hover + 1) % VENDORS.len();
            (state, None)
        }
        KeyCode::Up    => {
            state.vendor_hover =
                state.vendor_hover.checked_sub(1).unwrap_or(VENDORS.len() - 1);
            (state, None)
        }
        // Space and Enter both confirm the selection — spacebar is conventional
        // for dropdown selection in terminal UIs.
        KeyCode::Enter | KeyCode::Char(' ') => {
            // Confirm selection and auto-fill the Name field with the vendor's
            // default suggestion (only if the operator hasn't typed a custom name).
            let selected = VENDORS[state.vendor_hover].clone();
            let current_name: String = state.name_chars.iter().collect();
            // Auto-update only if the name still matches the old vendor default.
            if current_name == state.vendor.default_name() {
                let new_default = selected.default_name();
                state.name_chars  = new_default.chars().collect();
                state.name_cursor = state.name_chars.len();
            }
            state.vendor      = selected;
            state.vendor_open = false;
            state.focus       = NewOrchFocus::Name;
            (state, None)
        }
        KeyCode::Esc => {
            state.vendor_open = false;
            (state, None)
        }
        _ => (state, None),
    }
}

fn handle_vendor_closed(
    mut state: NewOrchState,
    key: KeyEvent,
) -> (NewOrchState, Option<NewOrchAction>) {
    match key.code {
        KeyCode::Enter | KeyCode::Down => {
            state.vendor_hover = VENDORS.iter().position(|v| v == &state.vendor).unwrap_or(0);
            state.vendor_open  = true;
            (state, None)
        }
        KeyCode::Tab    => { state.focus = NewOrchFocus::Name;   (state, None) }
        KeyCode::BackTab => { state.focus = NewOrchFocus::Cancel; (state, None) }
        KeyCode::Esc    => (state, Some(NewOrchAction::Cancel)),
        _ => (state, None),
    }
}

fn handle_name(
    mut state: NewOrchState,
    key: KeyEvent,
) -> (NewOrchState, Option<NewOrchAction>) {
    match key.code {
        KeyCode::Char(c) => {
            state.name_chars.insert(state.name_cursor, c);
            state.name_cursor += 1;
            // Clear any stale duplicate-name error as the operator edits.
            state.name_error = None;
            (state, None)
        }
        KeyCode::Backspace => {
            if state.name_cursor > 0 {
                state.name_chars.remove(state.name_cursor - 1);
                state.name_cursor -= 1;
                state.name_error = None;
            }
            (state, None)
        }
        KeyCode::Left  => { state.name_cursor = state.name_cursor.saturating_sub(1); (state, None) }
        KeyCode::Right => { if state.name_cursor < state.name_chars.len() { state.name_cursor += 1; } (state, None) }
        KeyCode::Tab    => { state.focus = NewOrchFocus::ApiKey;  (state, None) }
        KeyCode::BackTab => { state.focus = NewOrchFocus::Vendor; (state, None) }
        KeyCode::Esc    => (state, Some(NewOrchAction::Cancel)),
        _ => (state, None),
    }
}

fn handle_api_key(
    mut state: NewOrchState,
    key: KeyEvent,
) -> (NewOrchState, Option<NewOrchAction>) {
    // For Local vendor the API Key field is informational only — skip input.
    if state.vendor == OrchVendor::Local {
        return match key.code {
            KeyCode::Tab     => { state.focus = NewOrchFocus::Save;   (state, None) }
            KeyCode::BackTab => { state.focus = NewOrchFocus::Name;   (state, None) }
            KeyCode::Esc     => (state, Some(NewOrchAction::Cancel)),
            _ => (state, None),
        };
    }

    match key.code {
        KeyCode::Char(c) => {
            state.api_key_chars.insert(state.api_key_cursor, c);
            state.api_key_cursor += 1;
            (state, None)
        }
        KeyCode::Backspace => {
            if state.api_key_cursor > 0 {
                state.api_key_chars.remove(state.api_key_cursor - 1);
                state.api_key_cursor -= 1;
            }
            (state, None)
        }
        KeyCode::Left  => { state.api_key_cursor = state.api_key_cursor.saturating_sub(1); (state, None) }
        KeyCode::Right => { if state.api_key_cursor < state.api_key_chars.len() { state.api_key_cursor += 1; } (state, None) }
        KeyCode::Tab     => { state.focus = NewOrchFocus::Save;  (state, None) }
        KeyCode::BackTab => { state.focus = NewOrchFocus::Name;  (state, None) }
        KeyCode::Esc     => (state, Some(NewOrchAction::Cancel)),
        _ => (state, None),
    }
}

fn handle_save(
    mut state: NewOrchState,
    existing: &[crate::models::Orchestrator],
    key: KeyEvent,
) -> (NewOrchState, Option<NewOrchAction>) {
    match key.code {
        KeyCode::Enter => {
            let name: String = state.name_chars.iter().collect();

            // Reject duplicate names — the name is used as the keychain lookup
            // key so duplicates would silently overwrite each other's secrets.
            if existing.iter().any(|o| o.name == name) {
                state.name_error = Some(
                    format!("\"{}\" already exists — choose a different name", name),
                );
                // Return focus to the Name field so the operator can edit it.
                state.focus = NewOrchFocus::Name;
                return (state, None);
            }

            let api_key: String = state.api_key_chars.iter().collect();
            let orch = Orchestrator {
                name,
                vendor:  state.vendor.to_str().to_string(),
                api_key: if api_key.is_empty() { None } else { Some(api_key) },
            };
            (state, Some(NewOrchAction::Save(orch)))
        }
        KeyCode::Tab     => { state.focus = NewOrchFocus::Cancel; (state, None) }
        KeyCode::BackTab => { state.focus = NewOrchFocus::ApiKey; (state, None) }
        KeyCode::Esc     => (state, Some(NewOrchAction::Cancel)),
        _ => (state, None),
    }
}

fn handle_cancel_btn(
    mut state: NewOrchState,
    key: KeyEvent,
) -> (NewOrchState, Option<NewOrchAction>) {
    match key.code {
        KeyCode::Enter   => (state, Some(NewOrchAction::Cancel)),
        KeyCode::Tab     => { state.focus = NewOrchFocus::Vendor; (state, None) }
        KeyCode::BackTab => { state.focus = NewOrchFocus::Save;   (state, None) }
        KeyCode::Esc     => (state, Some(NewOrchAction::Cancel)),
        _ => (state, None),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render the New Orchestrator form centred on `area`.
pub fn render(frame: &mut Frame, area: Rect, state: &NewOrchState) {
    let form_w = 60_u16.min(area.width);
    let form_h = 23_u16.min(area.height); // +1 for the name-error row
    let form_area = centered_rect(form_w, form_h, area);

    frame.render_widget(Clear, form_area);

    frame.render_widget(
        Block::default()
            .title(Span::styled(" NEW ORCHESTRATOR ", theme::text_active()))
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
            Constraint::Length(1), // [2]  Vendor label
            Constraint::Length(3), // [3]  Vendor field
            Constraint::Length(1), // [4]  padding
            Constraint::Length(1), // [5]  Name label
            Constraint::Length(3), // [6]  Name input
            Constraint::Length(1), // [7]  Name error (empty when no error)
            Constraint::Length(1), // [8]  padding
            Constraint::Length(1), // [9]  API Key label
            Constraint::Length(3), // [10] API Key input
            Constraint::Fill(1),   // [11] flexible space
            Constraint::Length(1), // [12] Save + Cancel buttons
            Constraint::Length(1), // [13] bottom padding
        ])
        .split(inner);

    // Info note
    frame.render_widget(
        Paragraph::new(Span::styled(
            "  Use Tab to switch between inputs",
            theme::text_hint(),
        )),
        rows[0],
    );

    render_vendor_field(frame, rows[2], rows[3], state);
    render_name_field(frame, rows[5], rows[6], state);

    // Show the duplicate-name error inline, directly below the Name input.
    if let Some(err) = &state.name_error {
        frame.render_widget(
            Paragraph::new(Span::styled(format!("  ✗ {err}"), theme::error())),
            rows[7],
        );
    }

    render_api_key_field(frame, rows[9], rows[10], state);
    render_buttons(frame, rows[12], state);

    // Dropdown overlay — rendered last to float above everything.
    if state.vendor_open {
        render_vendor_dropdown(frame, rows[3], state);
    }
}

fn render_vendor_field(frame: &mut Frame, label: Rect, field: Rect, state: &NewOrchState) {
    frame.render_widget(
        Paragraph::new(Span::styled("  Vendor", theme::text_hint())),
        label,
    );

    let focused = state.focus == NewOrchFocus::Vendor || state.vendor_open;
    let text = format!(" {}  ▼", state.vendor.label());

    frame.render_widget(
        Paragraph::new(Span::styled(text, theme::text())).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if focused { theme::border_active() } else { theme::border() }),
        ),
        field,
    );
}

fn render_name_field(frame: &mut Frame, label: Rect, input: Rect, state: &NewOrchState) {
    frame.render_widget(
        Paragraph::new(Span::styled("  Name", theme::text_hint())),
        label,
    );

    let focused = state.focus == NewOrchFocus::Name;
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

fn render_api_key_field(frame: &mut Frame, label: Rect, input: Rect, state: &NewOrchState) {
    frame.render_widget(
        Paragraph::new(Span::styled("  API Key", theme::text_hint())),
        label,
    );

    // For Local, replace the input with an informational message.
    if state.vendor == OrchVendor::Local {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Not required for local models  (coming soon)",
                theme::text_hint(),
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme::border()),
            ),
            input,
        );
        return;
    }

    let focused = state.focus == NewOrchFocus::ApiKey;

    // Mask every character with ● — the cursor shows position within the mask.
    let before_mask: String = std::iter::repeat('●').take(state.api_key_cursor).collect();
    let after_mask:  String = std::iter::repeat('●')
        .take(state.api_key_chars.len().saturating_sub(state.api_key_cursor))
        .collect();

    let line = if focused {
        Line::from(vec![
            Span::styled(format!(" {before_mask}"), theme::text()),
            Span::styled("│", theme::text_active()),
            Span::styled(after_mask, theme::text()),
        ])
    } else {
        let mask: String = std::iter::repeat('●').take(state.api_key_chars.len()).collect();
        Line::from(Span::styled(format!(" {mask}"), theme::text()))
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

fn render_buttons(frame: &mut Frame, area: Rect, state: &NewOrchState) {
    // Right-align both buttons with a small gap between them.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(8),  // [ Save ]
            Constraint::Length(2),  // gap
            Constraint::Length(10), // [ Cancel ]
            Constraint::Length(2),  // right margin
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "[ Save ]",
            if state.focus == NewOrchFocus::Save { theme::selected() } else { theme::text_hint() },
        )),
        cols[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "[ Cancel ]",
            if state.focus == NewOrchFocus::Cancel { theme::selected() } else { theme::text_hint() },
        )),
        cols[3],
    );
}

fn render_vendor_dropdown(frame: &mut Frame, field_area: Rect, state: &NewOrchState) {
    let height = VENDORS.len() as u16 + 2; // items + top + bottom border
    let remaining = frame
        .area()
        .height
        .saturating_sub(field_area.y + field_area.height);

    if height > remaining {
        return;
    }

    let dropdown_area = Rect {
        x:      field_area.x,
        y:      field_area.y + field_area.height,
        width:  field_area.width,
        height: height.min(remaining),
    };

    frame.render_widget(Clear, dropdown_area);

    let lines: Vec<Line> = VENDORS
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let style = if i == state.vendor_hover { theme::selected() } else { theme::text() };
            Line::from(Span::styled(format!("  {}", v.label()), style))
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
