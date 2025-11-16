use eframe::egui;

/// Trait for stateless UI components
///
/// Stateless components are pure functions of their inputs.
/// They don't maintain any internal state between renders.
///
/// Example: Simple buttons, labels, icons, row renderers
pub trait StatelessComponent {
    type Props<'a>;
    type Output;

    /// Render the component with the given properties
    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output;
}

/// Trait for context-level components (panels)
///
/// Context components need access to the full egui::Context to create
/// top-level panels (TopBottomPanel, SidePanel, CentralPanel, etc.)
///
/// Follows one-way data binding pattern:
/// - Props flow down from parent to child (immutable)
/// - Events/callbacks flow up from child to parent (mutations)
///
/// Example: Toolbar, Settings panel, Central panel
pub trait ContextComponent {
    type Props<'a>;
    type Output;

    /// Render the component with access to the full egui Context and props
    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output;
}
