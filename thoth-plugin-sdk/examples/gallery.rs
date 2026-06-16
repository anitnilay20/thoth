//! A Storybook-style gallery for the Thoth plugin SDK's egui widgets.
//!
//! Run with:
//!   cargo run -p thoth-plugin-sdk --example gallery --features egui
//!
//! The SDK widgets are not self-contained: they read their palette from egui
//! memory (see `ThemeColors::from_ctx`) and render icons with the "phosphor"
//! font family. This example therefore does the two things the *host*
//! normally does — inject a `ThemeColors` every frame and register the icon
//! font — so the widgets render exactly as they would inside Thoth.

use eframe::egui;
use egui::Color32;
use thoth_plugin_sdk::components::{
    Breadcrumbs, Button, ButtonColor, ButtonGroups, ButtonSize, ButtonType, DataRow, IconButton,
    Input, JsonTree, Select, SelectOption, Separator, SidebarHeader, SidebarHeaderAction,
    TableView, ToggleSwitch, Typography, TypographyVariant,
};
use thoth_plugin_sdk::theme::{THEME_MEMORY_ID, TextToken, ThemeColors};

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Thoth SDK — Component Gallery",
        native_options,
        Box::new(|cc| {
            register_phosphor(&cc.egui_ctx);
            Ok(Box::new(Gallery::default()) as Box<dyn eframe::App>)
        }),
    )
}

/// Register the Phosphor icon font under the `"phosphor"` family, matching what
/// the Thoth host does so `phosphor_font_id` resolves.
fn register_phosphor(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts.families.insert(
        egui::FontFamily::Name("phosphor".into()),
        vec!["phosphor".into()],
    );
    ctx.set_fonts(fonts);
}

#[derive(PartialEq, Clone, Copy)]
enum Story {
    Button,
    ButtonGroup,
    Breadcrumbs,
    Typography,
    Separator,
    Input,
    Select,
    ToggleSwitch,
    IconButton,
    SidebarHeader,
    DataRow,
    TableView,
    JsonTree,
}

const STORIES: &[(Story, &str)] = &[
    (Story::Button, "Button"),
    (Story::ButtonGroup, "Button Group"),
    (Story::Breadcrumbs, "Breadcrumbs"),
    (Story::Typography, "Typography"),
    (Story::Separator, "Separator"),
    (Story::Input, "Input"),
    (Story::Select, "Select"),
    (Story::ToggleSwitch, "Toggle Switch"),
    (Story::IconButton, "Icon Button"),
    (Story::SidebarHeader, "Sidebar Header"),
    (Story::DataRow, "Data Row"),
    (Story::TableView, "Table View"),
    (Story::JsonTree, "JSON Tree"),
];

struct Gallery {
    story: Story,
    dark: bool,

    // Live controls for the Button story.
    label: String,
    color: ButtonColor,
    button_type: ButtonType,
    size: ButtonSize,
    enabled: bool,
    full_width: bool,
    show_icon: bool,

    // State for the ButtonGroup story.
    active: usize,

    // State for the Breadcrumbs story.
    crumb_path: String,
    crumb_separator: String,
    last_navigated: Option<String>,

