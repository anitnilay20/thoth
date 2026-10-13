//! Integration tests for component traits and host rendering of SDK components.

mod common;

use common::{run_context_test, run_ui_test};
use thoth::components::traits::StatelessComponent;
use thoth_plugin_sdk::components::{DataRow, SidebarHeader, SidebarHeaderAction};
use thoth_plugin_sdk::tokens::TextToken;

// ============================================================================
// DataRow Component Tests (now provided by thoth-plugin-sdk)
// ============================================================================

#[test]
fn test_data_row_basic() {
    run_ui_test(|ui| {
        let output = DataRow::builder()
            .display_text("key: value")
            .row_id("test-row")
            .key_token(TextToken::Key)
            .value_token(TextToken::Str)
            .syntax_highlighting(true)
            .build()
            .show(ui);

        assert!(!output.clicked);
        assert!(!output.right_clicked);
    });
}

#[test]
fn test_data_row_with_brackets() {
    run_ui_test(|ui| {
        let output = DataRow::builder()
            .display_text("array: []")
            .row_id("array-row")
            .key_token(TextToken::Key)
            .value_token(TextToken::Bracket)
            .syntax_highlighting(true)
            .build()
            .show(ui);

        assert!(!output.clicked);
    });
}

#[test]
fn test_data_row_with_indentation() {
    run_ui_test(|ui| {
        for level in 0..5usize {
            let output = DataRow::builder()
                .display_text(format!("level{level}: value"))
                .row_id(format!("indent-{level}"))
                .key_token(TextToken::Key)
                .value_token(TextToken::Str)
                .indent(level)
                .syntax_highlighting(true)
                .build()
                .show(ui);

            assert!(!output.clicked);
        }
    });
}

#[test]
fn test_data_row_different_text_tokens() {
    run_ui_test(|ui| {
        let token_pairs = [
            (TextToken::Key, Some(TextToken::Str)),
            (TextToken::Key, Some(TextToken::Number)),
            (TextToken::Key, Some(TextToken::Boolean)),
            (TextToken::Bracket, None),
        ];

        for (i, (token1, token2)) in token_pairs.iter().enumerate() {
            let output = DataRow::builder()
                .display_text("test: value")
                .row_id(format!("token-{i}"))
                .key_token(*token1)
                .maybe_value_token(*token2)
                .syntax_highlighting(true)
                .build()
                .show(ui);

            assert!(!output.clicked);
        }
    });
}

#[test]
fn test_data_row_with_selection() {
    run_ui_test(|ui| {
        let output = DataRow::builder()
            .display_text("selected: item")
            .row_id("selected-row")
            .key_token(TextToken::Key)
            .value_token(TextToken::Str)
            .selected(true)
            .syntax_highlighting(true)
            .build()
            .show(ui);

        assert!(!output.clicked);
    });
}

#[test]
fn test_data_row_tree_chrome_renders() {
    use thoth_plugin_sdk::components::DataRowIcon;
    run_ui_test(|ui| {
        // Expanded caret + leading icon + trailing text + indent.
        let out = DataRow::builder()
            .display_text("public")
            .row_id("sch:0")
            .key_token(TextToken::Key)
            .indent(2)
            .caret(true)
            .leading_icon(
                DataRowIcon::builder()
                    .glyph(egui_phosphor::regular::FOLDER)
                    .build(),
            )
            .trailing("12")
            .build()
            .show(ui);
        assert!(!out.clicked);
        assert!(!out.caret_clicked);

        // Collapsed caret variant.
        let out = DataRow::builder()
            .display_text("users")
            .row_id("tbl:0:0")
            .key_token(TextToken::Key)
            .caret(false)
            .build()
            .show(ui);
        assert!(!out.caret_clicked);
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

        assert!(output.clicked);
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

        assert!(!output.clicked);
        assert_eq!(output.text, "disabled");
    });
}

