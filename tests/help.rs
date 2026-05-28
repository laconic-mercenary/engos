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
fn down_advances_to_next_topic() {
    // There are now four topics; Down from 0 must move to 1.
    let state = HelpState { selected: 0 };
    let (next, action) = help::handle_key(state, key(KeyCode::Down));

    assert_eq!(next.selected, 1, "Down must advance from topic 0 to topic 1");
    assert!(action.is_none(), "navigation produces no action");
}

#[test]
fn down_wraps_from_last_topic() {
    use engos::help::TOPICS;
    let last = TOPICS.len() - 1;
    let state = HelpState { selected: last };
    let (next, _) = help::handle_key(state, key(KeyCode::Down));

    assert_eq!(next.selected, 0, "Down on the last topic must wrap to 0");
}

#[test]
fn up_wraps_from_first_topic() {
    use engos::help::TOPICS;
    let state = HelpState { selected: 0 };
    let (next, _) = help::handle_key(state, key(KeyCode::Up));

    // Up from the first topic wraps to the last.
    assert_eq!(next.selected, TOPICS.len() - 1, "Up on the first topic must wrap to the last");
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
