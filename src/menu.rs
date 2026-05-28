//! Menu bar and dropdown — data model, key handling, and rendering.
//!
//! Follows the same pattern as every interactive element in this codebase:
//! a plain data struct carries all state; a free `handle_key` function
//! produces the *next* state from a keypress; free render functions draw from
//! that state. No mutation through method calls.
//!
//! # Navigation
//!
//! Press `m` from the main panes to focus the menu bar.
//! `←` `→` moves between top-level items.
//! `↓` or `Enter` opens the dropdown.
//! `↑` `↓` navigates sub-items; `Enter` confirms.
//! `Esc` closes the dropdown (or unfocuses the bar if already closed).

use crate::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

// ── Static menu definition ────────────────────────────────────────────────────

/// A single top-level menu entry and its sub-items.
///
/// All strings are `&'static str` — the menu structure is fixed at compile
/// time and never heap-allocated per frame.
pub struct MenuItem {
    /// Short label shown in the bar (e.g. `"File"`).
    pub label: &'static str,
    /// The items shown in the dropdown when this entry is open.
    pub items: &'static [&'static str],
}

/// Full application menu, read by both the renderer and the key handler.
///
/// Items are rendered left-to-right in the order they appear here.
/// Add new top-level entries as the product grows.
///
/// # Index contract
///
/// Sub-item indices here must stay in sync with the indices used in
/// `resolve_action`. For the Help menu, sub-item indices also correspond
/// directly to `help::TOPICS` indices so selecting a topic here opens the
/// help screen with that topic pre-selected.
pub const MENU: &[MenuItem] = &[
    MenuItem {
        label: "File",
        items: &["New Project", "Quit"],
    },
    MenuItem {
        label: "Help",
        // Direct-action entry — Enter opens the help screen at topic 0.
        // Add items here if a sub-menu of topics is ever wanted.
        items: &[],
    },
];

// ── Actions and state ─────────────────────────────────────────────────────────

/// An action the caller should perform after the operator confirms an item.
///
/// Returned by [`handle_key`] so the menu module stays decoupled from
/// application-level concerns — it reports *what* was chosen, not *what to do*.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    NewProject,
    Quit,
    /// Open the help screen, pre-selecting the topic at the given index.
    /// The index corresponds to a position in `help::TOPICS`.
    Help(usize),
}

/// Everything needed to render and interact with the menu bar.
///
/// Plain data — no impl methods. All logic lives in free functions below.
#[derive(Debug, Clone)]
pub struct MenuState {
    /// Whether the menu bar currently holds keyboard focus.
    /// When `false`, keypresses bypass the menu and go to the active pane.
    pub focused: bool,
    /// Which top-level item is highlighted (index into [`MENU`]).
    pub item: usize,
    /// Whether the dropdown for `item` is currently open.
    pub open: bool,
    /// Highlighted sub-item index. Meaningful only when `open` is true.
    pub sub: usize,
}

impl Default for MenuState {
    fn default() -> Self {
        Self { focused: false, item: 0, open: false, sub: 0 }
    }
}

// ── Focus helper ──────────────────────────────────────────────────────────────

