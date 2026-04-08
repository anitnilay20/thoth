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
/// run_ui_test(|ui| {
///     let output = MyComponent::render(ui, props);
///     assert!(!output.clicked);
/// });
/// ```
#[allow(deprecated)]
pub fn run_ui_test<F>(mut f: F)
where
    F: FnMut(&mut egui::Ui),
{
    let ctx = create_test_context();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, &mut f);
    });
}

/// Run a context component test with a `&mut egui::Ui`.
///
/// `ContextComponent::render` takes `&mut Ui` (not `&Context`) so that
/// components can create top-level panels via `show_inside`. This helper
/// wraps `run_ui_test` so test closures receive the same `&mut Ui`.
///
/// # Example
/// ```ignore
/// run_context_test(|ui| {
///     let mut toolbar = Toolbar::new();
///     let output = toolbar.render(ui, ToolbarProps { ... });
/// });
/// ```
pub fn run_context_test<F>(f: F)
where
    F: FnMut(&mut egui::Ui),
{
    run_ui_test(f)
}
