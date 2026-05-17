//! Integration tests for OS-dispatched file open requests (Issue #67).
//!
//! These tests exercise the channel plumbing that connects the OS-level
//! file-open handler (e.g. Apple Events on macOS) to the application's
//! `WindowState`. The actual AppKit FFI is not testable in CI, but the
//! channel contract — enqueue from any thread, drain from the UI thread —
//! is fully covered here.
//!
//! ## What IS tested
//!
//! - The `file_open_channel` queue (enqueue, drain, ordering, thread safety)
//! - `ThothApp::poll_os_open_requests()` draining the queue into `window_state`
//! - Existing argv path still works (regression guard)
//! - Second file replaces current file, error state is cleared
//!
//! ## What is NOT tested (requires a real macOS `.app` bundle)
//!
//! - `install_all_handlers()` registering the ObjC method on NSObject
//! - macOS Launch Services delivering `odoc` Apple Events on cold launch
//! - macOS delivering `application:openURLs:` on warm (already-running) launch
//! - Finder "Open With" and drag-to-Dock interactions
//!
//! These were manually verified with a `cargo packager --release` bundle.
//! See the Issue #67 PR description for reproduction steps.
//!
//! **Note:** These tests share a single global queue (`OnceLock`), so each
//! test must call `_reset_for_test()` at the start. Run with
//! `--test-threads=1` for reliable results.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use thoth::platform::file_open_channel;

/// Helper: reset the global queue and drain any leftovers to ensure a clean slate.
fn reset() {
    file_open_channel::_reset_for_test();
    // Double-drain in case of any race with parallel tests
    let _ = file_open_channel::drain_open_requests();
}

/// Serialize all tests in this file to avoid races on the shared global queue.
fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap()
}

// ---------------------------------------------------------------------------
// Channel-level tests
// ---------------------------------------------------------------------------

#[test]
fn open_request_queue_round_trips_path() {
    let _guard = test_guard();
    reset();
    let p = PathBuf::from("/tmp/sample.json");
    file_open_channel::enqueue_open_request(p.clone());
    let drained = file_open_channel::drain_open_requests();
    assert_eq!(drained, vec![p]);
}

#[test]
fn open_request_queue_drains_completely() {
    let _guard = test_guard();
    reset();
    file_open_channel::enqueue_open_request(PathBuf::from("/tmp/a.json"));
    let _ = file_open_channel::drain_open_requests();
    assert!(
        file_open_channel::drain_open_requests().is_empty(),
        "queue must be empty after drain"
    );
}

#[test]
fn open_request_queue_preserves_order() {
    let _guard = test_guard();
    reset();
    for i in 0..5 {
        file_open_channel::enqueue_open_request(PathBuf::from(format!("/tmp/file{i}.json")));
    }
    let drained = file_open_channel::drain_open_requests();
    assert_eq!(drained.len(), 5);
    assert_eq!(drained[0], PathBuf::from("/tmp/file0.json"));
    assert_eq!(drained[4], PathBuf::from("/tmp/file4.json"));
}

#[test]
fn open_request_queue_is_thread_safe() {
    let _guard = test_guard();
    reset();
    let handles: Vec<_> = (0..16)
        .map(|i| {
            std::thread::spawn(move || {
                file_open_channel::enqueue_open_request(PathBuf::from(format!(
                    "/tmp/thread{i}.json"
                )));
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let drained = file_open_channel::drain_open_requests();
    assert_eq!(drained.len(), 16);
}

#[test]
fn open_request_queue_returns_empty_when_nothing_enqueued() {
    let _guard = test_guard();
    reset();
    assert!(file_open_channel::drain_open_requests().is_empty());
}

#[test]
fn multiple_drains_only_return_new_items() {
    let _guard = test_guard();
    reset();

    // First batch
    file_open_channel::enqueue_open_request(PathBuf::from("/tmp/first.json"));
    let batch1 = file_open_channel::drain_open_requests();
    assert_eq!(batch1.len(), 1);

    // Second batch
    file_open_channel::enqueue_open_request(PathBuf::from("/tmp/second.json"));
    file_open_channel::enqueue_open_request(PathBuf::from("/tmp/third.json"));
    let batch2 = file_open_channel::drain_open_requests();
    assert_eq!(batch2.len(), 2);
    assert_eq!(batch2[0], PathBuf::from("/tmp/second.json"));
    assert_eq!(batch2[1], PathBuf::from("/tmp/third.json"));
}

// ---------------------------------------------------------------------------
// App-level tests: ThothApp must drain the channel into window_state
// ---------------------------------------------------------------------------

use thoth::app::ThothApp;
use thoth::settings::Settings;

/// Helper: create a temporary JSON file and return its path.
fn make_temp_json_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("thoth_test_file_assoc");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, r#"{"test": true}"#).unwrap();
    path
}

#[test]
fn thoth_app_picks_up_os_dispatched_file() {
    let _guard = test_guard();
    reset();

    // Launch app with no file (simulates Finder-launched empty window)
    let mut app = ThothApp::new(Settings::default(), None);
    assert!(app.window_state.file_path.is_none());

    // Simulate macOS dispatching a file via Apple Event
    let path = make_temp_json_file("os_dispatch.json");
    file_open_channel::enqueue_open_request(path.clone());

    // Drive the OS-dispatch drain (this method must exist for the fix to work)
    app.poll_os_open_requests();

    assert_eq!(
        app.window_state.file_path.as_deref(),
        Some(path.as_path()),
        "poll_os_open_requests must set window_state.file_path"
    );
}

#[test]
fn argv_path_still_loads_on_construction() {
    let _guard = test_guard();
    // This guards against regressions: the existing CLI flow must keep working.
    let path = make_temp_json_file("argv_test.json");
    let app = ThothApp::new(Settings::default(), Some(path.clone()));
    assert_eq!(
        app.window_state.file_path.as_deref(),
        Some(path.as_path()),
        "file_to_open passed via argv must set window_state.file_path in constructor"
    );
}

#[test]
fn second_os_dispatch_replaces_current_file() {
    let _guard = test_guard();
    reset();

    let p1 = make_temp_json_file("first_file.json");
    let p2 = make_temp_json_file("second_file.json");

    // App launched with first file via argv
    let mut app = ThothApp::new(Settings::default(), Some(p1.clone()));
    assert_eq!(app.window_state.file_path.as_deref(), Some(p1.as_path()));

    // OS dispatches a second file (e.g. user double-clicks another .json in Finder)
    file_open_channel::enqueue_open_request(p2.clone());
    app.poll_os_open_requests();

    assert_eq!(
        app.window_state.file_path.as_deref(),
        Some(p2.as_path()),
        "OS-dispatched file must replace the currently loaded file"
    );
}

#[test]
fn os_dispatch_clears_previous_error() {
    let _guard = test_guard();
    reset();

    let mut app = ThothApp::new(Settings::default(), None);
    // Simulate an error state
    app.window_state.error = Some(thoth::error::ThothError::Unknown {
        message: "test error".to_string(),
    });

    let path = make_temp_json_file("clear_error.json");
    file_open_channel::enqueue_open_request(path.clone());
    app.poll_os_open_requests();

    assert!(
        app.window_state.error.is_none(),
        "OS-dispatched file must clear any previous error"
    );
    assert_eq!(app.window_state.file_path.as_deref(), Some(path.as_path()));
}
