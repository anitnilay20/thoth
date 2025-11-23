/// Application-wide constants for behavioral configuration
///
/// This module contains constants that define app behavior, limits, and constraints.
/// For UI/design system constants (spacing, colors, etc.), see theme.rs

// Sidebar configuration
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 350.0;
pub const MIN_SIDEBAR_WIDTH: f32 = DEFAULT_SIDEBAR_WIDTH;
pub const MAX_SIDEBAR_WIDTH_RATIO: f32 = 0.7; // 70% of window width

// Recent files configuration
pub const MAX_RECENT_FILES: usize = 10;