    // Stateful widgets owning their own value.
    input: Input,
    select: Select,
    toggled: bool,
    row_selected: bool,
    last_header_action: Option<usize>,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            story: Story::Button,
            dark: true,
            label: "Click me".to_owned(),
            color: ButtonColor::Primary,
            button_type: ButtonType::Elevated,
            size: ButtonSize::Medium,
            enabled: true,
            full_width: false,
            show_icon: false,
            active: 0,
            crumb_path: "users.42.settings.theme".to_owned(),
            crumb_separator: ".".to_owned(),
            last_navigated: None,
            input: Input::builder()
                .placeholder("Type something…".to_owned())
                .icon(egui_phosphor::regular::MAGNIFYING_GLASS.to_owned())
                .build(),
            select: Select::builder()
                .id("gallery-select".to_owned())
                .value("name".to_owned())
                .options(vec![
                    SelectOption::builder().value("name".into()).label("Name".into()).build(),
                    SelectOption::builder().value("date".into()).label("Date".into()).build(),
                    SelectOption::builder().value("size".into()).label("Size".into()).build(),
                ])
                .prefix_label("Sort: ".to_owned())
                .build(),
            toggled: true,
            row_selected: false,
            last_header_action: None,
        }
    }
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Mimic the host: publish the active palette into egui memory so every
        // SDK widget can read it back via `ThemeColors::from_ctx`.
        let colors = if self.dark { dark_palette() } else { light_palette() };
        ui.ctx()
            .memory_mut(|m| m.data.insert_temp(egui::Id::new(THEME_MEMORY_ID), colors));

        egui::Panel::left("stories")
            .resizable(false)
            .default_size(180.0)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                ui.heading("Components");
                ui.separator();
                for (story, label) in STORIES {
                    ui.selectable_value(&mut self.story, *story, *label);
                }

                ui.add_space(12.0);
                ui.separator();
                ui.checkbox(&mut self.dark, "Dark theme");
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).fill(colors.bg))
            .show_inside(ui, |ui| match self.story {
                Story::Button => self.button_story(ui),
                Story::ButtonGroup => self.button_group_story(ui),
                Story::Breadcrumbs => self.breadcrumbs_story(ui),
                Story::Typography => self.typography_story(ui),
                Story::Separator => self.separator_story(ui),
                Story::Input => self.input_story(ui),
                Story::Select => self.select_story(ui),
                Story::ToggleSwitch => self.toggle_story(ui),
                Story::IconButton => self.icon_button_story(ui),
                Story::SidebarHeader => self.sidebar_header_story(ui),
                Story::DataRow => self.data_row_story(ui),
                Story::TableView => self.table_view_story(ui),
                Story::JsonTree => self.json_tree_story(ui),
            });
    }
}

impl Gallery {
    fn button_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Button");
        ui.add_space(8.0);

        // ── Controls ──────────────────────────────────────────────────────
        egui::Grid::new("button-controls")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Label");
                ui.text_edit_singleline(&mut self.label);
                ui.end_row();

                ui.label("Color");
                combo(ui, "color", &mut self.color, &[
                    (ButtonColor::Default, "Default"),
                    (ButtonColor::Primary, "Primary"),
                    (ButtonColor::Secondary, "Secondary"),
                    (ButtonColor::Danger, "Danger"),
                    (ButtonColor::Success, "Success"),
                ]);
                ui.end_row();

                ui.label("Type");
                combo(ui, "type", &mut self.button_type, &[
                    (ButtonType::Elevated, "Elevated"),
                    (ButtonType::Text, "Text"),
                ]);
                ui.end_row();

                ui.label("Size");
                combo(ui, "size", &mut self.size, &[
                    (ButtonSize::Small, "Small"),
                    (ButtonSize::Medium, "Medium"),
                    (ButtonSize::Large, "Large"),
                ]);
                ui.end_row();

