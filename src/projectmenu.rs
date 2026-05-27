//! Project context menu — a small popup shown when the operator presses Enter
//! on a highlighted project in the main screen list.
//!
//! Two items: Open Project and Delete Project.
//! Navigation: ↑/↓ or Tab/Shift+Tab. Enter or Space confirms. Esc closes.

use crate::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Span,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ── Menu items ────────────────────────────────────────────────────────────────

/// The ordered list of items in the popup.
///
/// Keeping them in a slice means the hover index maps directly to the item
/// without any separate bookkeeping.
const ITEMS: &[&str] = &["Open Project", "Delete Project"];

// ── State ─────────────────────────────────────────────────────────────────────

/// State for the project context menu.
#[derive(Debug, Clone)]
pub struct ProjectMenuState {
    /// Index into `projects` — the project this menu is acting on.
    pub project_idx: usize,
    /// Which menu item is currently highlighted (0..ITEMS.len()).
    pub hover: usize,
}

/// Build a fresh menu state targeting the given project.
pub fn new_state(project_idx: usize) -> ProjectMenuState {
    ProjectMenuState { project_idx, hover: 0 }
}

/// An action the caller should handle after the menu resolves.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectMenuAction {
    /// Open the selected project. No workspace screen yet — placeholder.
    Open(usize),
    /// Permanently delete the project from the list and from disk config.
    Delete(usize),
    /// Operator pressed Esc or moved away — close without doing anything.
    Close,
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// Produce the next state and an optional action from a keypress.
pub fn handle_key(
    mut state: ProjectMenuState,
    key: KeyEvent,
) -> (ProjectMenuState, Option<ProjectMenuAction>) {
    match key.code {
        // ↓ / Tab — move to the next item, wrapping.
        KeyCode::Down | KeyCode::Tab => {
            state.hover = (state.hover + 1) % ITEMS.len();
            (state, None)
        }
        // ↑ / Shift+Tab — move to the previous item, wrapping.
        KeyCode::Up | KeyCode::BackTab => {
            state.hover = state.hover.checked_sub(1).unwrap_or(ITEMS.len() - 1);
            (state, None)
        }
        // Enter or Space confirms the highlighted item.
        KeyCode::Enter | KeyCode::Char(' ') => {
            let action = match state.hover {
                0 => ProjectMenuAction::Open(state.project_idx),
                1 => ProjectMenuAction::Delete(state.project_idx),
                _ => unreachable!("hover index out of range for ITEMS"),
            };
            (state, Some(action))
        }
        // Esc closes without acting.
        KeyCode::Esc => (state, Some(ProjectMenuAction::Close)),
        _ => (state, None),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render the context menu as a small popup centred on `area`.
///
/// `project_name` is used as the popup title so the operator can see which
/// project the actions will apply to.
pub fn render(frame: &mut Frame, area: Rect, state: &ProjectMenuState, project_name: &str) {
    // Size: wide enough for the longest item + margins; tall enough for the items.
    let popup_w = 26_u16.min(area.width);
    let popup_h = (ITEMS.len() as u16 + 4).min(area.height); // items + border + padding
    let popup_area = centered_rect(popup_w, popup_h, area);

    // Erase whatever is behind the popup.
    frame.render_widget(Clear, popup_area);

    // Outer border — title is the project name so the action is unambiguous.
    frame.render_widget(
        Block::default()
            .title(Span::styled(
                format!(" {} ", project_name),
                theme::text_active(),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::border_active()),
        popup_area,
    );

    // Content area — shrink by 1 on each side for the border.
    let inner = Rect {
        x:      popup_area.x + 1,
        y:      popup_area.y + 1,
        width:  popup_area.width.saturating_sub(2),
        height: popup_area.height.saturating_sub(2),
    };

    // Vertical layout: top padding | item… | bottom padding.
    let mut constraints = vec![Constraint::Fill(1)];
    constraints.extend(ITEMS.iter().map(|_| Constraint::Length(1)));
    constraints.push(Constraint::Fill(1));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Render each item. Hovered item is inverted; others are dim.
    for (i, label) in ITEMS.iter().enumerate() {
        let style = if i == state.hover { theme::selected() } else { theme::text() };
        frame.render_widget(
            Paragraph::new(Span::styled(format!("  {label}"), style)),
            rows[i + 1], // +1 for the top Fill padding row
        );
    }
}

// ── Layout helper ─────────────────────────────────────────────────────────────

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
