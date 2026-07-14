use oxiden_buffer::{Buffer, Position, Range, VecStorage};
use oxiden_core::{Command, Document, Editor};

fn editor_with(text: &str) -> Editor<VecStorage> {
    let mut buffer = Buffer::new(VecStorage::new());

    buffer.insert(Position::new(0, 0), text).unwrap();

    Editor::new(Document::new(buffer))
}

fn line(editor: &Editor<VecStorage>, index: usize) -> String {
    editor.document().buffer().line(index).unwrap().to_string()
}

// ===== MoveTo =====

#[test]
fn move_to_sets_cursor_position() {
    let mut editor = editor_with("Hello\nWorld");

    editor
        .execute(Command::MoveTo {
            position: Position::new(1, 3),
            vertical: false,
        })
        .unwrap();

    assert_eq!(editor.cursor().position(), Position::new(1, 3));
}

#[test]
fn move_to_clamps_out_of_range_line() {
    let mut editor = editor_with("Hello\nWorld");

    editor
        .execute(Command::MoveTo {
            position: Position::new(99, 0),
            vertical: false,
        })
        .unwrap();

    assert_eq!(editor.cursor().position(), Position::new(1, 0));
}

#[test]
fn move_to_clamps_out_of_range_column() {
    let mut editor = editor_with("Hello\nWorld");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 99),
            vertical: false,
        })
        .unwrap();

    assert_eq!(editor.cursor().position(), Position::new(0, 5));
}

// ===== Insert =====

#[test]
fn insert_char_writes_and_advances_cursor() {
    let mut editor = editor_with("");

    editor.execute(Command::Insert('a')).unwrap();
    editor.execute(Command::Insert('b')).unwrap();

    assert_eq!(line(&editor, 0), "ab");
    assert_eq!(editor.cursor().position(), Position::new(0, 2));
}

#[test]
fn insert_char_at_cursor_position() {
    let mut editor = editor_with("ac");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 1),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Insert('b')).unwrap();

    assert_eq!(line(&editor, 0), "abc");
    assert_eq!(editor.cursor().position(), Position::new(0, 2));
}

// ===== InsertText =====

#[test]
fn insert_text_single_line_advances_cursor_by_char_count() {
    let mut editor = editor_with("");

    editor.execute(Command::InsertText("Hello".to_string())).unwrap();

    assert_eq!(line(&editor, 0), "Hello");
    assert_eq!(editor.cursor().position(), Position::new(0, 5));
}

#[test]
fn insert_text_multiline_places_cursor_on_last_line() {
    let mut editor = editor_with("");

    editor.execute(Command::InsertText("one\ntwo\nthree".to_string())).unwrap();

    assert_eq!(editor.document().buffer().line_count(), 3);
    assert_eq!(line(&editor, 0), "one");
    assert_eq!(line(&editor, 1), "two");
    assert_eq!(line(&editor, 2), "three");
    assert_eq!(editor.cursor().position(), Position::new(2, 5));
}

#[test]
fn insert_text_multiline_in_middle_of_line_splits_tail_onto_last_part() {
    let mut editor = editor_with("HelloWorld");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 5),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::InsertText("\n".to_string())).unwrap();

    assert_eq!(line(&editor, 0), "Hello");
    assert_eq!(line(&editor, 1), "World");
    assert_eq!(editor.cursor().position(), Position::new(1, 0));
}

#[test]
fn insert_text_unicode_advances_cursor_by_char_not_byte_count() {
    let mut editor = editor_with("");

    editor.execute(Command::InsertText("😀😁".to_string())).unwrap();

    assert_eq!(editor.cursor().position(), Position::new(0, 2));
}

// ===== Backspace =====

#[test]
fn backspace_at_document_start_is_noop() {
    let mut editor = editor_with("abc");

    editor.execute(Command::Backspace).unwrap();

    assert_eq!(line(&editor, 0), "abc");
    assert_eq!(editor.cursor().position(), Position::new(0, 0));
}

