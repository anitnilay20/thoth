use eframe::egui::{self, Color32};

pub fn apply_theme(ctx: &egui::Context, dark: bool) {
    if dark {
        ctx.set_visuals(andromeda_visuals());
        // small spacing/rounding polish
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        // style.visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
        // style.visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
        // style.visuals.widgets.active.rounding = egui::Rounding::same(6.0);
        ctx.set_style(style);
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }
}

fn andromeda_visuals() -> egui::Visuals {
    use egui::Color32 as C;
    let mut v = egui::Visuals::dark();

    // Andromeda-ish palette
    let bg0 = C::from_rgb(0x1f, 0x22, 0x30); // window bg
    let bg1 = C::from_rgb(0x23, 0x26, 0x2e); // panels / cards
    let bg2 = C::from_rgb(0x1b, 0x1e, 0x28); // alt rows
    let txt = C::from_rgb(0xe6, 0xe6, 0xe6);
    let acc = C::from_rgb(0x82, 0xaa, 0xff);

    v.override_text_color = Some(txt);
    v.panel_fill = bg0;
    v.extreme_bg_color = bg1;
    v.widgets.noninteractive.bg_fill = bg1;
    v.widgets.inactive.bg_fill = bg1;
    v.widgets.hovered.bg_fill = C::from_rgb(0x2a, 0x2f, 0x3c);
    v.widgets.active.bg_fill = C::from_rgb(0x34, 0x3a, 0x49);
    v.faint_bg_color = bg2;

    v.selection.bg_fill = acc;
    v.selection.stroke = egui::Stroke::new(1.0, acc);
    v.hyperlink_color = acc;
    // v.window_rounding = egui::Rounding::same(10.0);
    v
}

pub fn row_fill(i: usize, ui: &egui::Ui) -> Color32 {
    if i % 2 == 1 {
        // Only paint odd rows
        if ui.visuals().dark_mode {
            Color32::from_rgb(0x25, 0x28, 0x33) // lighter stripe
        } else {
            Color32::from_rgb(0xEC, 0xEE, 0xF3) // light stripe for light mode
        }
    } else {
        Color32::TRANSPARENT // even rows = "no fill"
    }
}