/// Give the menu bar keyboard focus, resetting any prior selection.
///
/// A free function rather than a method so callers stay in control of when
/// and why focus transfers — no hidden state transition.
pub fn focus(state: MenuState) -> MenuState {
    MenuState { focused: true, open: false, sub: 0, ..state }
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// Produce the next [`MenuState`] and an optional [`MenuAction`] from a keypress.
///
/// Navigation keys mutate state and return `None`.
/// Confirming a sub-item returns `Some(action)` and closes the menu.
/// Unhandled keys are ignored — this function never consumes keypresses it
/// does not understand.
pub fn handle_key(state: MenuState, key: KeyEvent) -> (MenuState, Option<MenuAction>) {
    match (state.open, key.code) {
        // ── Bar focused, dropdown closed ──────────────────────────────────────

        // Move right through top-level items, wrapping at the end.
        (false, KeyCode::Right) => {
            let next = (state.item + 1) % MENU.len();
            (MenuState { item: next, sub: 0, ..state }, None)
        }

        // Move left, wrapping at the start.
        (false, KeyCode::Left) => {
            let prev = state.item.checked_sub(1).unwrap_or(MENU.len() - 1);
            (MenuState { item: prev, sub: 0, ..state }, None)
        }

        // Enter / Down: open the dropdown if the item has sub-items, or fire
        // a direct action if the items list is empty (e.g. Help).
        (false, KeyCode::Enter | KeyCode::Down) => {
            if MENU[state.item].items.is_empty() {
                // Direct-action item — close the menu and emit an action immediately.
                let action = resolve_direct_action(state.item);
                (MenuState { focused: false, ..state }, Some(action))
            } else {
                (MenuState { open: true, sub: 0, ..state }, None)
            }
        }

        // Esc unfocuses — keyboard focus returns to the panes.
        (false, KeyCode::Esc) => (MenuState { focused: false, ..state }, None),

        // ── Dropdown open ─────────────────────────────────────────────────────

        // Move down through sub-items, wrapping at the bottom.
        (true, KeyCode::Down) => {
            let next = (state.sub + 1) % MENU[state.item].items.len();
            (MenuState { sub: next, ..state }, None)
        }

        // Move up, wrapping at the top.
        (true, KeyCode::Up) => {
            let count = MENU[state.item].items.len();
            let prev = state.sub.checked_sub(1).unwrap_or(count - 1);
            (MenuState { sub: prev, ..state }, None)
        }

        // Enter or Space confirms the highlighted sub-item. Space is the
        // conventional selection key in many TUI dropdown contexts.
        (true, KeyCode::Enter) | (true, KeyCode::Char(' ')) => {
            let action = resolve_action(state.item, state.sub);
            let next = MenuState { open: false, focused: false, ..state };
            (next, Some(action))
        }

        // Esc closes the dropdown but keeps the bar focused — the operator can
        // move to another top-level item without re-pressing the focus key.
        (true, KeyCode::Esc) => (MenuState { open: false, sub: 0, ..state }, None),

        // All other keys are ignored.
        _ => (state, None),
    }
}

/// Map a (top-level index, sub-item index) pair to a concrete [`MenuAction`].
///
/// Only called for items that have sub-items (i.e. the dropdown path).
/// Direct-action items (empty `items` list) go through [`resolve_direct_action`].
fn resolve_action(item: usize, sub: usize) -> MenuAction {
    match (item, sub) {
        (0, 0) => MenuAction::NewProject,
        (0, 1) => MenuAction::Quit,
        _ => unreachable!("menu position ({item}, {sub}) has no sub-item action defined"),
    }
}

/// Map a top-level item index to a direct [`MenuAction`] for items with no sub-items.
///
/// Called when the operator presses Enter on a top-level entry whose `items`
/// list is empty. Adding a new direct-action item requires a new match arm here.
fn resolve_direct_action(item: usize) -> MenuAction {
    match item {
        // Help opens the help screen at the first topic.
        1 => MenuAction::Help(0),
        _ => unreachable!("menu item {item} has no direct action defined"),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render the menu bar into `area` (a single-row rect at the top of the screen).
///
/// Each top-level entry is shown as `[ Label ]`. The active entry is inverted
/// when the menu has focus, giving a clear visual cue without any graphical assets.
/// A dim hint on the right edge reminds the operator how to reach the menu.
pub fn render_bar(frame: &mut Frame, area: Rect, state: &MenuState) {
    // Build the list of styled spans for all top-level items.
    let mut spans: Vec<Span> = MENU
        .iter()
        .enumerate()
        .flat_map(|(i, entry)| {
            let active = state.focused && i == state.item;

            // Active: black-on-matrix-green selection. Inactive: faint green hint.
            let style = if active { theme::selected() } else { theme::text_hint() };

            // Brackets signal "focusable button" in a purely text environment.
            [Span::styled(format!("[ {} ]", entry.label), style), Span::raw("  ")]
        })
        .collect();

    // Dim hint shown only when unfocused — does not compete with active state.
    if !state.focused {
        spans.push(Span::styled(" m: menu ", theme::text_hint()));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Render the open dropdown below the currently active top-level item.
///
/// Positioned directly below its button in the bar. [`Clear`] erases whatever
/// is underneath first so the dropdown paints cleanly over pane borders.
/// Does nothing when [`MenuState::open`] is false.
pub fn render_dropdown(frame: &mut Frame, bar_area: Rect, state: &MenuState) {
    if !state.open {
        // Nothing to render — dropdown is closed.
        return;
    }

    let entry = &MENU[state.item];

    // Sum the rendered widths of all items *before* the active one to find
    // its horizontal position. Each item renders as "[ Label ]  " — label
    // length + 8 characters of surrounding decoration.
    let x_offset: u16 = MENU
        .iter()
        .take(state.item)
        .map(|m| m.label.len() as u16 + 8)
        .sum();

    // Size the dropdown to fit its contents: longest label + 2 chars padding
    // each side + 2 border chars. No cursor prefix — colour inversion is enough.
    let longest = entry.items.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
    let width  = longest + 4; // 2 padding each side
    let height = entry.items.len() as u16 + 2; // items + top border + bottom border

    // Clamp x so the dropdown never extends past the right edge of the screen.
    let screen = frame.area();
    let x = (bar_area.x + x_offset).min(screen.width.saturating_sub(width));
    let y = bar_area.y + 1; // immediately below the bar row

    let dropdown_area = Rect {
        x,
        y,
        width:  width.min(screen.width.saturating_sub(x)),
        height: height.min(screen.height.saturating_sub(y)),
    };

    // Erase whatever was drawn underneath before we paint the dropdown.
    // Without Clear, the pane border shows through transparent areas.
    frame.render_widget(Clear, dropdown_area);

    // Build one line per sub-item. The selected row is inverted (black on green);
    // others are dim green. Colour inversion alone is sufficient to show selection —
    // no cursor glyph needed.
    let lines: Vec<Line> = entry
        .items
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let style = if i == state.sub { theme::selected() } else { theme::text_hint() };
            // Two spaces of left padding keep text away from the border.
            Line::from(Span::styled(format!("  {label}"), style))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border()),
        ),
        dropdown_area,
    );
}