#[test]
fn backspace_within_line_deletes_previous_char_and_moves_cursor_left() {
    let mut editor = editor_with("abc");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 2),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Backspace).unwrap();

    assert_eq!(line(&editor, 0), "ac");
    assert_eq!(editor.cursor().position(), Position::new(0, 1));
}

#[test]
fn backspace_at_column_zero_joins_with_previous_line() {
    let mut editor = editor_with("Hello\nWorld");

    editor
        .execute(Command::MoveTo {
            position: Position::new(1, 0),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Backspace).unwrap();

    assert_eq!(editor.document().buffer().line_count(), 1);
    assert_eq!(line(&editor, 0), "HelloWorld");
    assert_eq!(editor.cursor().position(), Position::new(0, 5));
}

// ===== Delete =====

#[test]
fn delete_at_document_end_is_noop() {
    let mut editor = editor_with("abc");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 3),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Delete).unwrap();

    assert_eq!(line(&editor, 0), "abc");
    assert_eq!(editor.cursor().position(), Position::new(0, 3));
}

#[test]
fn delete_within_line_removes_next_char_and_keeps_cursor() {
    let mut editor = editor_with("abc");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 1),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Delete).unwrap();

    assert_eq!(line(&editor, 0), "ac");
    assert_eq!(editor.cursor().position(), Position::new(0, 1));
}

#[test]
fn delete_at_end_of_line_joins_with_next_line() {
    let mut editor = editor_with("Hello\nWorld");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 5),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Delete).unwrap();

    assert_eq!(editor.document().buffer().line_count(), 1);
    assert_eq!(line(&editor, 0), "HelloWorld");
    assert_eq!(editor.cursor().position(), Position::new(0, 5));
}

// ===== DeleteRange =====

#[test]
fn delete_range_removes_text_and_places_cursor_at_start() {
    let mut editor = editor_with("Hello World");

    editor
        .execute(Command::DeleteRange(Range::new(
            Position::new(0, 5),
            Position::new(0, 11),
        )))
        .unwrap();

    assert_eq!(line(&editor, 0), "Hello");
    assert_eq!(editor.cursor().position(), Position::new(0, 5));
}

#[test]
fn delete_range_with_reversed_bounds_still_places_cursor_at_earliest_position()
{
    let mut editor = editor_with("Hello World");

    // Bounds given in reverse (end before start), as a selection dragged
    // right-to-left would produce.
    editor
        .execute(Command::DeleteRange(Range::new(
            Position::new(0, 11),
            Position::new(0, 5),
        )))
        .unwrap();

    assert_eq!(line(&editor, 0), "Hello");
    assert_eq!(editor.cursor().position(), Position::new(0, 5));
}

#[test]
fn delete_range_with_invalid_position_returns_error() {
    let mut editor = editor_with("abc");

    let result = editor.execute(Command::DeleteRange(Range::new(
        Position::new(0, 0),
        Position::new(5, 0),
    )));

    assert!(result.is_err());
}

// ===== NewLine =====

#[test]
fn new_line_splits_current_line_and_moves_cursor_to_next_line_start() {
    let mut editor = editor_with("HelloWorld");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 5),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::NewLine).unwrap();

    assert_eq!(editor.document().buffer().line_count(), 2);
    assert_eq!(line(&editor, 0), "Hello");
    assert_eq!(line(&editor, 1), "World");
    assert_eq!(editor.cursor().position(), Position::new(1, 0));
}

// ===== Document dirty tracking =====

#[test]
fn editing_marks_document_dirty() {
    let mut editor = editor_with("abc");

    assert!(!editor.document().is_dirty());

    editor.execute(Command::Insert('!')).unwrap();

    assert!(editor.document().is_dirty());
}

// ===== Undo/Redo =====

