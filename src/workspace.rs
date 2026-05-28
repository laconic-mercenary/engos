//! Project workspace — the active engagement screen.
//!
//! Layout (left → right, top → bottom):
//!
//!   [project name header — 1 row]
//!   ┌─ Commands ──────────┐  ┌─ Report ───────────────────┐
//!   │ instruction text    │  │                            │
//!   │ (hides once chat    │  │                            │
//!   │  has messages)      │  └────────────────────────────┘
//!   │                     │  ┌─ Artifacts ─┐ ┌─ Logs ────┐
//!   │ [chat history]      │  │ Processed   │ │           │
//!   │                     │  │ Synthesized │ │           │
//!   │ ┌─ input ─────────┐ │  └─────────────┘ └───────────┘
//!   │ └─────────────────┘ │
//!   └─────────────────────┘
//!   [hint bar — 1 row]
//!
//! The input box is always focused.
//! Enter submits the typed command as a user chat message.
//! Esc returns to the main menu.

use crate::theme;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

// ── Chat types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Model,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub text: String,
}

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WorkspaceState {
    pub project_name: String,

    // ── Text input (always focused) ───────────────────────────────────────────
    pub input_chars:  Vec<char>,
    pub input_cursor: usize,

    // ── Chat history ──────────────────────────────────────────────────────────
    pub chat_history: Vec<ChatMessage>,

    // ── Artifact counters ─────────────────────────────────────────────────────
    pub artifacts_processed:   u64,
    pub artifacts_synthesized: u64,

    // ── Processing log ────────────────────────────────────────────────────────
    pub logs: Vec<String>,

    // ── Chat scroll ───────────────────────────────────────────────────────────
    /// How many rows the chat history view is scrolled up from the bottom.
    /// 0 = newest messages visible (default).
    /// Ctrl+↑ increments, Ctrl+↓ decrements.
    /// Resets to 0 whenever a new message is sent or a paste artifact arrives.
    pub chat_scroll: usize,

    // ── Close-confirmation dialog ─────────────────────────────────────────────
    /// When `true` the workspace shows the "Leave project?" overlay and waits
    /// for a deliberate `y` before closing. Any other key cancels.
    pub confirm_close: bool,
}

pub fn new_state(project_name: &str) -> WorkspaceState {
    WorkspaceState {
        project_name:          project_name.to_string(),
        input_chars:           Vec::new(),
        input_cursor:          0,
        chat_history:          Vec::new(),
        artifacts_processed:   0,
        artifacts_synthesized: 0,
        logs:                  Vec::new(),
        chat_scroll:           0,
        confirm_close:         false,
    }
}

// ── Actions ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceAction {
    Close,
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// All keypresses route to the text input (it is always focused).
///
/// `Esc` raises the close-confirmation overlay rather than closing immediately.
/// While the overlay is visible, `y` confirms the close and any other key
/// (including `Esc` again) dismisses it without closing.
/// `Ctrl+C` is intercepted before this function by the global handler in
/// `tui.rs`, so it never arrives here.
pub fn handle_key(
    mut state: WorkspaceState,
    key: KeyEvent,
) -> (WorkspaceState, Option<WorkspaceAction>) {
    // ── Confirmation overlay is active ────────────────────────────────────────
    if state.confirm_close {
        match (key.code, key.modifiers) {
            // Only an explicit 'y' confirms the close.
            (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) => {
                return (state, Some(WorkspaceAction::Close));
            }
            // Every other key — including Esc and 'n' — cancels.
            _ => {
                state.confirm_close = false;
                return (state, None);
            }
        }
    }

    // ── Normal workspace input ────────────────────────────────────────────────
    match (key.code, key.modifiers) {
        // Esc raises the confirmation dialog instead of closing immediately,
        // because leaving an open project stops directory monitoring and paste.
        (KeyCode::Esc, _) => {
            state.confirm_close = true;
            (state, None)
        }

        // Scroll chat history independently of the input.
        (KeyCode::Up, KeyModifiers::CONTROL) => {
            state.chat_scroll = state.chat_scroll.saturating_add(1);
            (state, None)
        }
        (KeyCode::Down, KeyModifiers::CONTROL) => {
            state.chat_scroll = state.chat_scroll.saturating_sub(1);
            (state, None)
        }

        // Submit the current input buffer as a user message.
        // Resets scroll so the new message is immediately visible.
        (KeyCode::Enter, _) => {
            if !state.input_chars.is_empty() {
                let text: String = state.input_chars.iter().collect();
                state.chat_history.push(ChatMessage { role: MessageRole::User, text });
                state.input_chars.clear();
                state.input_cursor = 0;
                state.chat_scroll  = 0;
            }
            (state, None)
        }

        // Cursor movement within the input.
        (KeyCode::Left, _) => {
            state.input_cursor = state.input_cursor.saturating_sub(1);
            (state, None)
        }
        (KeyCode::Right, _) => {
            if state.input_cursor < state.input_chars.len() {
                state.input_cursor += 1;
            }
            (state, None)
        }
        (KeyCode::Home, _) => {
            state.input_cursor = 0;
            (state, None)
        }
        (KeyCode::End, _) => {
            state.input_cursor = state.input_chars.len();
            (state, None)
        }

        // Delete characters.
        (KeyCode::Backspace, _) => {
            if state.input_cursor > 0 {
                state.input_chars.remove(state.input_cursor - 1);
                state.input_cursor -= 1;
            }
            (state, None)
        }
        (KeyCode::Delete, _) => {
            if state.input_cursor < state.input_chars.len() {
                state.input_chars.remove(state.input_cursor);
            }
            (state, None)
        }

        // Printable characters — NONE and SHIFT modifiers only so that
        // Ctrl/Alt combos do not accidentally land in the input buffer.
        // Hard cap at 500 chars; content beyond that belongs in an artifact,
        // not a command string.
        (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
            if state.input_chars.len() < 500 {
                state.input_chars.insert(state.input_cursor, c);
                state.input_cursor += 1;
            }
            (state, None)
        }

        _ => (state, None),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // project name header
            Constraint::Fill(1),   // main panes
            Constraint::Length(1), // hint bar
        ])
        .split(area);

    render_header(frame, rows[0], state);
    render_panes(frame, rows[1], state);
    render_hint_bar(frame, rows[2], state);

    // Confirmation overlay sits on top of everything — render last so it is
    // not obscured by any pane content.
    if state.confirm_close {
        render_confirm_overlay(frame, area);
    }
}

