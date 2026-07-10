use oxiden_buffer::{Buffer, Position, Range, RopeyStorage};

#[test]
fn insert_and_delete() {
    let mut buffer = Buffer::new(RopeyStorage::new());

    buffer.insert(Position::new(0, 0), "Hello").unwrap();
    buffer.insert(Position::new(0, 5), " World").unwrap();

    assert_eq!(buffer.line(0), Some("Hello World".to_string()));

    buffer
        .delete(Range::new(Position::new(0, 5), Position::new(0, 6)))
        .unwrap();

    assert_eq!(buffer.line(0), Some("HelloWorld".to_string()));
}

#[test]
fn multiline_insert() {
    let mut buffer = Buffer::new(RopeyStorage::new());

    buffer.insert(Position::new(0, 0), "one\ntwo\nthree").unwrap();

    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.line(0), Some("one".to_string()));
    assert_eq!(buffer.line(1), Some("two".to_string()));
    assert_eq!(buffer.line(2), Some("three".to_string()));
}

#[test]
fn multiline_delete() {
    let mut buffer = Buffer::new(RopeyStorage::new());

    buffer.insert(Position::new(0, 0), "Hello\nBeautiful\nWorld").unwrap();

    buffer
        .delete(Range::new(Position::new(0, 2), Position::new(2, 3)))
        .unwrap();

    assert_eq!(buffer.line_count(), 1);
    assert_eq!(buffer.line(0), Some("Held".to_string()));
}

#[test]
fn unicode_roundtrip() {
    let mut buffer = Buffer::new(RopeyStorage::new());

    buffer.insert(Position::new(0, 0), "😀 Rust 🚀").unwrap();

    assert_eq!(buffer.line(0), Some("😀 Rust 🚀".to_string()));
    assert_eq!(buffer.len_chars(), "😀 Rust 🚀".chars().count());
}

#[test]
fn invalid_position_propagates() {
    let mut buffer = Buffer::new(RopeyStorage::new());

    assert!(buffer.insert(Position::new(42, 0), "hello").is_err());
}

#[test]
fn large_document_survives_many_edits() {
    let mut buffer = Buffer::new(RopeyStorage::new());

    let paragraph = "Lorem ipsum dolor sit amet, consectetur adipiscing.";
    let lines: Vec<&str> = std::iter::repeat_n(paragraph, 500).collect();
    let text = lines.join("\n");

    buffer.insert(Position::new(0, 0), &text).unwrap();
    assert_eq!(buffer.line_count(), 500);
    assert_eq!(buffer.to_text(), text);

    buffer.insert(Position::new(250, 10), "EDIT").unwrap();
    buffer
        .delete(Range::new(Position::new(250, 10), Position::new(250, 14)))
        .unwrap();

    assert_eq!(buffer.to_text(), text);
}
