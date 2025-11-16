//! Common testing utilities shared across integration tests
//!
//! This module provides test helpers and utilities for testing
//! Thoth components in isolation.

pub mod mocks;

use eframe::egui;

/// Create a test egui context for component testing
pub fn create_test_context() -> egui::Context {
    egui::Context::default()
}

/// Run a component test with a proper egui context and UI
///
/// # Example
/// ```ignore
/// use thoth::components::data_row::{DataRow, DataRowProps};
/// use thoth::components::traits::StatelessComponent;
///
/// run_ui_test(|ui| {
///     let output = DataRow::render(ui, DataRowProps {
///         display_text: "test",
///         indent: 0,
///         is_expandable: false,
///         is_expanded: false,
///         text_tokens: (TextToken::Key, None),
///         background: egui::Color32::TRANSPARENT,
///         row_id: "test-id",
///     });
///     assert!(!output.clicked);
/// });
/// ```
pub fn run_ui_test<F>(mut f: F)
where
    F: FnMut(&mut egui::Ui),
{
    let ctx = create_test_context();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, &mut f);
    });
}

/// Run a context component test
///
/// # Example
/// ```ignore
/// use thoth::components::toolbar::{Toolbar, ToolbarProps};
/// use thoth::components::traits::ContextComponent;
///
/// run_context_test(|ctx| {
///     let mut toolbar = Toolbar::new();
///     let output = toolbar.render(ctx, ToolbarProps { ... });
///     assert_eq!(output.file_opened, false);
/// });
/// ```
pub fn run_context_test<F>(mut f: F)
where
    F: FnMut(&egui::Context),
{
    let ctx = create_test_context();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        f(ctx);
    });
}
