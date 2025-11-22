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

/// Trait for stateful UI components
///
/// Stateful components maintain internal state between renders.
/// They receive a mutable reference to themselves, props from parent, and a UI region to render in.
///
/// Follows one-way data binding pattern:
/// - Props flow down from parent to child (immutable)
/// - Component manages its own internal state (e.g., text input, checkboxes)
/// - Events flow up to parent via Output type
///
/// Example: Search panels, settings panels, forms with internal state
pub trait StatefulComponent {
    type Props<'a>;
    type Output;

    /// Render the component with mutable access to internal state and props from parent
    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output;
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