fn render_header(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ", theme::text_hint()),
            Span::styled(state.project_name.as_str(), theme::text_active()),
        ])),
        area,
    );
}

/// Hint bar — shows context-sensitive shortcuts across the full screen width.
fn render_hint_bar(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    let line = if state.confirm_close {
        Line::from(vec![
            Span::styled(" y ", theme::text_active()),
            Span::styled("leave project   ", theme::text_hint()),
            Span::styled(" any key ", theme::text_active()),
            Span::styled("stay", theme::text_hint()),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Esc ", theme::text_active()),
            Span::styled("leave   ", theme::text_hint()),
            Span::styled(" Enter ", theme::text_active()),
            Span::styled("send   ", theme::text_hint()),
            Span::styled(" Ctrl+↑↓ ", theme::text_active()),
            Span::styled("scroll   ", theme::text_hint()),
            Span::styled(" paste ", theme::text_active()),
            Span::styled("→ artifact", theme::text_hint()),
        ])
    };
    frame.render_widget(Paragraph::new(line), area);
}

// ── Pane layout ───────────────────────────────────────────────────────────────

fn render_panes(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    // Commands takes 38 %; Report + the bottom band share the remaining 62 %.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);

    render_commands(frame, cols[0], state);
    render_right_panel(frame, cols[1], state);
}

fn render_right_panel(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    // Report fills available height; Artifacts + Logs share a fixed-height band.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(8)])
        .split(area);

    render_report(frame, rows[0]);

    // Artifacts and Logs sit side-by-side in the bottom band.
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(rows[1]);

    render_artifacts(frame, bottom[0], state);
    render_logs(frame, bottom[1], state);
}

// ── Commands pane ─────────────────────────────────────────────────────────────

fn render_commands(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    // The Commands border is always active because the input is always focused.
    let block = Block::default()
        .title(Span::styled(" Commands ", theme::text_active()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_active());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.chat_history.is_empty() {
        // Before first message: instruction text at top, input at bottom.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),  // instruction text (wraps to ≤5 lines)
                Constraint::Fill(1),    // empty space
                Constraint::Length(14), // input box (12 inner rows + 2 border rows)
            ])
            .split(inner);

        render_instruction(frame, rows[0]);
        render_input(frame, rows[2], state);
    } else {
        // Once chat starts: history fills available space above the input.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),    // chat history
                Constraint::Length(14), // input box (12 inner rows + 2 border rows)
            ])
            .split(inner);

        render_chat_history(frame, rows[0], state);
        render_input(frame, rows[1], state);
    }
}

const INSTRUCTION: &str =
    "Instruct me what to do — for example \
     \"Watch /path/to/directory for notes\" or \
     \"Adjust the report to be less verbose\" — \
     my abilities are flexible. \
     You can also paste text or images directly; they will be logged as artifacts.";

