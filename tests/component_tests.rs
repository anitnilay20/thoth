//! Integration tests for component traits and implementations

mod common;

use common::{run_context_test, run_ui_test};
use thoth::components::data_row::{DataRow, DataRowProps, RowHighlights};
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
            DataRowProps::new(
                "key: value",
                (TextToken::Key, Some(TextToken::Str)),
                ui.visuals().widgets.noninteractive.bg_fill,
                "test-row",
                RowHighlights::default(),
                true,
            ),
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
            DataRowProps::new(
                "array: []",
                (TextToken::Key, Some(TextToken::Bracket)),
                ui.visuals().widgets.noninteractive.bg_fill,
                "array-row",
                RowHighlights::default(),
                true,
            ),
        );

        assert!(!output.clicked);
    });
}

#[test]
fn test_data_row_with_indentation() {
    run_ui_test(|ui| {
        // Test various indentation levels
        for level in 0..5usize {
            let output = DataRow::render(
                ui,
                DataRowProps {
                    indent: level,
                    ..DataRowProps::new(
                        &format!("level{}: value", level),
                        (TextToken::Key, Some(TextToken::Str)),
                        ui.visuals().widgets.noninteractive.bg_fill,
                        &format!("indent-{}", level),
                        RowHighlights::default(),
                        true,
                    )
                },
            );

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
            let output = DataRow::render(
                ui,
                DataRowProps::new(
                    "test: value",
                    (*token1, *token2),
                    ui.visuals().widgets.noninteractive.bg_fill,
                    &format!("token-{}", i),
                    RowHighlights::default(),
                    true,
                ),
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
                selected: true,
                ..DataRowProps::new(
                    "selected: item",
                    (TextToken::Key, Some(TextToken::Str)),
                    selected_bg,
                    "selected-row",
                    RowHighlights::default(),
                    true,
                )
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

        // First render
        component.render(ui, MockContextProps { title: "First" });
        assert_eq!(component.render_count, 1);
        assert_eq!(component.last_title, "First");

        // Second render
        component.render(ui, MockContextProps { title: "Second" });
        assert_eq!(component.render_count, 2);
        assert_eq!(component.last_title, "Second");
    });
}

// ============================================================================
// SidebarHeader Component Tests
// ============================================================================

use thoth::components::common::sidebar_header::{
    SidebarHeader, SidebarHeaderAction, SidebarHeaderProps,
};

#[test]
fn test_sidebar_header_title_only() {
    run_ui_test(|ui| {
        let out = SidebarHeader::render(ui, SidebarHeaderProps::new("CONNECTIONS"));
        assert!(out.action_clicked.is_none());
    });
}

#[test]
fn test_sidebar_header_with_trailing_and_actions() {
    run_ui_test(|ui| {
        let out = SidebarHeader::render(
            ui,
            SidebarHeaderProps {
                title: "PLUGIN STORE",
                trailing_text: Some("3 of 12"),
                actions: &[
                    SidebarHeaderAction {
                        icon: egui_phosphor::regular::MAGNIFYING_GLASS,
                        tooltip: "Search",
                    },
                    SidebarHeaderAction {
                        icon: egui_phosphor::regular::X,
                        tooltip: "Clear",
                    },
                ],
            },
        );
        // Nothing was clicked in a headless render.
        assert!(out.action_clicked.is_none());
    });
}

// ============================================================================
// DataRow tree-chrome render coverage (caret / leading icon / trailing)
// ============================================================================

#[test]
fn test_data_row_tree_chrome_renders() {
    use eframe::egui::Color32;
    run_ui_test(|ui| {
        // Expanded caret + leading icon + trailing text + indent.
        let out = DataRow::render(
            ui,
            DataRowProps {
                indent: 2,
                caret: Some(true),
                leading_icon: Some((egui_phosphor::regular::FOLDER, Color32::LIGHT_BLUE)),
                trailing: Some("12"),
                ..DataRowProps::new(
                    "public",
                    (TextToken::Key, None),
                    ui.visuals().widgets.noninteractive.bg_fill,
                    "sch:0",
                    RowHighlights::default(),
                    false,
                )
            },
        );
        assert!(!out.clicked);
        assert!(!out.caret_clicked);

        // Collapsed caret (leaf-less) variant.
        let out = DataRow::render(
            ui,
            DataRowProps {
                caret: Some(false),
                ..DataRowProps::new(
                    "users",
                    (TextToken::Key, None),
                    ui.visuals().widgets.noninteractive.bg_fill,
                    "tbl:0:0",
                    RowHighlights::default(),
                    false,
                )
            },
        );
        assert!(!out.caret_clicked);
    });
}

// ============================================================================
// render_node DSL smoke test — exercises many UiNode arms and the components
// they delegate to (DataRow, Table/TableView, List, Icon, Badge, Separator).
// ============================================================================

#[test]
fn test_render_ui_node_covers_many_variants() {
    use thoth::plugin::render_node::{UiNode, render_ui_node};

    let json = serde_json::json!({
        "type": "column",
        "gap": 4,
        "children": [
            { "type": "text", "value": "hello", "muted": true },
            { "type": "separator" },
            { "type": "spacer", "height": 6.0 },
            { "type": "badge", "label": "GET", "color": "#89b4fa" },
            { "type": "icon", "glyph": egui_phosphor::regular::DATABASE, "color": "info", "size": 14.0 },
            {
                "type": "data-row",
                "id": "sch:0",
                "label": "public",
                "indent": 1,
                "caret": true,
                "icon": { "glyph": egui_phosphor::regular::FOLDER, "color": "muted" },
                "trailing": "3"
            },
            {
                "type": "table",
                "headers": ["id", "name"],
                "rows": [[
                    { "type": "text", "value": "1" },
                    { "type": "text", "value": "alice" }
                ]]
            },
            {
                "type": "list",
                "id": "things",
                "items": [{ "title": "item one" }, { "title": "item two" }]
            }
        ]
    });

    let node: UiNode = serde_json::from_value(json).expect("valid UiNode");
    run_ui_test(|ui| {
        let mut events = Vec::new();
        render_ui_node(ui, &node, &mut events);
        // Headless render: no widget interactions emitted.
        assert!(events.is_empty());
    });
}
