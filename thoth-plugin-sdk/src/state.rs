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
