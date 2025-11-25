//! Unified error handling system for Thoth
//!
//! This module provides:
//! - `ThothError`: Custom error type with specific variants for different error categories
//! - `ErrorHandler`: Centralized error display logic with user-friendly messages
//! - `ErrorRecovery`: Recovery strategies for graceful error handling
//!
//! # Usage
//!
//! ```rust
//! use thoth::error::{ThothError, ErrorHandler};
//!
//! let error = ThothError::FileNotFound { path: "data.json".into() };
//! let message = ErrorHandler::get_user_message(&error);
//! ```

mod handler;
mod recovery;
mod types;

pub use handler::ErrorHandler;
pub use recovery::{ErrorRecovery, RecoveryAction};
pub use types::ThothError;

/// Convenient Result type alias using ThothError
pub type Result<T> = std::result::Result<T, ThothError>;
