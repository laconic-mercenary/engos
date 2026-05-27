//! Integration tests for the filesystem watcher.
//!
//! These tests exercise the watcher against a real (temporary) directory on
//! the host filesystem. No mocking — we want to catch OS-specific event
//! behaviour that a mock would hide.
//!
//! # macOS FSEvents caveats
//!
//! FSEvents is asynchronous and coalescing: it may deliver events with variable
//! latency, emit `Modified` before `Deleted` in a removal sequence, and replay
//! events for files that existed before the watcher started. Every helper in
//! this module is designed around these properties so tests remain deterministic.

// Import the watcher module and the event type we are testing.
use engos::watcher::{self, WatchEvent};

// Standard library filesystem operations used to trigger the events under test.
use std::fs;

// `TempDir` creates a unique temporary directory for each test and removes it
// when the value is dropped — no manual cleanup needed.
use tempfile::TempDir;

// `Receiver` is the async channel end returned by the watcher.
use tokio::sync::mpsc::Receiver;

// `timeout` wraps a future with a deadline so a missing event fails fast
// instead of hanging the test suite indefinitely.
use tokio::time::{Duration, timeout};

/// Create a fresh temporary directory for a test.
///
/// Each test gets its own directory so tests can run in parallel without
/// interfering with each other's events.
fn setup() -> TempDir {
    TempDir::new().expect("failed to create temp dir")
}

/// Wait for the next event on the channel, failing if none arrives within 3 s.
///
/// The 3-second deadline is long enough for slow CI runners but short enough
/// that a genuinely missing event does not stall the suite for a noticeable
/// amount of time.
async fn next_event(rx: &mut Receiver<WatchEvent>) -> WatchEvent {
    timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("timed out waiting for watcher event")
        .expect("watcher channel closed unexpectedly")
}

/// Consume events from the channel until one satisfies `pred`, discarding
/// all non-matching events that arrive first.
///
/// This is the correct way to assert on events because FSEvents on macOS can
/// interleave `Modified` events within `Create` and `Delete` sequences. If a
/// test asserted on `next_event` directly it would fail whenever macOS emits
/// an extra event before the expected one. `find_event` absorbs the noise.
async fn find_event<F>(rx: &mut Receiver<WatchEvent>, pred: F) -> WatchEvent
where
    F: Fn(&WatchEvent) -> bool,
{
    loop {
        // Block until any event arrives, then check whether it matches.
        let e = next_event(rx).await;
        if pred(&e) {
            // This is the event we were waiting for — return it.
            return e;
        }
        // Not a match; discard and wait for the next one.
    }
}

/// Start the watcher and block until it is confirmed live, returning a clean
/// receiver with no buffered events.
///
/// # Why a sentinel file?
///
/// Simply sleeping after `watcher::start` is not reliable: FSEvents on macOS
/// has variable latency (typically 50–300 ms) and can replay events for files
/// that existed before the watch started. Without a liveness signal we cannot
/// know when it is safe to begin the test.
///
/// The sentinel approach: write a known file, wait until its `Created` event
/// arrives. That event is proof that FSEvents is live and we are seeing
/// real-time events. We then drain the channel of all buffered noise (the
/// sentinel's deletion, any FSEvents duplicates) before returning.
async fn start_ready(dir: &TempDir) -> (Receiver<WatchEvent>, notify::RecommendedWatcher) {
    // Start the watcher. The `RecommendedWatcher` must be returned and held
    // alive by the caller — dropping it stops the watch.
    let (mut rx, w) = watcher::start(dir.path()).unwrap();

    // Brief pause to let the OS backend finish its initialisation before we
    // write the sentinel. Without this the sentinel write can race the watcher
    // setup and its event may never arrive.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Write the sentinel. The empty content is intentional — we only care that
    // the file exists long enough to trigger a `Created` event.
    let sentinel = dir.path().join("_ready");
    fs::write(&sentinel, "").unwrap();

    // Block until the sentinel's `Created` event arrives.
    // Any other events that arrive first are discarded by `find_event`.
    find_event(&mut rx, |e| {
        matches!(e, WatchEvent::Created(p) if p.ends_with("_ready"))
    })
    .await;

    // Remove the sentinel now that we have confirmed liveness. If removal
    // fails (e.g. already gone) we do not care — `ok()` discards the error.
    let _ = fs::remove_file(&sentinel);

    // Drain the channel until it is quiet for 300 ms. This clears:
    // - the sentinel's `Deleted` event
    // - any FSEvents duplicate or metadata events for the sentinel
    // After this loop the receiver is clean: any subsequent event was caused
    // by the test itself.
    while timeout(Duration::from_millis(300), rx.recv()).await.is_ok() {}

    (rx, w)
}

