use super::*;

#[test]
fn the_first_traces_take_the_palette_and_later_ones_still_differ() {
    let visuals = Visuals::dark();
    assert_eq!(trace_color(&visuals, 0), DARK.traces[0]);
    let colors: std::collections::BTreeSet<[u8; 4]> = (0..12)
        .map(|i| trace_color(&visuals, i).to_array())
        .collect();
    assert_eq!(colors.len(), 12);
}

#[test]
fn the_palette_wraps_without_the_grey() {
    let visuals = Visuals::light();
    assert_eq!(palette_color(&visuals, 5), LIGHT.traces[0]);
    assert!((0..5).all(|i| palette_color(&visuals, i) != palette_grey(&visuals)));
}

/// The rects of the widget `add` adds while resting, hovered and pressed.
fn rects_by_state(add: impl Fn(&mut egui::Ui) -> egui::Response) -> [egui::Rect; 3] {
    let ctx = egui::Context::default();
    install(&ctx);
    let frame = |events: Vec<egui::Event>| {
        let input = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 300.0),
            )),
            ..Default::default()
        };
        let mut rect = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(input, |ui| rect = add(ui).rect);
        output.textures_delta.clear();
        rect
    };
    frame(vec![]);
    let resting = frame(vec![]);
    let pos = resting.center();
    frame(vec![egui::Event::PointerMoved(pos)]);
    let hovered = frame(vec![]);
    frame(vec![egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    let pressed = frame(vec![]);
    [resting, hovered, pressed]
}

#[test]
fn choices_share_their_row_equally_unless_a_long_one_needs_more() {
    // all fit equally
    assert_eq!(share_widths(&[30.0, 40.0, 45.0], 150.0), [50.0; 3]);
    // the long one gets its width, the others share the rest equally
    assert_eq!(share_widths(&[30.0, 40.0, 70.0], 150.0), [40.0, 40.0, 70.0]);
    // and a short one that then needs more too
    assert_eq!(share_widths(&[30.0, 42.0, 70.0], 150.0), [38.0, 42.0, 70.0]);
    // in a panel too narrow for them, all are equal and cut off
    assert_eq!(share_widths(&[30.0, 40.0, 70.0], 120.0), [40.0; 3]);

    // measured in a ui
    let ctx = egui::Context::default();
    install(&ctx);
    let mut widths = vec![];
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(400.0, 300.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run_ui(input, |ui| {
        for room in [180.0, 60.0] {
            ui.allocate_ui(egui::vec2(room, 24.0), |ui| {
                let long = [(0, "Off"), (1, "Mean"), (2, "Mean ±σ")];
                widths.push(segment_widths(ui, &long));
            });
        }
    });
    let [wide, narrow] = [&widths[0], &widths[1]];
    assert!(wide[2] > wide[1] && wide[1] == wide[0], "{wide:?}");
    assert!(
        (wide.iter().sum::<f32>() - (180.0 - 4.0)).abs() < 0.5,
        "{wide:?}"
    );
    assert!(narrow.iter().all(|&width| width == narrow[0]), "{narrow:?}");
    assert!(
        (narrow.iter().sum::<f32>() - (60.0 - 4.0)).abs() < 0.5,
        "{narrow:?}"
    );
}

#[test]
fn buttons_framed_only_while_hovered_keep_their_size() {
    let [resting, hovered, pressed] = rects_by_state(|ui| ui.add(icon_button("+")));
    assert_eq!(resting, hovered);
    assert_eq!(resting, pressed);
    let [resting, hovered, pressed] = rects_by_state(|ui| ui.add(selectable(false, "Choice")));
    assert_eq!(resting, hovered);
    assert_eq!(resting, pressed);
}
