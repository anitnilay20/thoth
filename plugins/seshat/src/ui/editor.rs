//! The SQL editor tab: header, code editor, Run, and the typed results grid.

use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonSize, ButtonType, CodeEditor, Column, CustomSyntax, Row,
    Scroll, Select, SelectOption, SelectSize, Separator,
};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::constants::{KEYWORDS, SPECIAL, TYPES};
use crate::state::State;
use crate::ui::results::results_view;
use crate::ICON_PLAY;

pub(crate) fn editor_view(st: &State) -> RenderNode {
    // The database this editor queries against — also what autocomplete is
    // scoped to. Defaults to the connection's database; switchable via the
    // database dropdown below.
    let active_db = st.active_profile.as_ref().map(|p| p.database.as_str());

    // Table names from the active database's loaded schemas — fed to the
    // editor's autocomplete. Scoped to the active database so suggestions match
    // the database queries run against.
    let tables: Vec<String> = st
        .databases
        .iter()
        .filter(|d| Some(d.name.as_str()) == active_db)
        .filter_map(|d| d.schemas.as_ref())
        .flatten()
        .filter_map(|s| s.tables.as_ref())
        .flatten()
        .map(|t| t.name.clone())
        .collect();

    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                RenderNode::Row(
                    Row::builder()
                        .padding(8.0)
                        .gap(8.0)
                        .children(vec![
                            // Connection switcher: re-points this editor tab at a
                            // different saved connection (keeps the SQL, reloads
                            // schema/autocomplete for the new target).
                            RenderNode::Select(
                                Select::builder()
                                    .id("switch-connection")
                                    .value(st.active.clone().unwrap_or_default())
                                    .options(
                                        st.connections
                                            .iter()
                                            .map(|c| {
                                                SelectOption::builder()
                                                    .value(c.id.clone())
                                                    .label(c.name.clone())
                                                    .build()
                                            })
                                            .collect::<Vec<_>>(),
                                    )
                                    .size(SelectSize::Small)
                                    .width(180.0)
                                    .build(),
                            ),
                            // Database switcher: picks which database in the
                            // current connection queries + autocomplete target.
                            RenderNode::Select(
                                Select::builder()
                                    .id("switch-database")
                                    .value(active_db.unwrap_or_default().to_string())
                                    .options(
                                        st.databases
                                            .iter()
                                            .map(|d| {
                                                SelectOption::builder()
                                                    .value(d.name.clone())
                                                    .label(d.name.clone())
                                                    .build()
                                            })
                                            .collect::<Vec<_>>(),
                                    )
                                    .size(SelectSize::Small)
                                    .width(180.0)
                                    .build(),
                            ),
                            RenderNode::Separator(Separator::plain()),
                            // Small size so the Run button matches the connection
                            // select's height.
                            RenderNode::Button(
                                Button::builder()
                                    .id("run")
                                    .label("Run")
                                    .button_type(ButtonType::Elevated)
                                    .color(ButtonColor::Primary)
                                    .button_size(ButtonSize::Small)
                                    .icon(ICON_PLAY)
                                    .enabled(!st.loading)
                                    .build(),
                            ),
                        ])
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                RenderNode::CodeEditor(
                    CodeEditor::builder()
                        .id("sql")
                        .value(st.sql.clone())
                        .font_size(12.0)
                        .custom_syntax(
                            CustomSyntax::builder()
                                .language("sql")
                                .case_sensitive(false)
                                .comment("--")
                                .comment_multiline(("/*".to_string(), "*/".to_string()))
                                .keywords(KEYWORDS.iter().map(|s| s.to_string()).collect())
                                .types(TYPES.iter().map(|s| s.to_string()).collect())
                                // Built-in specials plus the live table names.
                                .special(
                                    SPECIAL
                                        .iter()
                                        .map(|s| s.to_string())
                                        .chain(tables)
                                        .collect(),
                                )
                                .build(),
                        )
                        .bordered(false)
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                RenderNode::Scroll(Scroll::builder().child(results_view(st)).both(true).build()),
            ])
            .build(),
    )
}
