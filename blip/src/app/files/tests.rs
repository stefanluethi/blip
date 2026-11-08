use super::*;

#[test]
fn read_names_the_file_it_cannot_read() {
    let error = File::read("a.blip".into(), Err("denied".into()));
    assert_eq!(error.err().unwrap(), "a.blip cannot be read: denied");
    let file = File::read("a.blip".into(), Ok(vec![1])).unwrap();
    assert_eq!(file.name, "a.blip");
    assert_eq!(file.bytes, [1]);
}
