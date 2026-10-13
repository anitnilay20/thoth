/// Cross-platform channel for OS-dispatched file open requests.
///
/// On macOS, when the user double-clicks a file in Finder or uses "Open With",
/// the OS sends an Apple Event (`kAEOpenDocuments`) rather than passing the path
/// via `argv`. This module provides a global queue that the platform-specific
/// handler (e.g. `NSAppleEventManager` on macOS) pushes paths into, and that
/// `ThothApp::update()` drains each frame — the same pattern used by
/// `poll_plugin_http_results`.
///
/// On non-macOS platforms files arrive via `argv`, so the queue stays empty
/// unless another platform adds a similar handler in the future.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Global queue of file paths that the OS asked the running app to open.
static OPEN_REQUEST_QUEUE: OnceLock<Mutex<VecDeque<PathBuf>>> = OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<PathBuf>> {
    OPEN_REQUEST_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Enqueue a file path received from the OS (e.g. via Apple Event on macOS).
///
/// This is safe to call from any thread — the macOS AppKit callback fires on
/// the main thread, but `Mutex` ensures correctness regardless.
pub fn enqueue_open_request(path: PathBuf) {
    if let Ok(mut q) = queue().lock() {
        q.push_back(path);
    } else {
        eprintln!(
            "[thoth] warning: failed to enqueue file open request for {}",
            path.display()
        );
    }
}

/// Drain all pending OS-dispatched open requests.
///
/// Called once per frame from `ThothApp::update()`. Returns an empty `Vec` if
/// there are no pending requests. Non-blocking.
pub fn drain_open_requests() -> Vec<PathBuf> {
    if let Ok(mut q) = queue().lock() {
        q.drain(..).collect()
    } else {
        eprintln!("[thoth] warning: failed to drain file open requests (mutex poisoned)");
        Vec::new()
    }
}

/// Reset the queue — **test-only**.
///
/// `OnceLock` cannot be truly reset, but we can clear the inner `VecDeque`.
/// Call this at the start of each test to avoid cross-test pollution.
#[doc(hidden)]
pub fn _reset_for_test() {
    if let Ok(mut q) = queue().lock() {
        q.clear();
    }
}

// Unit tests for this module live in tests/file_association_tests.rs
// (integration tests) where they run with --test-threads=1 to avoid
// cross-test pollution of the global queue.
