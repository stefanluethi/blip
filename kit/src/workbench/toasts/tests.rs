use super::*;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[test]
fn toasts_expire_after_they_are_shown() {
    let mut toasts = Toasts::default();
    toasts.info("saved");
    toasts.expire(100.0);
    assert_eq!(toasts.toasts.len(), 1);
    toasts.warning("cut off");
    toasts.expire(100.0 + DURATION / 2.0);
    assert_eq!(toasts.toasts.len(), 2);
    toasts.expire(100.0 + DURATION);
    assert_eq!(toasts.toasts.len(), 1);
    assert_eq!(toasts.toasts[0].text, "cut off");
}

#[test]
fn errors_stay_until_they_are_closed() {
    let mut toasts = Toasts::default();
    toasts.error("refused");
    toasts.expire(0.0);
    toasts.expire(10.0 * DURATION);
    assert_eq!(toasts.shown(), [(Level::Error, "refused")]);
}

#[test]
fn a_repeated_toast_is_counted_and_shown_for_longer() {
    let mut toasts = Toasts::default();
    toasts.warning("refused");
    toasts.expire(0.0);
    toasts.warning("refused");
    toasts.error("refused");
    toasts.expire(DURATION / 2.0);
    toasts.expire(DURATION);
    assert_eq!(toasts.toasts.len(), 2);
    assert_eq!(toasts.toasts[0].count, 2);

    for n in 0..MAX_TOASTS + 2 {
        toasts.error(n.to_string());
    }
    assert_eq!(toasts.toasts.len(), MAX_TOASTS);
    assert_eq!(toasts.toasts[0].text, "2");
}

#[test]
fn notices_of_the_store_keep_their_level() {
    let mut toasts = Toasts::default();
    toasts.notice(crate::store::Level::Warning, "cut off");
    toasts.notice(crate::store::Level::Error, "broke");
    assert_eq!(
        toasts.shown(),
        [(Level::Warning, "cut off"), (Level::Error, "broke")]
    );
}

fn harness() -> Harness<'static, Toasts> {
    Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, toasts: &mut Toasts| {
                toasts.ui(ui.ctx(), ui.max_rect(), None);
            },
            Toasts::default(),
        )
}

#[test]
fn a_toast_is_shown_until_it_is_closed() {
    let mut harness = harness();
    harness.state_mut().error("ws://a cannot be connected");
    harness.run();
    assert!(
        harness
            .query_by_label_contains("cannot be connected")
            .is_some()
    );
    harness.get_by_label(X).click();
    harness.run();
    assert!(harness.state().toasts.is_empty());
    assert!(
        harness
            .query_by_label_contains("cannot be connected")
            .is_none()
    );
}

#[test]
fn the_action_of_a_toast_is_given_back_once_clicked() {
    let mut harness = harness();
    harness
        .state_mut()
        .with_action(Level::Info, "Deleted Speed", "Undo", 7);
    // a second one with the same text has its own button
    harness
        .state_mut()
        .with_action(Level::Info, "Deleted Speed", "Undo", 8);
    harness.run();
    assert!(harness.state_mut().take_actions().is_empty());
    let mut undo = harness.get_all_by_label("Undo");
    undo.next().expect("an Undo button").click();
    drop(undo);
    harness.run();
    assert_eq!(harness.state_mut().take_actions(), [7]);
    assert!(harness.state_mut().take_actions().is_empty());
    assert_eq!(harness.state().toasts.len(), 1);
}

#[test]
fn the_close_button_is_as_high_as_the_controls() {
    let mut harness = harness();
    harness.state_mut().info("Saved a.blip");
    harness.run();
    let close = harness.get_by_label(X).rect();
    assert!(close.height() >= theme::CONTROL_HEIGHT, "{close:?}");
}

#[test]
fn a_toast_is_dismissed_by_the_value_of_its_action() {
    let mut toasts = Toasts::default();
    toasts.with_action(Level::Info, "Deleted Speed", "Undo", 1);
    toasts.with_action(Level::Info, "Deleted Logs", "Undo", 2);
    toasts.info("Saved");
    toasts.dismiss(1);
    let texts: Vec<&str> = toasts.shown().into_iter().map(|(_, text)| text).collect();
    assert_eq!(texts, ["Deleted Logs", "Saved"]);
}
