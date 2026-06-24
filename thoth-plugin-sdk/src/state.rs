//! Ergonomic plugin-global state.
//!
//! Thoth plugins are single-threaded WebAssembly modules, yet the WIT export
//! functions are free functions with no `self`, so every plugin ends up storing
//! its state in a `thread_local! { static STATE: RefCell<Option<T>> }` and
//! repeating the borrow dance at every call site.
//!
//! [`PluginState`] collapses that into a single `static`:
//!
//! ```
//! use thoth_plugin_sdk::state::PluginState;
//!
//! #[derive(Default)]
//! struct MyState {
//!     url: String,
//!     hits: u32,
//! }
//!
//! static STATE: PluginState<MyState> = PluginState::new();
//!
//! STATE.with_mut(|s| {
//!     s.url = "https://example.com".into();
//!     s.hits += 1;
//! });
//!
//! let url = STATE.with(|s| s.url.clone());
//! assert_eq!(url, "https://example.com");
//! assert_eq!(STATE.with(|s| s.hits), 1);
//!
//! STATE.reset(); // e.g. from `on_close`
//! assert!(!STATE.is_initialised());
//! ```

use std::cell::RefCell;

/// A plugin-global, lazily-initialised state cell usable as a `static`.
///
/// The value is created on first access via [`Default`] (or supplied up front
/// with [`set`](PluginState::set)). Access is mediated by closures so the
/// borrow never escapes.
///
/// # Threading
///
/// Thoth plugins run single-threaded on `wasm32`, so this type is declared
/// [`Sync`] to be usable in a `static`. It is **not** safe to share across
/// threads; do not use it from a multi-threaded host context.
pub struct PluginState<T> {
    cell: RefCell<Option<T>>,
}

// SAFETY: Thoth plugins execute single-threaded on wasm32; the cell is never
// accessed concurrently. This bound only exists so the value can live in a
// `static`.
unsafe impl<T> Sync for PluginState<T> {}

impl<T> PluginState<T> {
    /// Create an empty state cell. `const`, so it can initialise a `static`.
    pub const fn new() -> Self {
        Self {
            cell: RefCell::new(None),
        }
    }

    /// Replace the stored value, initialising it if necessary.
    pub fn set(&self, value: T) {
        *self.cell.borrow_mut() = Some(value);
    }

    /// Drop the stored value. The next [`with`](Self::with) /
    /// [`with_mut`](Self::with_mut) re-initialises it from [`Default`].
    pub fn reset(&self) {
        *self.cell.borrow_mut() = None;
    }

    /// Whether a value is currently stored.
    pub fn is_initialised(&self) -> bool {
        self.cell.borrow().is_some()
    }

    /// Run `f` with a shared reference *only if* a value is stored, returning
    /// `None` otherwise. Use this (with [`set`](Self::set)) when "absent" is a
    /// meaningful state — e.g. a resource that hasn't been opened yet — and you
    /// don't want [`Default`]-initialisation.
    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.cell.borrow().as_ref().map(f)
    }

    /// Mutable counterpart to [`try_with`](Self::try_with).
    pub fn try_with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.cell.borrow_mut().as_mut().map(f)
    }
}

impl<T: Default> PluginState<T> {
    /// Run `f` with a shared reference to the state, initialising it from
    /// [`Default`] if it has not been set yet.
    ///
    /// Nested [`with`](Self::with) calls are fine; a nested
    /// [`with_mut`](Self::with_mut) while a `with` borrow is held will panic
    /// (the same rule as [`RefCell`]).
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        if self.cell.borrow().is_none() {
            *self.cell.borrow_mut() = Some(T::default());
        }
        let guard = self.cell.borrow();
        f(guard.as_ref().expect("just initialised"))
    }

    /// Run `f` with a mutable reference to the state, initialising it from
    /// [`Default`] if it has not been set yet.
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut guard = self.cell.borrow_mut();
        f(guard.get_or_insert_with(T::default))
    }

    /// Return a clone of the current state, initialising from [`Default`] if
    /// unset.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(|s| s.clone())
    }
}

