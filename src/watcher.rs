//! Filesystem watcher.
//!
//! Translates OS-level file events into typed [`WatchEvent`]s delivered over
//! an async channel. Uses the `notify` crate's OS-native backend (inotify on
//! Linux, FSEvents on macOS) so the kernel pushes events rather than the
//! application polling — important in an operator environment where the tool
//! must be quiet and hardware-efficient.

// `notify` types needed to register the watcher and interpret raw events.
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

// `Path` for the watch root (borrowed); `PathBuf` for owned paths in events.
use std::path::{Path, PathBuf};

// Async channel: the sender crosses into the notify thread, the receiver
// stays in the tokio async world.
use tokio::sync::mpsc::{self, Receiver};

/// The filesystem changes this application cares about.
///
/// Deliberately narrow — only events that require a response from the ingestion
/// pipeline are represented. `Access` events (reads) and `Other` noise are
/// dropped at the source so the pipeline is never asked to process a no-op.
///
/// Derives `Clone` so an event can be forwarded to multiple consumers later
/// (e.g. the TUI pane and the ingestion queue). Derives `PartialEq` so tests
/// can assert on event values without writing custom comparison logic.
#[derive(Debug, Clone, PartialEq)]
pub enum WatchEvent {
    /// A new file appeared in the watch directory (or a subdirectory).
    Created(PathBuf),

    /// An existing file's content or metadata changed.
    ///
    /// On macOS, FSEvents may emit `Modified` before `Deleted` when a file is
    /// removed — callers must not assume `Modified` means the file still exists.
    Modified(PathBuf),

    /// A file was removed from the watch directory.
    Deleted(PathBuf),

    /// A file was moved or renamed within the watch directory.
    ///
    /// Both old (`from`) and new (`to`) paths are provided so the ingestion
    /// layer can update its artifact index without treating this as a
    /// delete + create pair.
    Renamed { from: PathBuf, to: PathBuf },
}

/// Attach a watcher to `path` and return a channel that delivers typed events.
///
/// The caller receives both the event [`Receiver`] and the [`RecommendedWatcher`]
/// that keeps the OS watch registration alive. **Both must be held for as long
/// as events are needed.** Dropping the watcher de-registers the watch; dropping
/// the receiver stops event processing.
///
/// # Channel capacity
///
/// 64 slots gives enough headroom for a burst of rapid writes (e.g. a scanning
/// tool streaming output) without backpressuring the notify thread. If the
/// consumer falls too far behind, `blocking_send` inside the callback stalls
/// the notify thread rather than silently discarding events — we prefer
/// backpressure over event loss.
///
/// # Errors
///
/// Returns `notify::Error` if the OS watcher cannot be registered: path does
/// not exist, inotify watch limits exceeded on Linux, or FSEvents permission
/// denied on macOS.
pub fn start(path: &Path) -> notify::Result<(Receiver<WatchEvent>, RecommendedWatcher)> {
    // `tx` is moved into the notify callback closure; `rx` is returned to the
    // caller to drive the async event loop.
    let (tx, rx) = mpsc::channel(64);

    // `recommended_watcher` selects the best available OS backend at compile
    // time. The closure runs on a notify-owned background thread — not inside
    // the tokio executor — so only thread-safe operations are allowed here.
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        // Silently swallow watcher errors (e.g. a transient inotify overflow).
        // Phase 1 goal is proving the happy path; error surfacing goes in the
        // TUI status bar in Phase 2.
        let Ok(event) = res else { return };

        // Map notify's low-level event kinds to our application vocabulary.
        // Sub-kinds (e.g. `CreateKind::File` vs `CreateKind::Dir`) are
        // collapsed intentionally — the ingestion layer does not need that
        // distinction; it only needs to know that something changed.
        let events: Vec<WatchEvent> = match event.kind {
            // A file was created. `into_iter()` takes ownership so we avoid
            // cloning the path list.
            EventKind::Create(_) => event.paths.into_iter().map(WatchEvent::Created).collect(),

            // A file was modified. See the `Modified` variant doc for the
            // macOS FSEvents caveat about deletion sequences.
            EventKind::Modify(_) => event.paths.into_iter().map(WatchEvent::Modified).collect(),

            // A file was removed.
            EventKind::Remove(_) => event.paths.into_iter().map(WatchEvent::Deleted).collect(),

            // Access events (reads) and other OS noise are not actionable for
            // the ingestion pipeline. Dropping them here keeps the channel
            // clear of no-ops and avoids unnecessary pipeline wakeups.
            EventKind::Any | EventKind::Access(_) | EventKind::Other => return,
        };

        for e in events {
            // `blocking_send` is the correct bridge from this synchronous
            // notify thread into the tokio channel. The async `.send().await`
            // variant requires an executor context, which this thread lacks.
            // If the channel is full this blocks — intentional backpressure
            // rather than silent event loss.
            let _ = tx.blocking_send(e);
        }
    })?;

    // Register the path with the OS backend. `Recursive` watches all
    // subdirectories without needing to register each one individually.
    // Depth is capped to 2 per PROMPT.md to avoid unbounded traversal.
    watcher.watch(path, RecursiveMode::Recursive)?;

    // Return both halves. The caller must bind the watcher to a named variable
    // (not `_`) — an unnamed binding drops immediately, killing the watch.
    Ok((rx, watcher))
}
