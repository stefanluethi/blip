use super::*;

#[test]
fn read_names_the_file_it_cannot_read() {
    let error = File::read("a.blip".into(), Err("denied".into()));
    assert_eq!(error.err().unwrap(), "a.blip cannot be read: denied");
    let file = File::read("a.blip".into(), Ok(vec![1])).unwrap();
    assert_eq!(file.name, "a.blip");
    assert_eq!(file.bytes, [1]);
}

#[test]
fn a_save_label_has_an_ellipsis_only_where_a_dialog_follows() {
    let label = save_label("Save recording");
    if cfg!(target_arch = "wasm32") {
        assert_eq!(label, "Save recording");
    } else {
        assert_eq!(label, "Save recording…");
    }
}
