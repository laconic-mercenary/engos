//! Help screen — a navigable topics list and a content pane.
//!
//! Layout: two side-by-side panes filling the terminal, with a one-row hint
//! bar at the bottom. Highlighting a topic in the left pane immediately
//! updates the content on the right.
//!
//! Topics and their content are defined as static data. Adding a new topic
//! requires only a new entry in [`TOPICS`] — no other code changes needed.

use crate::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

// ── Topic data ────────────────────────────────────────────────────────────────

/// A single help topic: a short title shown in the list, and content shown in
/// the content pane when the topic is selected.
pub struct HelpTopic {
    /// Short label rendered in the topics list and as the content pane title.
    pub title: &'static str,
    /// Body text displayed in the content pane.
    /// Multi-line strings are rendered verbatim; wrapping is handled by ratatui.
    pub content: &'static str,
}

/// All help topics, in the order they appear in the list.
///
/// The index of each entry here matches the `topic` field of
/// [`MenuAction::Help`] — keep them in sync when adding topics.
pub const TOPICS: &[HelpTopic] = &[HelpTopic {
    title: "First Time Users",
    // Placeholder content — replace with real onboarding text before the demo.
    content: "asdfasdf",
}];

// ── State and actions ─────────────────────────────────────────────────────────

/// All state the help screen needs to render and respond to input.
///
/// Plain data — no methods. All logic lives in free functions below.
#[derive(Debug, Clone)]
pub struct HelpState {
    /// Index of the currently highlighted topic in [`TOPICS`].
    pub selected: usize,
}

impl Default for HelpState {
    fn default() -> Self {
        // Start with the first topic selected so the content pane is never empty.
        Self { selected: 0 }
    }
}

/// An action the caller should handle after a keypress on the help screen.
#[derive(Debug, Clone, PartialEq)]
pub enum HelpAction {
    /// The operator pressed Esc — close the help screen and return to Main.
    Close,
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// Produce the next [`HelpState`] and an optional [`HelpAction`] from a keypress.
///
/// `↑`/`↓` navigate the topic list. `Esc` closes the screen.
/// All other keys are ignored so they do not leak into the calling context.
pub fn handle_key(state: HelpState, key: KeyEvent) -> (HelpState, Option<HelpAction>) {
    match key.code {
        // Move down through topics, wrapping at the end.
        KeyCode::Down => {
            let next = (state.selected + 1) % TOPICS.len();
            (HelpState { selected: next }, None)
        }

        // Move up, wrapping at the top.
        KeyCode::Up => {
            let prev = state.selected.checked_sub(1).unwrap_or(TOPICS.len() - 1);
            (HelpState { selected: prev }, None)
        }

        // Esc signals the caller to close the help screen.
        KeyCode::Esc => (state, Some(HelpAction::Close)),

        // All other keys are ignored.
        _ => (state, None),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render the full help screen into `area`.
///
/// Splits `area` into the two-pane layout above and a one-row hint bar below.
/// Pure function — draws from state, mutates nothing.
pub fn render(frame: &mut Frame, area: Rect, state: &HelpState) {
    // One row at the bottom for navigation hints; the rest for the panes.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1)])
        .split(area);

    render_panes(frame, rows[0], state);
    render_hint_bar(frame, rows[1]);
}

/// Render the topics list and content pane side by side.
fn render_panes(frame: &mut Frame, area: Rect, state: &HelpState) {
    // 28 % for the narrow topics list; 72 % for the wider content pane.
    // The ratio keeps the list compact without truncating typical topic titles.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
        .split(area);

    render_topics(frame, cols[0], state);
    render_content(frame, cols[1], state);
}

/// Render the navigable topics list into `area`.
///
/// The selected topic is highlighted with `theme::selected()`; all others use
/// `theme::text()`. The `>` cursor gives an unambiguous position indicator.
fn render_topics(frame: &mut Frame, area: Rect, state: &HelpState) {
    // Build one styled line per topic. The `>` cursor on the selected entry
    // gives a clear position indicator that works in any terminal colour scheme.
    let lines: Vec<Line> = TOPICS
        .iter()
        .enumerate()
        .map(|(i, topic)| {
            let (cursor, style) = if i == state.selected {
                // Bright green selection — matches the dropdown highlight style.
                (">", theme::selected())
            } else {
                (" ", theme::text())
            };
            Line::from(Span::styled(format!("{cursor} {}", topic.title), style))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(Span::styled(" TOPICS ", theme::text_active()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border()),
        ),
        area,
    );
}

/// Render the content pane for the currently selected topic.
///
/// The pane title is the topic name so the operator always knows which topic
/// they are reading, even after scrolling away from the list.
fn render_content(frame: &mut Frame, area: Rect, state: &HelpState) {
    let topic = &TOPICS[state.selected];

    frame.render_widget(
        Paragraph::new(topic.content)
            // Body text at standard brightness — readable but not aggressive.
            .style(theme::text())
            .block(
                Block::default()
                    // Title mirrors the selected topic name.
                    .title(Span::styled(
                        format!(" {} ", topic.title),
                        theme::text_active(),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme::border()),
            ),
        area,
    );
}

/// Render the one-row hint bar at the bottom of the help screen.
///
/// Key labels are bright green; descriptions are dim — the operator's eye
/// goes to the key first, then reads the label.
fn render_hint_bar(frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(" ↑↓ ", theme::text_active()),
        Span::styled("navigate topics    ", theme::text_hint()),
        Span::styled("Esc ", theme::text_active()),
        Span::styled("close", theme::text_hint()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
