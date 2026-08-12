use eframe::egui;
use thoth::components::welcome::{MARK_GLOW_BLUR, WRAP_MAX_W, WRAP_PAD_X, WelcomePanel};

/// `.wrap{max-width:WRAP_MAX_W;margin:0 auto;padding:… WRAP_PAD_X}` — how far the
/// wrap's content sits from the pane's edge at a given pane width.
fn wrap_inset(pane_w: f32) -> f32 {
    (pane_w - pane_w.min(WRAP_MAX_W)) / 2.0 + WRAP_PAD_X
}

fn run(size: egui::Vec2, recent: &[String]) -> egui::Rect {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts.families.insert(
        egui::FontFamily::Name("phosphor".into()),
        vec!["phosphor".into()],
    );
    ctx.set_fonts(fonts);
    let mut shapes_rect = egui::Rect::NOTHING;
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        let out = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(ctx, |ui| {
                    let _ = WelcomePanel::render(ui, recent, None);
                });
        });
        // egui reports an id clash by painting a "🔥 …" debug label.
        for clipped in &out.shapes {
            if let egui::Shape::Text(t) = &clipped.shape {
                assert!(
                    !t.galley.text().starts_with('🔥'),
                    "egui debug error painted: {}",
                    t.galley.text()
                );
            }
        }

        // Union of every painted shape, to see where content actually landed.
        let mut union = egui::Rect::NOTHING;
        for prim in ctx.tessellate(out.shapes, out.pixels_per_point) {
            let bounds = match &prim.primitive {
                egui::epaint::Primitive::Mesh(m) => m.calc_bounds(),
                egui::epaint::Primitive::Callback(cb) => cb.rect,
            };
            union = union.union(prim.clip_rect.intersect(bounds));
        }
        shapes_rect = union;
    }
    shapes_rect
}

#[test]
fn tall_viewport_centres_the_block() {
    let recent: Vec<String> = vec![
        "/tmp/os_dispatch.json".into(),
        "/tmp/users.ndjson".into(),
        "/tmp/revenue.csv".into(),
    ];
    let size = egui::vec2(1200.0, 900.0);
    let painted = run(size, &recent);
    let top_gap = painted.top();
    let bottom_gap = size.y - painted.bottom();
    assert!(
        (top_gap - bottom_gap).abs() < 24.0,
        "block should be vertically centred: top {top_gap} vs bottom {bottom_gap}"
    );
    // The wrap is centred in the pane; the tolerance is the mark's glow, which
    // bleeds outwards by up to its blur radius.
    let inset = wrap_inset(size.x);
    let tol = f32::from(MARK_GLOW_BLUR);
    assert!(
        (painted.left() - inset).abs() < tol,
        "wrap should be centred horizontally at WRAP_MAX_W/WRAP_PAD_X inset {inset}, got left {}",
        painted.left()
    );
    assert!(
        ((size.x - painted.right()) - inset).abs() < tol,
        "wrap right edge should mirror the left inset {inset}, got right {}",
        painted.right()
    );
}

#[test]
fn short_viewport_scrolls_from_the_top() {
    let recent: Vec<String> = vec!["/tmp/a.json".into()];
    let size = egui::vec2(900.0, 320.0);
    let painted = run(size, &recent);
    assert!(
        painted.top() >= -1.0 && painted.top() < 45.0,
        "content must start at the top when it overflows, got {}",
        painted.top()
    );
}

#[test]
fn empty_recent_list_is_fine() {
    let painted = run(egui::vec2(700.0, 600.0), &[]);
    assert!(painted.is_positive());
}
