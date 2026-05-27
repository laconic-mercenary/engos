//! Visual theme — the single source of truth for every colour used in the UI.
//!
//! All colour values are named constants. No hex literals or `Color::Rgb` calls
//! should appear anywhere else in the codebase — change a colour here and it
//! propagates everywhere automatically.
//!
//! # The palette
//!
//! Four brightness levels of the same green hue, matching the phosphor glow
//! of the original Matrix rain sequence (#00FF41). Each level has a clear role:
//!
//! | Constant       | Role                                          |
//! |----------------|-----------------------------------------------|
//! | `GREEN`        | Primary text, focused borders, selection bg  |
//! | `GREEN_DIM`    | Standard borders, secondary text             |
//! | `GREEN_DARK`   | Inactive borders, subtle hints               |
//! | `GREEN_FAINT`  | Barely-visible decoration, splash border     |
//! | `BLACK`        | Foreground on highlighted items (inverted)   |

use ratatui::style::{Color, Modifier, Style};

// ── Colour constants ──────────────────────────────────────────────────────────

/// Matrix phosphor green — #00FF41.
/// Use for active text, focused widget borders, and selection backgrounds.
pub const GREEN: Color = Color::Rgb(0, 255, 65);

/// Medium green — softer than `GREEN` for extended reading.
/// Use for standard body text and normal-state borders.
pub const GREEN_DIM: Color = Color::Rgb(0, 200, 50);

/// Dark green — recedes into the background.
/// Use for inactive borders and low-emphasis hints.
pub const GREEN_DARK: Color = Color::Rgb(0, 100, 20);

/// Very faint green — barely perceptible glow.
/// Use for decorative borders (e.g. the splash box) that should not draw
/// attention away from the content.
pub const GREEN_FAINT: Color = Color::Rgb(0, 50, 10);

/// Pure black — the foreground colour when an item is highlighted.
/// Inverted against `GREEN` gives maximum contrast for selections.
pub const BLACK: Color = Color::Black;

// ── Style constructors ────────────────────────────────────────────────────────
//
// These are free functions, not methods — call sites compose styles from these
// building blocks rather than reaching for raw Color values.

/// Primary text — medium green, readable on a black background.
pub fn text() -> Style {
    Style::default().fg(GREEN_DIM)
}

/// Focused / active text — bright green, draws the eye.
pub fn text_active() -> Style {
    Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
}

/// Dim hint text — dark green, present but not demanding.
pub fn text_hint() -> Style {
    Style::default().fg(GREEN_DARK)
}

/// Selected item — black text on bright green; maximum contrast.
/// Used for the highlighted row in dropdowns and lists.
pub fn selected() -> Style {
    Style::default().fg(BLACK).bg(GREEN)
}

/// Standard widget border — dark green, visible but not dominant.
pub fn border() -> Style {
    Style::default().fg(GREEN_DARK)
}

/// Focused widget border — bright green, clearly signals active state.
pub fn border_active() -> Style {
    Style::default().fg(GREEN)
}

/// Splash screen border — very faint green, frames without competing.
pub fn border_splash() -> Style {
    Style::default().fg(GREEN_FAINT)
}

/// Validation error — red text, universally understood as an error signal.
///
/// The only non-green colour in the palette; reserved strictly for errors so
/// operators notice it immediately without it becoming visual noise.
pub fn error() -> Style {
    Style::default().fg(Color::Red)
}