                ui.label("Flags");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.enabled, "enabled");
                    ui.checkbox(&mut self.full_width, "full width");
                    ui.checkbox(&mut self.show_icon, "icon");
                });
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);

        // ── Live preview ──────────────────────────────────────────────────
        let icon = self.show_icon.then_some(egui_phosphor::regular::STAR);
        let button = Button::builder()
            .label(self.label.as_str())
            .color(self.color)
            .button_type(self.button_type)
            .button_size(self.size)
            .enabled(self.enabled)
            .full_width(self.full_width)
            .maybe_icon(icon)
            .build();

        if ui.add(button).clicked() {
            println!("button clicked");
        }

        ui.add_space(24.0);
        ui.label(egui::RichText::new("All colors").strong());
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for (c, name) in [
                (ButtonColor::Default, "Default"),
                (ButtonColor::Primary, "Primary"),
                (ButtonColor::Secondary, "Secondary"),
                (ButtonColor::Danger, "Danger"),
                (ButtonColor::Success, "Success"),
            ] {
                ui.add(
                    Button::builder()
                        .label(name)
                        .color(c)
                        .button_type(ButtonType::Elevated)
                        .button_size(ButtonSize::Medium)
                        .enabled(true)
                        .full_width(false)
                        .build(),
                );
            }
        });
    }

    fn button_group_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Button Group");
        ui.add_space(8.0);
        ui.label(format!("Active index: {}", self.active));
        ui.add_space(12.0);

        let items: Vec<Button> = ["GET", "POST", "PUT", "DELETE"]
            .iter()
            .map(|label| {
                Button::builder()
                    .label(label)
                    .color(ButtonColor::Primary)
                    .button_type(ButtonType::Text)
                    .button_size(ButtonSize::Medium)
                    .enabled(true)
                    .full_width(false)
                    .build()
            })
            .collect();

        let group = ButtonGroups::builder()
            .items(items)
            .active(self.active)
            .build();
        if let Some(i) = group.show(ui).inner {
            self.active = i;
        }
    }

    fn breadcrumbs_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Breadcrumbs");
        ui.add_space(8.0);

        // ── Controls ──────────────────────────────────────────────────────
        egui::Grid::new("breadcrumbs-controls")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Path");
                ui.text_edit_singleline(&mut self.crumb_path);
                ui.end_row();

                ui.label("Separator");
                ui.text_edit_singleline(&mut self.crumb_separator);
                ui.end_row();
            });
        ui.label(
            egui::RichText::new("Input is dot-separated; numeric segments render as [n]. \
                 The separator only changes how segments are joined for display/navigation.")
                .small()
                .weak(),
        );

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);

        // ── Live preview ──────────────────────────────────────────────────
        let crumbs = Breadcrumbs::builder()
            .path(self.crumb_path.as_str())
            .maybe_separator((!self.crumb_separator.is_empty()).then_some(self.crumb_separator.as_str()))
            .build();
        if let Some(path) = crumbs.show(ui).inner {
            self.last_navigated = Some(path);
        }

        ui.add_space(16.0);
        match &self.last_navigated {
            Some(p) => ui.label(format!("Navigated to: {p}")),
            None => ui.label("Click a segment to navigate."),
        };
    }

    fn typography_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Typography");
        ui.add_space(8.0);
        let variants = [
            (TypographyVariant::PanelHeader, "PanelHeader"),
            (TypographyVariant::SectionHeader, "SectionHeader"),
            (TypographyVariant::GroupLabel, "GroupLabel"),
            (TypographyVariant::Title, "Title"),
            (TypographyVariant::Heading, "Heading"),
            (TypographyVariant::BodyLarge, "BodyLarge"),
            (TypographyVariant::Body, "Body"),
            (TypographyVariant::BodyMuted, "BodyMuted"),
            (TypographyVariant::Subtitle, "Subtitle"),
            (TypographyVariant::Caption, "Caption"),
            (TypographyVariant::Label, "Label"),
            (TypographyVariant::Mono, "Mono"),
        ];
        for (variant, name) in variants {
            ui.add(
                Typography::builder()
                    .text(name)
                    .variant(variant)
                    .build(),
            );
            ui.add_space(2.0);
        }
        ui.add(Separator::with_margin(8.0));
        ui.add(
            Typography::builder()
                .text("bold · italic · underline · #cba6f7")
                .bold(true)
                .italic(true)
                .underline(true)
                .color("#cba6f7")
                .build(),
        );
    }

    fn separator_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Separator");
        ui.add_space(8.0);
        ui.label("plain:");
        ui.add(Separator::plain());
        ui.label("with_margin(16):");
        ui.add(Separator::with_margin(16.0));
        ui.label("with_margins(0, 24):");
        ui.add(Separator::with_margins(0.0, 24.0));
        ui.label("done");
    }

    fn input_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Input");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("single").clicked() {
                self.input.multiline = false;
            }
            if ui.button("multiline").clicked() {
                self.input.multiline = true;
            }
            ui.checkbox(&mut self.input.password, "password");
            ui.checkbox(&mut self.input.disabled, "disabled");
        });
        ui.add_space(12.0);
        self.input.show(ui);
        ui.add_space(8.0);
        ui.label(format!("value: {:?}", self.input.value));
    }

    fn select_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Select");
        ui.add_space(8.0);
        self.select.width = Some(220.0);
        if let Some(v) = self.select.show(ui).inner {
            println!("selected {v}");
        }
        ui.add_space(8.0);
        ui.label(format!("value: {}", self.select.value));
    }

    fn toggle_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Toggle Switch");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let toggle = ToggleSwitch::builder()
                .enabled(self.toggled)
                .hover_text("Toggle me")
                .build();
            if ui.add(toggle).clicked() {
                self.toggled = !self.toggled;
            }
            ui.label(if self.toggled { "on" } else { "off" });
        });
    }

    fn icon_button_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Icon Button");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(IconButton::builder().icon(egui_phosphor::regular::STAR).tooltip("Plain").build());
            ui.add(IconButton::builder().icon(egui_phosphor::regular::GEAR).frame(true).tooltip("Framed").build());
            ui.add(IconButton::builder().icon(egui_phosphor::regular::HEART).selected(true).tooltip("Selected").build());
            ui.add(IconButton::builder().icon(egui_phosphor::regular::TRASH).disabled(true).tooltip("Disabled").build());
            ui.add(IconButton::builder().icon(egui_phosphor::regular::BELL).badge_color("#f38ba8").tooltip("Badge").build());
            ui.add(IconButton::builder().icon(egui_phosphor::regular::PLUS).size(28.0).tooltip("Large").build());
        });
    }

    fn sidebar_header_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sidebar Header");
        ui.add_space(8.0);
        let header = SidebarHeader::builder()
            .title("RECENT FILES")
            .trailing_text("3 of 12")
            .actions(vec![
                SidebarHeaderAction::builder()
                    .icon(egui_phosphor::regular::ARROWS_CLOCKWISE)
                    .tooltip("Refresh")
                    .build(),
                SidebarHeaderAction::builder()
                    .icon(egui_phosphor::regular::TRASH)
                    .tooltip("Clear")
                    .build(),
            ])
            .build();
        if let Some(i) = header.show(ui).inner {
            self.last_header_action = Some(i);
        }
        ui.add_space(8.0);
        ui.label(format!("last action: {:?}", self.last_header_action));
    }

    fn data_row_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Data Row");
        ui.add_space(8.0);

        DataRow::builder()
            .display_text("object".to_owned())
            .row_id("dr-0".to_owned())
            .key_token(TextToken::Key)
            .caret(true)
            .indent(0)
            .build()
            .show(ui);

        DataRow::builder()
            .display_text("name: \"thoth\"".to_owned())
            .row_id("dr-1".to_owned())
            .key_token(TextToken::Key)
            .value_token(TextToken::Str)
            .syntax_highlighting(true)
            .indent(1)
            .selected(self.row_selected)
            .build()
            .show(ui);

        let count = DataRow::builder()
            .display_text("count: 42".to_owned())
            .row_id("dr-2".to_owned())
            .key_token(TextToken::Key)
            .value_token(TextToken::Number)
            .syntax_highlighting(true)
            .indent(1)
            .trailing("int".to_owned())
            .build()
            .show(ui);
        if count.clicked {
            self.row_selected = !self.row_selected;
        }

        ui.add_space(8.0);
        ui.label("Click the 'count' row to toggle the selected row above.");
    }

    fn table_view_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("Table View");
        ui.add_space(8.0);
        let table = TableView::builder()
            .headers(vec![
                "id  ·  int".into(),
                "name  ·  text".into(),
                "lang  ·  text".into(),
            ])
            .rows(
                (1..=50)
                    .map(|i| {
                        vec![
                            i.to_string(),
                            format!("plugin-{i}"),
                            if i % 2 == 0 { "rust" } else { "wasm" }.to_owned(),
                        ]
                    })
                    .collect(),
            )
            .build();
        if let Some(row) = table.show(ui) {
            println!("clicked row {row}");
        }
    }

    fn json_tree_story(&mut self, ui: &mut egui::Ui) {
        ui.heading("JSON Tree");
        ui.add_space(8.0);
        let value = serde_json::json!({
            "name": "thoth",
            "version": "0.3.25",
            "tags": ["json", "viewer", "egui"],
            "meta": { "stars": 42, "active": true, "license": null },
        });
        JsonTree::builder()
            .value(value)
            .id("gallery-json".to_owned())
            .build()
            .show(ui);
    }
}

