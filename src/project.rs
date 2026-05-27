//! Project data model, navigation state, key handling, and rendering.
//!
//! The main screen shows two panes side by side: a scrollable project list on
//! the left, and statistics for the highlighted project on the right.

use crate::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use serde::{Deserialize, Serialize};

// ── Data model ────────────────────────────────────────────────────────────────

/// A single engagement project loaded from `~/.engos/projects.json`.
///
/// All fields are strings or integers so the JSON schema stays simple and
/// human-editable without requiring any date-parsing library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Human-readable engagement name shown in the project list.
    pub name: String,
    /// ISO 8601 datetime string for when the engagement started.
    pub start_datetime: String,
    /// Free-form model identifier, e.g. `"anthropic claude-opus-4-7"` or
    /// `"qwen_7B_1.2.2"`. Stored as a string so it is not tied to a fixed enum.
    pub specialist_model: String,
    /// Running total of artifacts ever collected — includes deleted items so
    /// the count never decreases and reflects total engagement activity.
    pub artifacts_collected: u64,
    /// Number of artifacts that have been synthesised into the report structure.
    pub artifacts_synthesized: u64,
    /// ISO 8601 datetime the project was last opened in engos, if ever.
    /// `None` for projects that were created but never opened interactively.
    #[serde(default)]
    pub last_opened: Option<String>,
    /// ISO 8601 datetime the project data was last written to disk.
    /// `None` for projects imported from an older config that predates this field.
    #[serde(default)]
    pub last_modified: Option<String>,
}

// ── Navigation state ──────────────────────────────────────────────────────────

/// All state needed to render and navigate the projects screen.
///
/// Plain data — no methods. Logic lives in free functions below.
#[derive(Debug, Clone)]
pub struct ProjectState {
    /// Index of the currently highlighted project, or `None` if the list is empty.
    pub selected: Option<usize>,
}

impl ProjectState {
    /// Initialise state from the loaded project list.
    ///
    /// Pre-selects the first project so the stats pane is never blank on load
    /// (assuming at least one project exists).
    pub fn init(projects: &[Project]) -> Self {
        Self {
            selected: if projects.is_empty() { None } else { Some(0) },
        }
    }
}

// ── Key handling ──────────────────────────────────────────────────────────────

/// Produce the next [`ProjectState`] from a keypress.
///
/// `↑`/`↓` navigate the list with wrap-around. All other keys are ignored so
/// they do not get consumed before reaching other handlers.
pub fn handle_key(state: ProjectState, projects: &[Project], key: KeyEvent) -> ProjectState {
    // Nothing to navigate if the list is empty.
    if projects.is_empty() {
        return state;
    }

    match key.code {
        // Move down through the list, wrapping at the end.
        KeyCode::Down => {
            let cur  = state.selected.unwrap_or(0);
            let next = (cur + 1) % projects.len();
            ProjectState { selected: Some(next) }
        }
        // Move up, wrapping at the top.
        KeyCode::Up => {
            let cur  = state.selected.unwrap_or(0);
            let prev = cur.checked_sub(1).unwrap_or(projects.len() - 1);
            ProjectState { selected: Some(prev) }
        }
        _ => state,
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render the full projects screen into `area`.
///
/// Splits `area` into a 30 % project list (left) and a 70 % stats pane
/// (right). Pure function — draws from state, mutates nothing.
pub fn render(frame: &mut Frame, area: Rect, projects: &[Project], state: &ProjectState) {
    // 30/70 split mirrors the operator's mental model: the list is a narrow
    // index; the stats are the main reading surface.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    render_list(frame, cols[0], projects, state);
    render_stats(frame, cols[1], projects, state);
}

/// Render the scrollable project list pane.
///
/// The selected row is highlighted with `theme::selected()` (black on green).
/// All others use standard text brightness so the selection stands out clearly.
fn render_list(frame: &mut Frame, area: Rect, projects: &[Project], state: &ProjectState) {
    let lines: Vec<Line> = if projects.is_empty() {
        // Empty-state message so the pane is never blank.
        vec![Line::from(Span::styled(
            "  No projects found",
            theme::text_hint(),
        ))]
    } else {
        projects
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // Colour inversion alone is a sufficient selection indicator —
                // no cursor glyph needed.
                let style = if state.selected == Some(i) {
                    theme::selected()
                } else {
                    theme::text()
                };
                Line::from(Span::styled(format!("  {}", p.name), style))
            })
            .collect()
    };

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(Span::styled(" PROJECTS ", theme::text_active()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::border()),
        ),
        area,
    );
}

/// Render the statistics pane for the currently selected project.
///
/// Shows engagement metadata and artifact counters. Pressing Enter on a
/// highlighted project opens the project context menu. When no project is
/// selected the welcome prompt is shown instead.
fn render_stats(frame: &mut Frame, area: Rect, projects: &[Project], state: &ProjectState) {
    match state.selected.and_then(|i| projects.get(i)) {
        None => {
            // No project selected — show a welcome prompt.
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Welcome to Engos",
                        theme::text_active(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Please create a new project or continue an existing one.",
                        theme::text_hint(),
                    )),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(theme::border()),
                ),
                area,
            );
        }

        Some(p) => {
            // Pre-compute all owned strings up front so every span is
            // Cow::Owned ('static) and no borrow needs to outlive this arm.
            let collected   = p.artifacts_collected.to_string();
            let synthesized = p.artifacts_synthesized.to_string();
            // Use an em-dash for fields that have not been recorded yet so
            // the layout is consistent regardless of which fields are present.
            let last_opened   = p.last_opened.clone().unwrap_or_else(|| "—".to_string());
            let last_modified = p.last_modified.clone().unwrap_or_else(|| "—".to_string());

            let lines: Vec<Line> = vec![
                Line::from(""),
                // ── Engagement metadata ───────────────────────────────────
                stat_line("  Start Date          ", p.start_datetime.clone()),
                stat_line("  Specialist Model    ", p.specialist_model.clone()),
                Line::from(""),
                // ── Activity timestamps ───────────────────────────────────
                stat_line("  Last Opened         ", last_opened),
                stat_line("  Last Modified       ", last_modified),
                Line::from(""),
                // ── Artifact counters ─────────────────────────────────────
                stat_line("  Artifacts Collected    ", collected),
                stat_line("  Artifacts Synthesized  ", synthesized),
                Line::from(""),
            ];

            frame.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        // Title mirrors the project name so the operator knows
                        // which project they are reading.
                        .title(Span::styled(
                            format!("  {}  ", p.name),
                            theme::text_active(),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(theme::border()),
                ),
                area,
            );
        }
    }
}

/// Build a two-column statistic row: dim label on the left, bright value on
/// the right.
///
/// Both arguments are owned `String`s so the returned `Line` is `'static`
/// (all spans use `Cow::Owned`). This sidesteps lifetime complexity when
/// building lines inside a match arm that borrows from `projects`.
fn stat_line(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(label, theme::text_hint()),
        Span::styled(value, theme::text()),
    ])
}
