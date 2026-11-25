use super::*;

#[test]
fn title_legend_entries_are_left_out_only_when_they_do_not_fit() {
    // each entry 30 wide besides its name of 40, "…" 10
    let entries = [(30.0, 40.0), (30.0, 40.0), (30.0, 40.0)];
    assert_eq!(fit_title_legend(&entries, 210.0, 10.0), [40.0, 40.0, 40.0]);
    // the third one is cut off where its name keeps 16 or more, with room for no "…"
    assert_eq!(fit_title_legend(&entries, 200.0, 10.0), [40.0, 40.0, 30.0]);
    // else left out for "…"
    assert_eq!(fit_title_legend(&entries, 150.0, 10.0), [40.0, 40.0]);
    // the second one is cut off before the "…" of the third
    assert_eq!(fit_title_legend(&entries, 130.0, 10.0), [40.0, 20.0]);
    assert_eq!(fit_title_legend(&entries, 40.0, 10.0), Vec::<f32>::new());
}
