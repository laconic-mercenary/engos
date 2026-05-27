//! Unit tests for the help screen state machine.
//!
//! Pure state-machine tests — no terminal, no filesystem, no timing.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use engos::help::{self, HelpAction, HelpState};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

#[test]
fn default_selects_first_topic() {
    // The content pane must never be empty — first topic is pre-selected.
    let state = HelpState::default();
    assert_eq!(state.selected, 0, "default state must select the first topic");
}

#[test]
fn down_advances_topic() {
    // With only one topic, Down wraps back to 0.
    // When more topics are added this test should be updated to verify advancement.
    let state = HelpState { selected: 0 };
    let (next, action) = help::handle_key(state, key(KeyCode::Down));

    // Wraps because there is currently only one topic.
    assert_eq!(next.selected, 0, "Down on the last topic must wrap to 0");
    assert!(action.is_none(), "navigation produces no action");
}

#[test]
fn up_wraps_from_first_topic() {
    let state = HelpState { selected: 0 };
    let (next, _) = help::handle_key(state, key(KeyCode::Up));

    // With one topic, Up also wraps back to 0.
    assert_eq!(next.selected, 0, "Up on the first topic must wrap to the last");
}

#[test]
fn esc_emits_close_action() {
    let state = HelpState::default();
    let (_, action) = help::handle_key(state, key(KeyCode::Esc));

    assert_eq!(action, Some(HelpAction::Close), "Esc must emit Close");
}

#[test]
fn unhandled_key_produces_no_action() {
    let state = HelpState::default();
    let (_, action) = help::handle_key(state, key(KeyCode::Char('x')));

    assert!(action.is_none(), "unhandled keys must produce no action");
}