impl<T> Default for PluginState<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::PluginState;

    #[derive(Default, Clone, PartialEq, Debug)]
    struct Counter {
        count: u32,
        name: String,
    }

    // ── is_initialised / set / reset ─────────────────────────────────────────

    #[test]
    fn new_is_not_initialised() {
        let state: PluginState<Counter> = PluginState::new();
        assert!(!state.is_initialised());
    }

    #[test]
    fn set_marks_initialised() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 1, name: "a".into() });
        assert!(state.is_initialised());
    }

    #[test]
    fn reset_clears_value() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 5, name: "x".into() });
        state.reset();
        assert!(!state.is_initialised());
    }

    #[test]
    fn reset_on_empty_is_idempotent() {
        let state: PluginState<Counter> = PluginState::new();
        state.reset(); // should not panic
        assert!(!state.is_initialised());
    }

    #[test]
    fn set_replaces_existing_value() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 1, name: "first".into() });
        state.set(Counter { count: 2, name: "second".into() });
        let name = state.try_with(|s| s.name.clone()).unwrap();
        assert_eq!(name, "second");
    }

    // ── try_with / try_with_mut ───────────────────────────────────────────────

    #[test]
    fn try_with_returns_none_when_empty() {
        let state: PluginState<Counter> = PluginState::new();
        assert!(state.try_with(|_| ()).is_none());
    }

    #[test]
    fn try_with_returns_some_when_set() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 42, name: "hello".into() });
        let count = state.try_with(|s| s.count);
        assert_eq!(count, Some(42));
    }

    #[test]
    fn try_with_does_not_initialise() {
        let state: PluginState<Counter> = PluginState::new();
        let _ = state.try_with(|s| s.count);
        assert!(!state.is_initialised());
    }

    #[test]
    fn try_with_mut_returns_none_when_empty() {
        let state: PluginState<Counter> = PluginState::new();
        assert!(state.try_with_mut(|_| ()).is_none());
    }

    #[test]
    fn try_with_mut_mutates_when_set() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 0, name: String::new() });
        state.try_with_mut(|s| s.count += 10);
        let count = state.try_with(|s| s.count).unwrap();
        assert_eq!(count, 10);
    }

    // ── with (auto-initialises from Default) ──────────────────────────────────

    #[test]
    fn with_auto_initialises_from_default() {
        let state: PluginState<Counter> = PluginState::new();
        let count = state.with(|s| s.count);
        assert_eq!(count, 0);
        assert!(state.is_initialised());
    }

    #[test]
    fn with_reads_existing_value() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 7, name: "test".into() });
        let count = state.with(|s| s.count);
        assert_eq!(count, 7);
    }

    #[test]
    fn with_nested_reads_are_allowed() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 3, name: "n".into() });
        let (a, b) = state.with(|s| (s.count, state.with(|t| t.count)));
        assert_eq!(a, 3);
        assert_eq!(b, 3);
    }

    // ── with_mut ──────────────────────────────────────────────────────────────

    #[test]
    fn with_mut_auto_initialises_and_mutates() {
        let state: PluginState<Counter> = PluginState::new();
        state.with_mut(|s| s.count = 99);
        let count = state.with(|s| s.count);
        assert_eq!(count, 99);
    }

    #[test]
    fn with_mut_mutates_existing_value() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 5, name: "start".into() });
        state.with_mut(|s| {
            s.count += 10;
            s.name = "changed".into();
        });
        let (count, name) = state.with(|s| (s.count, s.name.clone()));
        assert_eq!(count, 15);
        assert_eq!(name, "changed");
    }

    #[test]
    fn with_mut_return_value_is_forwarded() {
        let state: PluginState<Counter> = PluginState::new();
        let prev = state.with_mut(|s| {
            let old = s.count;
            s.count = 5;
            old
        });
        assert_eq!(prev, 0);
        assert_eq!(state.with(|s| s.count), 5);
    }

    // ── get ───────────────────────────────────────────────────────────────────

    #[test]
    fn get_returns_clone_of_default_when_unset() {
        let state: PluginState<Counter> = PluginState::new();
        let c = state.get();
        assert_eq!(c, Counter::default());
        assert!(state.is_initialised());
    }

    #[test]
    fn get_returns_clone_of_stored_value() {
        let state: PluginState<Counter> = PluginState::new();
        state.set(Counter { count: 77, name: "clone".into() });
        let c = state.get();
        assert_eq!(c.count, 77);
        assert_eq!(c.name, "clone");
    }

    // ── Default impl for PluginState ──────────────────────────────────────────

    #[test]
    fn default_creates_empty_state() {
        let state: PluginState<Counter> = PluginState::default();
        assert!(!state.is_initialised());
    }
}