/// A small combo-box helper for picking one of several `Copy` enum variants.
fn combo<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: &str,
    current: &mut T,
    options: &[(T, &str)],
) {
    let selected_label = options
        .iter()
        .find(|(v, _)| v == current)
        .map(|(_, l)| *l)
        .unwrap_or("");
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected_label)
        .show_ui(ui, |ui| {
            for (value, label) in options {
                ui.selectable_value(current, *value, *label);
            }
        });
}

// ── Demo palettes ──────────────────────────────────────────────────────────
// Standalone palettes so the gallery doesn't depend on the host's theme crate.
// (Roughly Catppuccin Mocha / Latte.)

fn dark_palette() -> ThemeColors {
    let rgb = Color32::from_rgb;
    ThemeColors {
        bg: rgb(0x1e, 0x1e, 0x2e),
        bg_panel: rgb(0x18, 0x18, 0x25),
        bg_sunken: rgb(0x11, 0x11, 0x1b),
        surface: rgb(0x31, 0x32, 0x44),
        surface_raised: rgb(0x45, 0x47, 0x5a),
        surface_active: rgb(0x58, 0x5b, 0x70),
        fg: rgb(0xcd, 0xd6, 0xf4),
        fg_muted: rgb(0xa6, 0xad, 0xc8),
        syntax_key: rgb(0x89, 0xb4, 0xfa),
        syntax_string: rgb(0xa6, 0xe3, 0xa1),
        syntax_number: rgb(0xfa, 0xb3, 0x87),
        syntax_bool: rgb(0xf3, 0x8b, 0xa8),
        syntax_punctuation: rgb(0xba, 0xc2, 0xde),
        success: rgb(0xa6, 0xe3, 0xa1),
        warning: rgb(0xf9, 0xe2, 0xaf),
        error: rgb(0xf3, 0x8b, 0xa8),
        info: rgb(0x89, 0xdc, 0xeb),
        accent: rgb(0xcb, 0xa6, 0xf7),
        accent_secondary: rgb(0xf5, 0xc2, 0xe7),
        sidebar_hover: rgb(0x45, 0x47, 0x5a),
        sidebar_header: rgb(0xa6, 0xad, 0xc8),
        indent_guide: rgb(0x45, 0x47, 0x5a),
    }
}

