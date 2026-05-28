//! Game-style main menu — centred rectangular pane with four navigable items.
//!
//! ↑ / ↓  navigate items.  Enter / Space  confirms.  q  quits directly.
//! The ▶ cursor marks the selected item; unselected items are dim.

use crate::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ── Items ─────────────────────────────────────────────────────────────────────

const ITEMS: &[&str] = &["New Report", "Existing Report", "Options", "Help", "Exit"];

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MainMenuState {
    pub selected: usize,
}

impl Default for MainMenuState {
    fn default() -> Self {
        Self { selected: 0 }
    }
}

// ── Actions ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MainMenuAction {
    NewReport,
    ExistingReport,
    Options,
    Help,
    Quit,
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// Produce the next state and an optional action from a keypress.
///
/// Navigation wraps around at both ends. Ctrl+C and q both fire Quit so the
/// operator always has a reliable escape hatch from the landing screen.
pub fn handle_key(state: MainMenuState, key: KeyEvent) -> (MainMenuState, Option<MainMenuAction>) {
    use crossterm::event::KeyModifiers;
    match (key.code, key.modifiers) {
        // Hard quit shortcuts that bypass the Exit item.
        (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
            (state, Some(MainMenuAction::Quit))
        }

        // Navigate up, wrapping to the last item.
        (KeyCode::Up, _) => {
            let prev = state.selected.checked_sub(1).unwrap_or(ITEMS.len() - 1);
            (MainMenuState { selected: prev }, None)
        }

        // Navigate down, wrapping to the first item.
        (KeyCode::Down, _) => {
            let next = (state.selected + 1) % ITEMS.len();
            (MainMenuState { selected: next }, None)
        }

        // Enter or Space confirms the highlighted item.
        (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => {
            let action = match state.selected {
                0 => MainMenuAction::NewReport,
                1 => MainMenuAction::ExistingReport,
                2 => MainMenuAction::Options,
                3 => MainMenuAction::Help,
                _ => MainMenuAction::Quit,
            };
            (state, Some(action))
        }

        _ => (state, None),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render the main menu centred on `area`.
///
/// Box is 36 wide × 18 tall — wide enough for all labels with prefix padding,
/// tall enough that items sit with a blank line of breathing room between each.
pub fn render(frame: &mut Frame, area: Rect, state: &MainMenuState) {
    let box_w = 38_u16.min(area.width);
    // 2-row title block + 1 rule + 1 gap + 5 items + 4 spacers + 2 Fill = 22.
    let box_h = 22_u16.min(area.height);
    let box_area = centered_rect(box_w, box_h, area);

    frame.render_widget(Clear, box_area);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::border()),
        box_area,
    );

    let inner = inner_rect(box_area);

    // Layout:
    //  [0]  Fill   — top padding
    //  [1]  title text  (E  N  G  O  S)
    //  [2]  rule        (──────────────)
    //  [3]  gap before items
    //  [4]  item 0  (New Report)
    //  [5]  spacer
    //  [6]  item 1  (Existing Report)
    //  [7]  spacer
    //  [8]  item 2  (Options)
    //  [9]  spacer
    //  [10] item 3  (Help)
    //  [11] spacer
    //  [12] item 4  (Exit)
    //  [13] Fill   — bottom padding
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),   // top padding
            Constraint::Length(1), // title
            Constraint::Length(1), // rule
            Constraint::Length(1), // gap
            Constraint::Length(1), // item 0
            Constraint::Length(1), // spacer
            Constraint::Length(1), // item 1
            Constraint::Length(1), // spacer
            Constraint::Length(1), // item 2
            Constraint::Length(1), // spacer
            Constraint::Length(1), // item 3
            Constraint::Length(1), // spacer
            Constraint::Length(1), // item 4
            Constraint::Fill(1),   // bottom padding
        ])
        .split(inner);

    // Title — wide-spaced capitals, centred.
    frame.render_widget(
        Paragraph::new(Span::styled("E  N  G  O  S", theme::text_active()))
            .alignment(Alignment::Center),
        rows[1],
    );

    // Decorative rule below the title.
    frame.render_widget(
        Paragraph::new(Span::styled("─────────────────────────────────", theme::text_hint()))
            .alignment(Alignment::Center),
        rows[2],
    );

    let item_rows = [rows[4], rows[6], rows[8], rows[10], rows[12]];
    for (i, (&row, &label)) in item_rows.iter().zip(ITEMS.iter()).enumerate() {
        render_item(frame, row, label, i == state.selected);
    }
}

/// Render a single menu item row.
///
/// Selected item: bright green + ▶ prefix.
/// Unselected: dim green with aligned space prefix so labels stay left-aligned.
fn render_item(frame: &mut Frame, area: Rect, label: &str, selected: bool) {
    let line = if selected {
        Line::from(vec![
            Span::styled("  ▶  ", theme::text_active()),
            Span::styled(label, theme::text_active()),
        ])
    } else {
        Line::from(Span::styled(format!("     {label}"), theme::text_hint()))
    };
    frame.render_widget(Paragraph::new(line), area);
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
