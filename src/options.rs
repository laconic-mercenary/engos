//! Options screen — application settings.
//!
//! Currently contains only the Appearance section: a colour palette selector.
//! Navigating the list immediately applies the palette so the operator sees
//! the change in real time. Esc closes and keeps whatever is selected.

use crate::theme::{self, Palette, PALETTE_NAMES};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OptionsState {
    /// Index of the highlighted (and currently active) palette.
    pub selected: usize,
}

/// Build a fresh state reflecting the palette that is active right now.
///
/// Called each time the operator enters the Options screen so the cursor
/// always starts on the current selection rather than defaulting to index 0.
pub fn new_state() -> OptionsState {
    OptionsState { selected: theme::current_palette() as usize }
}

// ── Actions ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum OptionsAction {
    Close,
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// Navigate the palette list.
///
/// Moving the cursor immediately calls [`theme::set_palette`] so changes are
/// visible in real time with no separate Apply step. Esc closes without
/// reverting — whatever is selected stays active.
pub fn handle_key(
    mut state: OptionsState,
    key: KeyEvent,
) -> (OptionsState, Option<OptionsAction>) {
    match key.code {
        KeyCode::Up => {
            let prev = state.selected.checked_sub(1).unwrap_or(PALETTE_NAMES.len() - 1);
            state.selected = prev;
            theme::set_palette(Palette::from_index(prev));
            (state, None)
        }
        KeyCode::Down => {
            let next = (state.selected + 1) % PALETTE_NAMES.len();
            state.selected = next;
            theme::set_palette(Palette::from_index(next));
            (state, None)
        }
        KeyCode::Esc => (state, Some(OptionsAction::Close)),
        _ => (state, None),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render the options screen.
///
/// A centred dialog contains the Appearance group box. A hint bar spans the
/// full width below the dialog.
pub fn render(frame: &mut Frame, area: Rect, state: &OptionsState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1)])
        .split(area);

    render_dialog(frame, rows[0], state);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓ ", theme::text_active()),
            Span::styled("change palette   ", theme::text_hint()),
            Span::styled("Esc ", theme::text_active()),
            Span::styled("close", theme::text_hint()),
        ])),
        rows[1],
    );
}

fn render_dialog(frame: &mut Frame, area: Rect, state: &OptionsState) {
    // Height breakdown: 2 border + 1 top pad + (2 group border + 2 inner pad + 4 items) + 1 bot pad = 14.
    // Using 16 gives a little extra breathing room.
    let dialog_w = 46_u16.min(area.width);
    let dialog_h = 16_u16.min(area.height);
    let dialog_area = centered_rect(dialog_w, dialog_h, area);

    frame.render_widget(Clear, dialog_area);
    frame.render_widget(
        Block::default()
            .title(Span::styled(" OPTIONS ", theme::text_active()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::border_active()),
        dialog_area,
    );

    let inner = inner_rect(dialog_area);

    // One row of padding above and below the Appearance group box.
    let inner_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(inner);

    render_appearance_group(frame, inner_rows[1], state);
}

fn render_appearance_group(frame: &mut Frame, area: Rect, state: &OptionsState) {
    let block = Block::default()
        .title(Span::styled(" Appearance ", theme::text_active()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertically centre the four palette items inside the group box.
    let item_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1), // Green
            Constraint::Length(1), // Blue
            Constraint::Length(1), // White
            Constraint::Length(1), // Red
            Constraint::Fill(1),
        ])
        .split(inner);

    let palettes = [Palette::Green, Palette::Blue, Palette::White, Palette::Red];
    for (i, &palette) in palettes.iter().enumerate() {
        render_palette_item(frame, item_rows[i + 1], palette, PALETTE_NAMES[i], i == state.selected);
    }
}

/// Render one palette row: selection prefix + colour swatch (██) + label.
///
/// The swatch is always rendered in the bright accent colour of *that* palette
/// regardless of which palette is currently active, so every option is always
/// visually identifiable.
fn render_palette_item(
    frame: &mut Frame,
    area: Rect,
    palette: Palette,
    label: &'static str,
    selected: bool,
) {
    let swatch = Style::default().fg(theme::palette_bright_color(palette));
    let (prefix, label_style) = if selected {
        ("  ▶  ", theme::text_active())
    } else {
        ("     ", theme::text_hint())
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(prefix, label_style),
            Span::styled("██ ", swatch),
            Span::styled(label, label_style),
        ])),
        area,
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
