use super::super::tests::nested;
use super::*;

#[test]
fn the_nearest_side_is_relative_to_the_size() {
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 100.0));
    assert_eq!(nearest_side(rect, egui::pos2(30.0, 50.0)), Side::Left);
    assert_eq!(nearest_side(rect, egui::pos2(390.0, 50.0)), Side::Right);
    // 20 px from the top is nearer than 100 px from the left of the wide card
    assert_eq!(nearest_side(rect, egui::pos2(100.0, 20.0)), Side::Top);
    assert_eq!(nearest_side(rect, egui::pos2(200.0, 90.0)), Side::Bottom);
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
}

#[test]
fn the_middle_of_a_view_is_inset_by_a_quarter_and_at_least_40_px() {
    assert_eq!(
        middle(rect(0.0, 0.0, 100.0, 100.0)),
        rect(25.0, 25.0, 50.0, 50.0)
    );
    assert_eq!(
        middle(rect(0.0, 0.0, 120.0, 60.0)),
        rect(30.0, 10.0, 60.0, 40.0)
    );
    assert_eq!(
        middle(rect(0.0, 0.0, 60.0, 60.0)),
        rect(10.0, 10.0, 40.0, 40.0)
    );
    assert_eq!(
        middle(rect(0.0, 0.0, 30.0, 200.0)),
        rect(0.0, 50.0, 30.0, 100.0)
    );
}

#[test]
fn a_card_takes_the_middle_as_a_whole_and_the_rest_by_side() {
    let card = rect(0.0, 0.0, 100.0, 100.0);
    assert_eq!(drop_on_card(card, egui::pos2(50.0, 50.0)), Drop::Into);
    assert_eq!(drop_on_card(card, egui::pos2(26.0, 50.0)), Drop::Into);
    assert_eq!(
        drop_on_card(card, egui::pos2(20.0, 50.0)),
        Drop::Side(Side::Left)
    );
    assert_eq!(
        drop_on_card(card, egui::pos2(50.0, 90.0)),
        Drop::Side(Side::Bottom)
    );
    let card = rect(0.0, 0.0, 120.0, 60.0);
    assert_eq!(drop_on_card(card, egui::pos2(60.0, 12.0)), Drop::Into);
    assert_eq!(
        drop_on_card(card, egui::pos2(60.0, 8.0)),
        Drop::Side(Side::Top)
    );
}

#[test]
fn the_marker_is_the_half_at_a_side_and_the_whole_view_in_the_middle() {
    let n = nested();
    let card = rect(0.0, 0.0, 100.0, 100.0);
    let shown = Shown {
        cards: vec![(n.table, card)],
        tab_buttons: vec![],
        tab_rows: vec![],
    };
    let side = n
        .layout
        .drop_target(n.xy, &shown, egui::pos2(10.0, 50.0))
        .unwrap();
    assert_eq!(marked_area(&side), Some(rect(0.0, 0.0, 50.0, 100.0)));
    assert_eq!(marked_line(&side), None);
    let middle = n
        .layout
        .drop_target(n.xy, &shown, egui::pos2(50.0, 50.0))
        .unwrap();
    assert_eq!(middle.drop, Drop::Into);
    assert_eq!(marked_area(&middle), Some(card));
    assert_eq!(marked_line(&middle), None);
}

#[test]
fn the_empty_part_of_a_tab_row_takes_the_dragged_one_as_last_tab() {
    let n = nested();
    let row = rect(300.0, 0.0, 100.0, 28.0);
    let shown = Shown {
        cards: vec![],
        tab_buttons: vec![(n.states, rect(200.0, 0.0, 100.0, 28.0))],
        tab_rows: vec![(n.tabs3, row)],
    };
    let target = n
        .layout
        .drop_target(n.xy, &shown, egui::pos2(350.0, 14.0))
        .unwrap();
    assert_eq!((target.tile_id, target.drop), (n.tabs3, Drop::Into));
    assert!(target.on_tab_row);
    assert_eq!(marked_area(&target), Some(row));
    assert_eq!(
        marked_line(&target),
        Some([egui::pos2(300.0, 0.0), egui::pos2(300.0, 28.0)])
    );
    let tab = n
        .layout
        .drop_target(n.xy, &shown, egui::pos2(260.0, 14.0))
        .unwrap();
    assert_eq!((tab.tile_id, tab.drop), (n.states, Drop::After));
    assert!(
        n.layout
            .drop_target(n.tabs3, &shown, egui::pos2(350.0, 14.0))
            .is_none()
    );
}
