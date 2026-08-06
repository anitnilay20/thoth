use eframe::egui;
use thoth::components::welcome::WelcomePanel;

fn run(size: egui::Vec2, recent: &[String]) -> (Vec<egui::Rect>, egui::Rect) {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts.families.insert(
        egui::FontFamily::Name("phosphor".into()),
        vec!["phosphor".into()],
    );
    ctx.set_fonts(fonts);
    let mut shapes_rect = egui::Rect::NOTHING;
    let mut rects = Vec::new();
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        rects.clear();
        let out = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(ctx, |ui| {
                    let _ = WelcomePanel::render(ui, recent, None);
                    rects.push(ui.min_rect());
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
    (rects.clone(), shapes_rect)
}

#[test]
fn tall_viewport_centres_the_block() {
    let recent: Vec<String> = vec![
        "/tmp/os_dispatch.json".into(),
        "/tmp/users.ndjson".into(),
        "/tmp/revenue.csv".into(),
    ];
    let size = egui::vec2(1200.0, 900.0);
    let (_, painted) = run(size, &recent);
    println!("tall painted = {painted:?}");
    let top_gap = painted.top();
    let bottom_gap = size.y - painted.bottom();
    assert!(
        (top_gap - bottom_gap).abs() < 24.0,
        "block should be vertically centred: top {top_gap} vs bottom {bottom_gap}"
    );
    // 980-wide wrap centred in a 1200 pane: content starts at 110+44 = 154, and
    // the mark's 18px glow blur bleeds ~9px to the left of it.
    assert!(
        (painted.left() - 154.0).abs() < 12.0,
        "wrap should be centred horizontally, got left {}",
        painted.left()
    );
    assert!(
        ((1200.0 - painted.right()) - 154.0).abs() < 12.0,
        "wrap right edge should mirror the left, got right {}",
        painted.right()
    );
}

#[test]
fn short_viewport_scrolls_from_the_top() {
    let recent: Vec<String> = vec!["/tmp/a.json".into()];
    let size = egui::vec2(900.0, 320.0);
    let (_, painted) = run(size, &recent);
    println!("short painted = {painted:?}");
    assert!(
        painted.top() >= -1.0 && painted.top() < 45.0,
        "content must start at the top when it overflows, got {}",
        painted.top()
    );
}

#[test]
fn empty_recent_list_is_fine() {
    let (_, painted) = run(egui::vec2(700.0, 600.0), &[]);
    println!("narrow painted = {painted:?}");
    assert!(painted.is_positive());
}
