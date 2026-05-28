//! Visual theme — the single source of truth for every colour and style in the UI.
//!
//! # Palette system
//!
//! A global atomic stores the active palette (Green / Blue / White / Red).
//! All style constructors read it at call time, so [`set_palette`] takes effect
//! on the very next render frame with no other code changes.
//!
//! # Style constructors
//!
//! Call these fresh each render frame — do not cache the returned `Style`.

use std::sync::atomic::{AtomicU8, Ordering};

use ratatui::style::{Color, Modifier, Style};

// ── Palette ───────────────────────────────────────────────────────────────────

/// One of the four supported colour palettes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Palette {
    Green = 0,
    Blue  = 1,
    White = 2,
    Red   = 3,
}

impl Palette {
    /// Map a raw index back to a variant. Out-of-range values fall back to Green.
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Palette::Blue,
            2 => Palette::White,
            3 => Palette::Red,
            _ => Palette::Green,
        }
    }
}

/// Human-readable names in display order, index-aligned with `Palette as usize`.
pub const PALETTE_NAMES: &[&str] = &["Green", "Blue", "White", "Red"];

/// Active palette — written once at startup and whenever the operator changes
/// it in Options. `Relaxed` ordering is sufficient; no data depends on this
/// value across threads.
static CURRENT_PALETTE: AtomicU8 = AtomicU8::new(0);

/// Switch to the given palette. Takes effect on the next render frame.
pub fn set_palette(p: Palette) {
    CURRENT_PALETTE.store(p as u8, Ordering::Relaxed);
}

/// Return the currently active palette.
pub fn current_palette() -> Palette {
    Palette::from_index(CURRENT_PALETTE.load(Ordering::Relaxed) as usize)
}

// ── Colour constants — one set per palette ────────────────────────────────────
//
// Four brightness levels with consistent roles across every palette:
//   BRIGHT → primary accent / active text / selection background
//   NORMAL → readable body text
//   DIM    → borders, inactive elements, secondary hints
//   FAINT  → barely-visible decoration (splash border, etc.)

// Green — Matrix phosphor (default)
const G_BRIGHT: Color = Color::Rgb(  0, 255,  65);
const G_NORMAL: Color = Color::Rgb(  0, 200,  50);
const G_DIM:    Color = Color::Rgb(  0, 100,  20);
const G_FAINT:  Color = Color::Rgb(  0,  50,  10);

// Blue — deep-sky blue
const B_BRIGHT: Color = Color::Rgb(  0, 191, 255);
const B_NORMAL: Color = Color::Rgb(  0, 150, 200);
const B_DIM:    Color = Color::Rgb(  1,  79, 134);
const B_FAINT:  Color = Color::Rgb(  0,  37,  58);

// White — greyscale
const W_BRIGHT: Color = Color::Rgb(255, 255, 255);
const W_NORMAL: Color = Color::Rgb(160, 160, 160);
const W_DIM:    Color = Color::Rgb( 80,  80,  80);
const W_FAINT:  Color = Color::Rgb( 32,  32,  32);

// Red — warm red
const R_BRIGHT: Color = Color::Rgb(255,  68,  68);
const R_NORMAL: Color = Color::Rgb(200,  40,  40);
const R_DIM:    Color = Color::Rgb(120,   0,   0);
const R_FAINT:  Color = Color::Rgb( 48,   0,   0);

// Black — foreground for inverted items; palette-independent.
const BLACK: Color = Color::Black;

// ── Palette-derived colour accessors ─────────────────────────────────────────

fn bright() -> Color {
    match current_palette() {
        Palette::Green => G_BRIGHT,
        Palette::Blue  => B_BRIGHT,
        Palette::White => W_BRIGHT,
        Palette::Red   => R_BRIGHT,
    }
}

fn normal() -> Color {
    match current_palette() {
        Palette::Green => G_NORMAL,
        Palette::Blue  => B_NORMAL,
        Palette::White => W_NORMAL,
        Palette::Red   => R_NORMAL,
    }
}

fn dim() -> Color {
    match current_palette() {
        Palette::Green => G_DIM,
        Palette::Blue  => B_DIM,
        Palette::White => W_DIM,
        Palette::Red   => R_DIM,
    }
}

fn faint() -> Color {
    match current_palette() {
        Palette::Green => G_FAINT,
        Palette::Blue  => B_FAINT,
        Palette::White => W_FAINT,
        Palette::Red   => R_FAINT,
    }
}

// ── Style constructors ────────────────────────────────────────────────────────

/// Primary text — readable body text.
pub fn text() -> Style {
    Style::default().fg(normal())
}

/// Focused / active text — draws the eye; used for titles and active items.
pub fn text_active() -> Style {
    Style::default().fg(bright()).add_modifier(Modifier::BOLD)
}

/// Dim hint text — present but receding; used for labels and secondary info.
pub fn text_hint() -> Style {
    Style::default().fg(dim())
}

/// Selected item — inverted: black text on bright accent background.
pub fn selected() -> Style {
    Style::default().fg(BLACK).bg(bright())
}

/// Standard widget border — dim; visible but not dominant.
pub fn border() -> Style {
    Style::default().fg(dim())
}

/// Focused widget border — bright; clearly signals the active widget.
pub fn border_active() -> Style {
    Style::default().fg(bright())
}

/// Splash screen border — very faint; frames without competing.
pub fn border_splash() -> Style {
    Style::default().fg(faint())
}

/// Validation error — always a distinct red regardless of the active palette
/// so errors are immediately recognisable against any accent colour.
pub fn error() -> Style {
    Style::default().fg(Color::Rgb(255, 70, 70))
}

// ── Palette utilities ─────────────────────────────────────────────────────────

/// Return the bright accent colour for a specific palette without changing the
/// active one. Used by the options screen to render colour swatches.
pub fn palette_bright_color(p: Palette) -> Color {
    match p {
        Palette::Green => G_BRIGHT,
        Palette::Blue  => B_BRIGHT,
        Palette::White => W_BRIGHT,
        Palette::Red   => R_BRIGHT,
    }
}
