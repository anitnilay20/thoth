//! Mock implementations of component traits for testing

use eframe::egui;
use thoth::components::traits::{ContextComponent, StatelessComponent};

/// Mock stateless component for testing
///
/// Always returns a predictable output for testing purposes
pub struct MockStatelessComponent;

#[derive(Debug, PartialEq)]
pub struct MockProps<'a> {
    pub text: &'a str,
    pub enabled: bool,
}

#[derive(Debug, PartialEq)]
pub struct MockOutput {
    pub clicked: bool,
    pub text: String,
}

impl StatelessComponent for MockStatelessComponent {
    type Props<'a> = MockProps<'a>;
    type Output = MockOutput;

    fn render(_ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        MockOutput {
            clicked: props.enabled,
            text: props.text.to_string(),
        }
    }
}

/// Mock context component for testing
pub struct MockContextComponent {
    pub render_count: usize,
    pub last_title: String,
}

#[derive(Debug)]
pub struct MockContextProps<'a> {
    pub title: &'a str,
}

#[derive(Debug, PartialEq)]
pub struct MockContextOutput {
    pub rendered: bool,
    pub title: String,
}

impl ContextComponent for MockContextComponent {
    type Props<'a> = MockContextProps<'a>;
    type Output = MockContextOutput;

    fn render(&mut self, _ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        self.render_count += 1;
        self.last_title = props.title.to_string();
        MockContextOutput {
            rendered: true,
            title: props.title.to_string(),
        }
    }
}

impl Default for MockContextComponent {
    fn default() -> Self {
        Self {
            render_count: 0,
            last_title: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{run_context_test, run_ui_test};

    #[test]
    fn test_mock_stateless_component() {
        run_ui_test(|ui| {
            let output = MockStatelessComponent::render(
                ui,
                MockProps {
                    text: "test",
                    enabled: true,
                },
            );

            assert_eq!(output.clicked, true);
            assert_eq!(output.text, "test");
        });
    }

    #[test]
    fn test_mock_stateless_component_disabled() {
        run_ui_test(|ui| {
            let output = MockStatelessComponent::render(
                ui,
                MockProps {
                    text: "disabled",
                    enabled: false,
                },
            );

            assert_eq!(output.clicked, false);
            assert_eq!(output.text, "disabled");
        });
    }

    #[test]
    fn test_mock_context_component() {
        run_context_test(|ctx| {
            let mut component = MockContextComponent::default();

            let output = component.render(ctx, MockContextProps { title: "Test" });

            assert_eq!(output.rendered, true);
            assert_eq!(output.title, "Test");
            assert_eq!(component.render_count, 1);
            assert_eq!(component.last_title, "Test");
        });
    }

    #[test]
    fn test_mock_context_component_multiple_renders() {
        run_context_test(|ctx| {
            let mut component = MockContextComponent::default();

            // First render
            component.render(ctx, MockContextProps { title: "First" });
            assert_eq!(component.render_count, 1);
            assert_eq!(component.last_title, "First");

            // Second render
            component.render(ctx, MockContextProps { title: "Second" });
            assert_eq!(component.render_count, 2);
            assert_eq!(component.last_title, "Second");
        });
    }
}