#[test]
fn test_mock_context_component() {
    use common::mocks::{MockContextComponent, MockContextProps};
    use thoth::components::traits::ContextComponent;

    run_context_test(|ui| {
        let mut component = MockContextComponent::default();

        let output = component.render(ui, MockContextProps { title: "Test" });

        assert!(output.rendered);
        assert_eq!(output.title, "Test");
        assert_eq!(component.render_count, 1);
        assert_eq!(component.last_title, "Test");
    });
}

#[test]
fn test_mock_context_component_multiple_renders() {
    use common::mocks::{MockContextComponent, MockContextProps};
    use thoth::components::traits::ContextComponent;

    run_context_test(|ui| {
        let mut component = MockContextComponent::default();

        component.render(ui, MockContextProps { title: "First" });
        assert_eq!(component.render_count, 1);
        assert_eq!(component.last_title, "First");

        component.render(ui, MockContextProps { title: "Second" });
        assert_eq!(component.render_count, 2);
        assert_eq!(component.last_title, "Second");
    });
}

// ============================================================================
// SidebarHeader Component Tests (now provided by thoth-plugin-sdk)
// ============================================================================

#[test]
fn test_sidebar_header_title_only() {
    run_ui_test(|ui| {
        let out = SidebarHeader::builder()
            .title("CONNECTIONS")
            .build()
            .show(ui);
        assert!(out.inner.is_none());
    });
}

#[test]
fn test_sidebar_header_with_trailing_and_actions() {
    run_ui_test(|ui| {
        let out = SidebarHeader::builder()
            .title("PLUGIN STORE")
            .trailing_text("3 of 12")
            .actions(vec![
                SidebarHeaderAction::builder()
                    .icon(egui_phosphor::regular::MAGNIFYING_GLASS)
                    .tooltip("Search")
                    .build(),
                SidebarHeaderAction::builder()
                    .icon(egui_phosphor::regular::X)
                    .tooltip("Clear")
                    .build(),
            ])
            .build()
            .show(ui);
        // Nothing was clicked in a headless render.
        assert!(out.inner.is_none());
    });
}

// ============================================================================
// render_node DSL smoke test — exercises many RenderNode arms and the
// components they delegate to (DataRow, Table, List, Icon, Badge, Separator).
// ============================================================================

#[test]
fn test_render_ui_node_covers_many_variants() {
    use thoth::plugin::render_node::{UiNode, render_ui_node};
    use thoth_plugin_sdk::components::{
        Badge, Column, Icon, List, ListItem, Separator, Spacer, TableView,
    };

    // Round-trip a builder-built tree through JSON to also exercise serde, then
    // render it headlessly to cover many RenderNode arms.
    let tree = UiNode::Column(
        Column::builder()
            .gap(4.0)
            .children(vec![
                UiNode::text("hello"),
                UiNode::Separator(Separator::plain()),
                UiNode::Spacer(Spacer::builder().size(6.0).build()),
                UiNode::Badge(Badge::builder().label("GET").color("#89b4fa").build()),
                UiNode::Icon(
                    Icon::builder()
                        .glyph(egui_phosphor::regular::DATABASE)
                        .color("info")
                        .build(),
                ),
                UiNode::DataRow(
                    DataRow::builder()
                        .display_text("public")
                        .row_id("sch:0")
                        .key_token(TextToken::Key)
                        .indent(1)
                        .caret(true)
                        .trailing("3")
                        .build(),
                ),
                UiNode::Table(
                    TableView::builder()
                        .headers(vec!["id".to_string(), "name".to_string()])
                        .rows(vec![vec![UiNode::text("1"), UiNode::text("alice")]])
                        .build(),
                ),
                UiNode::List(
                    List::builder()
                        .id("things")
                        .items(vec![
                            ListItem::builder().title("item one").build(),
                            ListItem::builder().title("item two").build(),
                        ])
                        .build(),
                ),
            ])
            .build(),
    );

    let json = serde_json::to_string(&tree).expect("serialize");
    let mut node: UiNode = serde_json::from_str(&json).expect("valid UiNode");
    run_ui_test(|ui| {
        let mut events = Vec::new();
        render_ui_node(ui, &mut node, &mut events);
        // Headless render: no widget interactions emitted.
        assert!(events.is_empty());
    });
}
