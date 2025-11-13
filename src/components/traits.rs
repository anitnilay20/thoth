use eframe::egui;

/// Trait for stateless UI components
///
/// Stateless components are pure functions of their inputs.
/// They don't maintain any internal state between renders.
///
/// Example: Simple buttons, labels, icons
pub trait StatelessComponent {
    type Props;
    type Output;

    /// Render the component with the given properties
    fn render(ui: &mut egui::Ui, props: Self::Props) -> Self::Output;
}

/// Trait for stateful UI components
///
/// Stateful components maintain internal state that persists between renders.
/// They manage their own state and update it during rendering.
///
/// Example: Text inputs, counters, expandable sections, file viewers
pub trait StatefulComponent {
    type Output;

    /// Render the component and return any output
    fn render(&mut self, ui: &mut egui::Ui) -> Self::Output;
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