#[test]
fn undo_reverts_a_single_insert() {
    let mut editor = editor_with("ab");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 1),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Insert('X')).unwrap();
    assert_eq!(line(&editor, 0), "aXb");

    editor.execute(Command::Undo).unwrap();

    assert_eq!(line(&editor, 0), "ab");
    assert_eq!(editor.cursor().position(), Position::new(0, 1));
}

#[test]
fn redo_reapplies_an_undone_insert() {
    let mut editor = editor_with("ab");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 1),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Insert('X')).unwrap();
    editor.execute(Command::Undo).unwrap();
    editor.execute(Command::Redo).unwrap();

    assert_eq!(line(&editor, 0), "aXb");
    assert_eq!(editor.cursor().position(), Position::new(0, 2));
}

#[test]
fn consecutive_typing_undoes_in_one_step() {
    let mut editor = editor_with("");

    editor.execute(Command::Insert('h')).unwrap();
    editor.execute(Command::Insert('i')).unwrap();
    editor.execute(Command::Insert('!')).unwrap();
    assert_eq!(line(&editor, 0), "hi!");

    editor.execute(Command::Undo).unwrap();

    assert_eq!(line(&editor, 0), "");
}

#[test]
fn moving_the_cursor_between_edits_breaks_the_undo_group() {
    let mut editor = editor_with("");

    editor.execute(Command::Insert('a')).unwrap();
    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 1),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Insert('b')).unwrap();
    assert_eq!(line(&editor, 0), "ab");

    editor.execute(Command::Undo).unwrap();
    assert_eq!(line(&editor, 0), "a");

    editor.execute(Command::Undo).unwrap();
    assert_eq!(line(&editor, 0), "");
}

#[test]
fn undo_reverts_a_backspace() {
    let mut editor = editor_with("abc");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 3),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::Backspace).unwrap();
    assert_eq!(line(&editor, 0), "ab");

    editor.execute(Command::Undo).unwrap();

    assert_eq!(line(&editor, 0), "abc");
    assert_eq!(editor.cursor().position(), Position::new(0, 3));
}

#[test]
fn undo_reverts_a_newline_and_rejoins_the_lines() {
    let mut editor = editor_with("HelloWorld");

    editor
        .execute(Command::MoveTo {
            position: Position::new(0, 5),
            vertical: false,
        })
        .unwrap();
    editor.execute(Command::NewLine).unwrap();
    assert_eq!(editor.document().buffer().line_count(), 2);

    editor.execute(Command::Undo).unwrap();

    assert_eq!(editor.document().buffer().line_count(), 1);
    assert_eq!(line(&editor, 0), "HelloWorld");
    assert_eq!(editor.cursor().position(), Position::new(0, 5));
}

#[test]
fn undo_reverts_a_delete_range() {
    let mut editor = editor_with("Hello World");

    editor
        .execute(Command::DeleteRange(Range::new(
            Position::new(0, 5),
            Position::new(0, 11),
        )))
        .unwrap();
    assert_eq!(line(&editor, 0), "Hello");

    editor.execute(Command::Undo).unwrap();

    assert_eq!(line(&editor, 0), "Hello World");
}

#[test]
fn undo_with_nothing_to_undo_is_a_no_op() {
    let mut editor = editor_with("abc");

    editor.execute(Command::Undo).unwrap();

    assert_eq!(line(&editor, 0), "abc");
    assert_eq!(editor.cursor().position(), Position::new(0, 0));
}

#[test]
fn redo_with_nothing_to_redo_is_a_no_op() {
    let mut editor = editor_with("abc");

    editor.execute(Command::Redo).unwrap();

    assert_eq!(line(&editor, 0), "abc");
}

#[test]
fn new_edit_after_undo_clears_the_redo_stack() {
    let mut editor = editor_with("");

    editor.execute(Command::Insert('a')).unwrap();
    editor.execute(Command::Undo).unwrap();
    editor.execute(Command::Insert('b')).unwrap();

    // The redone-then-overwritten "a" should be gone for good.
    editor.execute(Command::Redo).unwrap();

    assert_eq!(line(&editor, 0), "b");
}
