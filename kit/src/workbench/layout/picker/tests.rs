use super::*;

#[test]
fn the_view_types_shrink_to_icons_then_to_a_menu() {
    let form = |w: f32, h: f32| picker_form(egui::vec2(w, h), 8);
    assert_eq!(form(216.0, 356.0), PickerForm::List);
    assert_eq!(form(800.0, 196.0), PickerForm::List);
    assert_eq!(form(215.0, 356.0), PickerForm::Icons);
    assert_eq!(form(800.0, 195.0), PickerForm::Icons);
    assert_eq!(form(200.0, 100.0), PickerForm::Icons);
    assert_eq!(form(300.0, 64.0), PickerForm::Icons);
    assert_eq!(form(300.0, 63.0), PickerForm::Menu);
    assert_eq!(form(100.0, 100.0), PickerForm::Menu);
    assert_eq!(form(40.0, 40.0), PickerForm::Menu);
    assert_eq!(form(0.0, 0.0), PickerForm::Menu);
}
