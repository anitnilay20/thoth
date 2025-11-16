//! Integration tests for component traits and implementations

mod common;

use common::{run_context_test, run_ui_test};
use thoth::components::data_row::{DataRow, DataRowProps};
use thoth::components::traits::StatelessComponent;
use thoth::theme::TextToken;

// ============================================================================
// DataRow Component Tests
// ============================================================================

#[test]
fn test_data_row_basic() {
    run_ui_test(|ui| {
        let output = DataRow::render(
            ui,
            DataRowProps {
                display_text: "key: value",
                indent: 0,
                text_tokens: (TextToken::Key, Some(TextToken::Str)),
                background: ui.visuals().widgets.noninteractive.bg_fill,
                row_id: "test-row",
            },
        );

        // Initially not clicked
        assert!(!output.clicked);
        assert!(!output.right_clicked);
    });
}

#[test]
fn test_data_row_with_brackets() {
    run_ui_test(|ui| {
        let output = DataRow::render(
            ui,
            DataRowProps {
                display_text: "array: []",
                indent: 1,
                text_tokens: (TextToken::Key, Some(TextToken::Bracket)),
                background: ui.visuals().widgets.noninteractive.bg_fill,
                row_id: "array-row",
            },
        );

        assert!(!output.clicked);
    });
}

#[test]
fn test_data_row_with_indentation() {
    run_ui_test(|ui| {
        // Test various indentation levels
        for indent in 0..5 {
            let output = DataRow::render(
                ui,
                DataRowProps {
                    display_text: &format!("level{}: value", indent),
                    indent,
                    text_tokens: (TextToken::Key, Some(TextToken::Str)),
                    background: ui.visuals().widgets.noninteractive.bg_fill,
                    row_id: &format!("indent-{}", indent),
                },
            );

            assert!(!output.clicked);
        }
    });
}

#[test]
fn test_data_row_different_text_tokens() {
    run_ui_test(|ui| {
        let token_pairs = vec![
            (TextToken::Key, Some(TextToken::Str)),
            (TextToken::Key, Some(TextToken::Number)),
            (TextToken::Key, Some(TextToken::Boolean)),
            (TextToken::Bracket, None),
        ];

        for (i, (token1, token2)) in token_pairs.iter().enumerate() {
            let output = DataRow::render(
                ui,
                DataRowProps {
                    display_text: "test: value",
                    indent: 0,
                    text_tokens: (*token1, *token2),
                    background: ui.visuals().widgets.noninteractive.bg_fill,
                    row_id: &format!("token-{}", i),
                },
            );

            assert!(!output.clicked);
        }
    });
}

#[test]
fn test_data_row_with_selection_background() {
    run_ui_test(|ui| {
        let selected_bg = ui.visuals().selection.bg_fill;

        let output = DataRow::render(
            ui,
            DataRowProps {
                display_text: "selected: item",
                indent: 0,
                text_tokens: (TextToken::Key, Some(TextToken::Str)),
                background: selected_bg,
                row_id: "selected-row",
            },
        );

        assert!(!output.clicked);
    });
}

// ============================================================================
// Mock Component Tests
// ============================================================================

#[test]
fn test_mock_stateless_component() {
    use common::mocks::{MockProps, MockStatelessComponent};

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
    use common::mocks::{MockProps, MockStatelessComponent};

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
    use common::mocks::{MockContextComponent, MockContextProps};
    use thoth::components::traits::ContextComponent;

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
    use common::mocks::{MockContextComponent, MockContextProps};
    use thoth::components::traits::ContextComponent;

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
