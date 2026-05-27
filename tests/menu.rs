//! Unit tests for the menu state machine.
//!
//! The menu module is a pure state machine — given a state and a keypress it
//! produces a new state and an optional action. Pure functions are the easiest
//! things to test: no filesystem, no terminal, no timing.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use engos::menu::{self, MenuAction, MenuState};

/// Construct a bare keypress event from a [`KeyCode`].
///
/// Crossterm requires all five fields to be set; this helper fills in the
/// boilerplate so tests stay focused on the code under test.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

// ── focus() ───────────────────────────────────────────────────────────────────

#[test]
fn focus_sets_focused_and_clears_open() {
    // Start with the menu open on a sub-item to verify focus resets everything.
    let state = MenuState { focused: false, item: 0, open: true, sub: 1 };
    let focused = menu::focus(state);

    assert!(focused.focused, "focus() must set focused = true");
    assert!(!focused.open,   "focus() must close any open dropdown");
    assert_eq!(focused.sub, 0, "focus() must reset the sub-item index");
}

// ── handle_key — bar focused, dropdown closed ────────────────────────────────

#[test]
fn down_opens_dropdown_for_item_with_sub_items() {
    // File (item 0) has sub-items — Down must open its dropdown.
    let state = menu::focus(MenuState::default()); // item = 0 (File)

    let (next, action) = menu::handle_key(state, key(KeyCode::Down));

    assert!(next.open,        "Down on File must open its dropdown");
    assert!(action.is_none(), "opening the dropdown produces no action");
}

#[test]
fn enter_opens_dropdown_for_item_with_sub_items() {
    // File (item 0) has sub-items — Enter must open its dropdown.
    let state = menu::focus(MenuState::default()); // item = 0 (File)

    let (next, action) = menu::handle_key(state, key(KeyCode::Enter));

    assert!(next.open,        "Enter on File must open its dropdown");
    assert!(action.is_none(), "opening the dropdown produces no action");
}

#[test]
fn enter_on_direct_action_item_fires_without_dropdown() {
    // Help (item 1) has no sub-items — Enter must fire its action directly,
    // without going through a dropdown at all.
    let state = MenuState { focused: true, item: 1, open: false, sub: 0 };

    let (next, action) = menu::handle_key(state, key(KeyCode::Enter));

    assert!(!next.open,    "direct-action item must not open a dropdown");
    assert!(!next.focused, "direct-action item must unfocus the menu");
    assert_eq!(action, Some(MenuAction::Help(0)), "Enter on Help must emit Help(0)");
}

#[test]
fn down_on_direct_action_item_fires_without_dropdown() {
    // Down has the same behaviour as Enter for direct-action items.
    let state = MenuState { focused: true, item: 1, open: false, sub: 0 };

    let (_, action) = menu::handle_key(state, key(KeyCode::Down));

    assert_eq!(action, Some(MenuAction::Help(0)), "Down on Help must also emit Help(0)");
}

#[test]
fn esc_on_bar_unfocuses_menu() {
    let state = menu::focus(MenuState::default());

    let (next, action) = menu::handle_key(state, key(KeyCode::Esc));

    assert!(!next.focused,    "Esc on the bar must unfocus the menu");
    assert!(action.is_none(), "unfocusing produces no action");
}

#[test]
fn right_advances_from_file_to_help() {
    // File is item 0; pressing Right should move to Help (item 1).
    let state = menu::focus(MenuState::default()); // item = 0 (File)

    let (next, _) = menu::handle_key(state, key(KeyCode::Right));

    assert_eq!(next.item, 1, "Right from File must advance to Help");
}

#[test]
fn right_wraps_from_last_item_to_first() {
    // Help is the last item; pressing Right must wrap back to File (item 0).
    let state = MenuState { focused: true, item: 1, open: false, sub: 0 }; // Help

    let (next, _) = menu::handle_key(state, key(KeyCode::Right));

    assert_eq!(next.item, 0, "Right from the last item must wrap to item 0");
}

// ── handle_key — dropdown open ────────────────────────────────────────────────

#[test]
fn down_moves_to_next_sub_item() {
    // Open dropdown, start on sub-item 0 (New Project).
    let state = MenuState { focused: true, item: 0, open: true, sub: 0 };

    let (next, action) = menu::handle_key(state, key(KeyCode::Down));

    assert_eq!(next.sub, 1,   "Down must advance to sub-item 1 (Quit)");
    assert!(action.is_none(), "navigation produces no action");
}

#[test]
fn up_wraps_from_first_sub_item() {
    // On the first sub-item, Up must wrap to the last one.
    let state = MenuState { focused: true, item: 0, open: true, sub: 0 };

    let (next, _) = menu::handle_key(state, key(KeyCode::Up));

    // File has 2 items (0 = New Project, 1 = Quit) — wrap from 0 to 1.
    assert_eq!(next.sub, 1, "Up must wrap from sub-item 0 to the last item");
}

#[test]
fn down_wraps_from_last_sub_item() {
    // On the last sub-item, Down must wrap back to 0.
    let state = MenuState { focused: true, item: 0, open: true, sub: 1 };

    let (next, _) = menu::handle_key(state, key(KeyCode::Down));

    assert_eq!(next.sub, 0, "Down must wrap from the last sub-item to 0");
}

#[test]
fn enter_on_new_project_emits_action_and_closes() {
    let state = MenuState { focused: true, item: 0, open: true, sub: 0 };

    let (next, action) = menu::handle_key(state, key(KeyCode::Enter));

    assert_eq!(action, Some(MenuAction::NewProject), "Enter on item 0 must emit NewProject");
    assert!(!next.open,    "confirming an item must close the dropdown");
    assert!(!next.focused, "confirming an item must unfocus the menu");
}

#[test]
fn enter_on_quit_emits_action_and_closes() {
    let state = MenuState { focused: true, item: 0, open: true, sub: 1 };

    let (next, action) = menu::handle_key(state, key(KeyCode::Enter));

    assert_eq!(action, Some(MenuAction::Quit), "Enter on item 1 must emit Quit");
    assert!(!next.open,    "confirming an item must close the dropdown");
    assert!(!next.focused, "confirming an item must unfocus the menu");
}

#[test]
fn esc_closes_dropdown_but_keeps_bar_focused() {
    // Esc from an open dropdown should close it but leave the bar focused so
    // the operator can navigate to a different top-level item.
    let state = MenuState { focused: true, item: 0, open: true, sub: 1 };

    let (next, action) = menu::handle_key(state, key(KeyCode::Esc));

    assert!(!next.open,   "Esc must close the dropdown");
    assert!(next.focused, "Esc must keep the bar focused");
    assert_eq!(next.sub, 0, "Esc must reset the sub-item index");
    assert!(action.is_none(), "Esc produces no action");
}