fn render_instruction(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new(INSTRUCTION)
            .style(theme::text_hint())
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_chat_history(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    let all_lines: Vec<Line> = state.chat_history.iter()
        .map(|msg| match msg.role {
            MessageRole::User  => Line::from(vec![
                Span::styled("  ▶  ", theme::text_hint()),
                Span::styled(msg.text.as_str(), theme::text()),
            ]),
            MessageRole::Model => Line::from(vec![
                Span::styled("  ◀  ", theme::text_active()),
                Span::styled(msg.text.as_str(), theme::text()),
            ]),
        })
        .collect();

    let total  = all_lines.len();
    let height = area.height as usize;

    if total == 0 || height == 0 { return; }

    if total <= height {
        // Everything fits — no scroll, no indicators.
        frame.render_widget(Paragraph::new(all_lines), area);
        return;
    }

    // Reserve 2 rows for up/down indicators; the remaining rows show messages.
    // When an indicator isn't needed its slot is empty (Paragraph pads naturally).
    let msg_rows   = height.saturating_sub(2).max(1);
    let max_scroll = total.saturating_sub(msg_rows);
    let scroll     = state.chat_scroll.min(max_scroll);

    // The visible window ends at `anchor_end` (exclusive) and holds `msg_rows` lines.
    let anchor_end   = total - scroll;
    let anchor_start = anchor_end.saturating_sub(msg_rows);

    let need_top    = anchor_start > 0;
    let need_bottom = scroll > 0;

    let mut lines: Vec<Line> = Vec::with_capacity(height);

    if need_top {
        lines.push(Line::from(Span::styled(
            format!("  ↑  {} older", anchor_start),
            theme::text_hint(),
        )));
    }

    lines.extend_from_slice(&all_lines[anchor_start..anchor_end]);

    if need_bottom {
        lines.push(Line::from(Span::styled(
            format!("  ↓  {} newer", scroll),
            theme::text_hint(),
        )));
    }

    frame.render_widget(Paragraph::new(lines), area);
}


fn render_input(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    let before: String = state.input_chars[..state.input_cursor].iter().collect();
    let after:  String = state.input_chars[state.input_cursor..].iter().collect();

    // Wrap is enabled so long commands flow naturally across the three inner
    // rows rather than scrolling horizontally out of sight. The cursor `│`
    // appears at the correct character position within the wrapped text.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {before}"), theme::text()),
            Span::styled("│", theme::text_active()),
            Span::styled(after, theme::text()),
        ]))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border_active()),
        ),
        area,
    );
}

// ── Report pane ───────────────────────────────────────────────────────────────

fn render_report(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Block::default()
            .title(Span::styled(" Report ", theme::text_active()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::border()),
        area,
    );
}

// ── Artifacts pane ────────────────────────────────────────────────────────────

fn render_artifacts(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    let processed   = state.artifacts_processed.to_string();
    let synthesized = state.artifacts_synthesized.to_string();

    let block = Block::default()
        .title(Span::styled(" Artifacts ", theme::text_active()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Centre the two stat rows vertically within the group box.
    let center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(2), Constraint::Fill(1)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(vec![
            stat_line("  Processed    ", processed),
            stat_line("  Synthesized  ", synthesized),
        ]),
        center[1],
    );
}

/// Build a two-column stat row — dim label, normal value.
fn stat_line(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(label, theme::text_hint()),
        Span::styled(value, theme::text()),
    ])
}

// ── Logs pane ─────────────────────────────────────────────────────────────────

fn render_logs(frame: &mut Frame, area: Rect, state: &WorkspaceState) {
    let block = Block::default()
        .title(Span::styled(" Logs ", theme::text_active()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_lines = inner.height as usize;
    let start = state.logs.len().saturating_sub(max_lines);
    let lines: Vec<Line> = state.logs[start..]
        .iter()
        .map(|entry| Line::from(Span::styled(format!("  {entry}"), theme::text_hint())))
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

// ── Close-confirmation overlay ────────────────────────────────────────────────

/// Render a small centred dialog asking the operator to confirm leaving the
/// project. Rendered on top of the workspace — `Clear` erases whatever is
/// behind it before drawing the box.
fn render_confirm_overlay(frame: &mut Frame, area: Rect) {
    let box_w = 52_u16.min(area.width);
    let box_h = 9_u16.min(area.height);
    let box_area = centered_rect(box_w, box_h, area);

    frame.render_widget(Clear, box_area);
    frame.render_widget(
        Block::default()
            .title(Span::styled(" Leave Project? ", theme::text_active()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::border_active()),
        box_area,
    );

    let inner = inner_rect(box_area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(2), // message text (may wrap to 2 lines)
            Constraint::Fill(1),
            Constraint::Length(1), // key hint
            Constraint::Fill(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new("  Directory monitoring and paste input will stop.")
            .style(theme::text_hint())
            .wrap(Wrap { trim: true }),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  y ", theme::text_active()),
            Span::styled("leave project   ", theme::text_hint()),
            Span::styled("any key ", theme::text_active()),
            Span::styled("stay", theme::text_hint()),
        ])),
        rows[3],
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
