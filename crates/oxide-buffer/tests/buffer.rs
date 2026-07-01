use oxide_buffer::{Buffer, Position, Range, VecStorage};

#[test]
fn insert_and_delete() {
    let mut buffer = Buffer::new(VecStorage::new());

    buffer.insert(Position::new(0, 0), "Hello").unwrap();
    buffer.insert(Position::new(0, 5), " World").unwrap();

    assert_eq!(buffer.line(0), Some("Hello World"));

    buffer
        .delete(Range::new(Position::new(0, 5), Position::new(0, 6)))
        .unwrap();

    assert_eq!(buffer.line(0), Some("HelloWorld"));
}

#[test]
fn multiline_insert() {
    let mut buffer = Buffer::new(VecStorage::new());

    buffer.insert(Position::new(0, 0), "one\ntwo\nthree").unwrap();

    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.line(0), Some("one"));
    assert_eq!(buffer.line(1), Some("two"));
    assert_eq!(buffer.line(2), Some("three"));
}

#[test]
fn multiline_delete() {
    let mut buffer = Buffer::new(VecStorage::new());

    buffer.insert(Position::new(0, 0), "Hello\nBeautiful\nWorld").unwrap();

    buffer
        .delete(Range::new(Position::new(0, 2), Position::new(2, 3)))
        .unwrap();

    assert_eq!(buffer.line_count(), 1);
    assert_eq!(buffer.line(0), Some("Held"));
}

#[test]
fn unicode_roundtrip() {
    let mut buffer = Buffer::new(VecStorage::new());

    buffer.insert(Position::new(0, 0), "😀 Rust 🚀").unwrap();

    assert_eq!(buffer.line(0), Some("😀 Rust 🚀"));
    assert_eq!(buffer.len_chars(), "😀 Rust 🚀".chars().count());
}

#[test]
fn invalid_position_propagates() {
    let mut buffer = Buffer::new(VecStorage::new());

    assert!(buffer.insert(Position::new(42, 0), "hello").is_err());
}