/// Watcher reports a `Created` event when a new file appears in the watched directory.
#[tokio::test]
async fn detects_file_creation() {
    let dir = setup();
    let (mut rx, _w) = start_ready(&dir).await;

    // Write a new file to trigger a `Created` event.
    fs::write(dir.path().join("notes.txt"), "initial content").unwrap();

    // Wait for the specific `Created(notes.txt)` event, ignoring any
    // FSEvents noise that arrives first.
    let event = find_event(&mut rx, |e| {
        matches!(e, WatchEvent::Created(p) if p.ends_with("notes.txt"))
    })
    .await;

    // Confirm the matched event has the right variant.
    assert!(matches!(event, WatchEvent::Created(_)));
}

/// Watcher reports a `Modified` event when an existing file's content changes.
#[tokio::test]
async fn detects_file_modification() {
    let dir = setup();
    let (mut rx, _w) = start_ready(&dir).await;

    let file = dir.path().join("capture.log");

    // Create the file through the watcher so its `Created` event is in the
    // channel. We must consume that event before modifying the file —
    // otherwise `find_event` below might match the `Created` instead of
    // the `Modified` we actually want to test.
    fs::write(&file, "line 1").unwrap();
    find_event(&mut rx, |e| {
        matches!(e, WatchEvent::Created(p) if p.ends_with("capture.log"))
    })
    .await;

    // Overwrite with new content to trigger a `Modified` event.
    fs::write(&file, "line 1\nline 2").unwrap();

    let event = find_event(&mut rx, |e| {
        matches!(e, WatchEvent::Modified(p) if p.ends_with("capture.log"))
    })
    .await;

    assert!(matches!(event, WatchEvent::Modified(_)));
}

/// Watcher reports a `Deleted` event when a file is removed.
///
/// On macOS, FSEvents can emit a `Modified` event immediately before `Deleted`
/// in a removal sequence (updating metadata before the final unlink). The test
/// uses `find_event` to skip the `Modified` and wait for the `Deleted`.
#[tokio::test]
async fn detects_file_deletion() {
    let dir = setup();
    let (mut rx, _w) = start_ready(&dir).await;

    let file = dir.path().join("tmp.txt");

    // Create the file through the watcher and wait for its `Created` event
    // before removing it — same reasoning as in `detects_file_modification`.
    fs::write(&file, "bye").unwrap();
    find_event(&mut rx, |e| {
        matches!(e, WatchEvent::Created(p) if p.ends_with("tmp.txt"))
    })
    .await;

    // Remove the file to trigger a `Deleted` event.
    fs::remove_file(&file).unwrap();

    let event = find_event(&mut rx, |e| {
        matches!(e, WatchEvent::Deleted(p) if p.ends_with("tmp.txt"))
    })
    .await;

    assert!(matches!(event, WatchEvent::Deleted(_)));
}

/// Watcher does not report events for files written outside its watch directory.
///
/// This guards against a misconfigured recursive watch accidentally picking up
/// events from sibling directories on the same filesystem.
#[tokio::test]
async fn does_not_report_files_outside_watch_dir() {
    let watched = setup();
    // A completely separate temporary directory on the same filesystem.
    let other = setup();

    let (mut rx, _w) = start_ready(&watched).await;

    // Write to the OTHER directory — the watcher is not registered there,
    // so no event should arrive on `rx`.
    fs::write(other.path().join("noise.txt"), "unrelated").unwrap();

    // Give FSEvents a generous window; if anything arrives it is a bug.
    let result = timeout(Duration::from_millis(400), rx.recv()).await;
    assert!(
        result.is_err(),
        "received an unexpected event from outside the watch directory"
    );
}