fn light_palette() -> ThemeColors {
    let rgb = Color32::from_rgb;
    ThemeColors {
        bg: rgb(0xef, 0xf1, 0xf5),
        bg_panel: rgb(0xe6, 0xe9, 0xef),
        bg_sunken: rgb(0xdc, 0xe0, 0xe8),
        surface: rgb(0xcc, 0xd0, 0xda),
        surface_raised: rgb(0xbc, 0xc0, 0xcc),
        surface_active: rgb(0xac, 0xb0, 0xbe),
        fg: rgb(0x4c, 0x4f, 0x69),
        fg_muted: rgb(0x6c, 0x6f, 0x85),
        syntax_key: rgb(0x1e, 0x66, 0xf5),
        syntax_string: rgb(0x40, 0xa0, 0x2b),
        syntax_number: rgb(0xfe, 0x64, 0x0b),
        syntax_bool: rgb(0xd2, 0x0f, 0x39),
        syntax_punctuation: rgb(0x5c, 0x5f, 0x77),
        success: rgb(0x40, 0xa0, 0x2b),
        warning: rgb(0xdf, 0x8e, 0x1d),
        error: rgb(0xd2, 0x0f, 0x39),
        info: rgb(0x04, 0xa5, 0xe5),
        accent: rgb(0x88, 0x39, 0xef),
        accent_secondary: rgb(0xea, 0x76, 0xcb),
        sidebar_hover: rgb(0xbc, 0xc0, 0xcc),
        sidebar_header: rgb(0x6c, 0x6f, 0x85),
        indent_guide: rgb(0xbc, 0xc0, 0xcc),
    }
}
