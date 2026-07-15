//! A [`TextStorage`] backend built on a rope: a binary tree of text chunks
//! rather than one contiguous allocation (or one `String` per line, as
//! [`super::VecStorage`] uses).
//!
//! Unlike `VecStorage`, the document's text — newlines included — lives in
//! the tree itself; there is no separate per-line `Vec`. Each internal node
//! caches the character count and newline count of its left subtree, which
//! is enough to translate a `(line, column)` [`Position`] into a global
//! character offset, or a character range into borrowed text, in
//! `O(log n)` tree descents rather than a full scan. Leaves are capped at
//! [`MAX_LEAF_LEN`] characters: an insert that would grow a leaf past the
//! cap rebuilds just that leaf into a small balanced subtree, which is what
//! keeps a rope's edit cost close to `O(log n)` instead of the `O(n)`
//! reallocation a single giant `String` would need.
//!
//! This is a reasonable trade-off for large documents and edits away from
//! the end of a line, at the cost of more bookkeeping than `VecStorage` and
//! (since a logical line's text isn't guaranteed to live in one leaf) no
//! borrowed [`TextStorage::Line`] — `line()` always returns an owned
//! `String`.

use crate::{BufferError, Position, Range, Result, TextStorage};

/// Leaves are split once their character count exceeds this. Keeps a single
/// edit's leaf-level work bounded, independent of overall document size.
const MAX_LEAF_LEN: usize = 1024;

/// Subtrees smaller than this are never rebalanced, regardless of how
/// skewed they are: at this size the depth difference a rebuild would buy
/// back is negligible, so it's not worth paying for.
const REBALANCE_MIN_CHARS: usize = 2 * MAX_LEAF_LEN;

/// How skewed a subtree is allowed to get before [`Node::rebalance_if_needed`]
/// rebuilds it. A value of `0.25` means: once the smaller side drops below
/// 25% of the subtree's total size, rebuild. Smaller values rebalance less
/// often (cheaper, but allows more skew); larger values keep the tree
/// closer to perfectly balanced at the cost of more rebuilds.
const IMBALANCE_FACTOR: f64 = 0.25;

/// A node in the rope tree: either a chunk of text, or a fork joining two
/// subtrees end-to-end (left's text immediately followed by right's).
enum Node {
    Leaf(String),
    Internal(Internal),
}

/// An internal rope node. `left_chars`/`left_newlines` cache the left
/// subtree's totals so navigation never has to walk it just to find out how
/// big it is.
struct Internal {
    left: Box<Node>,
    right: Box<Node>,
    /// Character count of the entire left subtree.
    left_chars: usize,
    /// Count of `\n` characters in the entire left subtree. Note this is a
    /// count of newline *characters*, not lines: the line that straddles
    /// the left/right boundary (if the left subtree doesn't end in `\n`) is
    /// one logical line split across both children.
    left_newlines: usize,
    /// Character count of this *entire* subtree (left + right). Kept
    /// alongside `left_chars` so [`Node::rebalance_if_needed`] can read
    /// both sides' sizes in O(1) rather than walking the right subtree.
    chars: usize,
}

impl Node {
    /// Builds a (roughly balanced) subtree for `text` by recursively
    /// bisecting until every leaf is at or under [`MAX_LEAF_LEN`].
    fn from_str(text: &str) -> Self {
        let total_chars = text.chars().count();

        if total_chars <= MAX_LEAF_LEN {
            return Node::Leaf(text.to_string());
        }

        let mid = total_chars / 2;
        let byte_mid = char_to_byte(text, mid);
        let (left_text, right_text) = text.split_at(byte_mid);

        Node::Internal(Internal {
            left: Box::new(Node::from_str(left_text)),
            right: Box::new(Node::from_str(right_text)),
            left_chars: left_text.chars().count(),
            left_newlines: left_text.matches('\n').count(),
            chars: total_chars,
        })
    }

    /// After an edit changes this subtree's shape, checks whether it has
    /// become skewed enough to hurt performance and, if so, rebuilds it
    /// into a balanced subtree from scratch.
    ///
    /// This is what keeps [`RopeStorage`] close to its advertised
    /// `O(log n)` behavior under one-sided edit patterns — most notably
    /// sequential typing, which (without this) repeatedly splits the
    /// rightmost leaf without ever folding the resulting nodes back
    /// together, degrading the tree into a linked list along the right
    /// spine. A full rebuild is `O(subtree size)`, but it's only triggered
    /// once a subtree has drifted past [`IMBALANCE_FACTOR`], which — since
    /// a rebuild resets it to perfectly balanced — happens rarely enough
    /// relative to subtree size to keep the amortized cost per edit
    /// logarithmic (the standard scapegoat-tree argument).
    fn rebalance_if_needed(&mut self) {
        let Node::Internal(n) = self else { return };

        let total = n.chars;
        if total <= REBALANCE_MIN_CHARS {
            return;
        }

        let right_chars = total - n.left_chars;
        let smaller = n.left_chars.min(right_chars);

        if (smaller as f64) < IMBALANCE_FACTOR * total as f64 {
            let mut buf = String::with_capacity(total);
            self.flatten(&mut buf);
            *self = Node::from_str(&buf);
        }
    }

    /// Inserts `text` at character offset `at` (relative to this node).
    /// Returns the `(chars_added, newlines_added)` delta, so callers on the
    /// way back up can adjust cached counts without re-walking the subtree
    /// they didn't touch.
    fn insert(&mut self, at: usize, text: &str) -> (usize, usize) {
        match self {
            Node::Leaf(s) => {
                let byte = char_to_byte(s, at);
                s.insert_str(byte, text);

                if s.chars().count() > MAX_LEAF_LEN {
                    let full = std::mem::take(s);
                    *self = Node::from_str(&full);
                }

                (text.chars().count(), text.matches('\n').count())
            }
            Node::Internal(n) => {
                let (dc, dn) = if at <= n.left_chars {
                    let (dc, dn) = n.left.insert(at, text);
                    n.left_chars += dc;
                    n.left_newlines += dn;
                    (dc, dn)
                } else {
                    n.right.insert(at - n.left_chars, text)
                };

                n.chars += dc;

                self.rebalance_if_needed();

                (dc, dn)
            }
        }
    }

    /// Removes the character range `[start, end)` (relative to this node).
    /// Returns the `(chars_removed, newlines_removed)` delta, same purpose
    /// as in [`Self::insert`].
    fn delete(&mut self, start: usize, end: usize) -> (usize, usize) {
        if start >= end {
            return (0, 0);
        }

        match self {
            Node::Leaf(s) => {
                let sb = char_to_byte(s, start);
                let eb = char_to_byte(s, end);
                let removed_newlines = s[sb..eb].matches('\n').count();
                s.replace_range(sb..eb, "");
                (end - start, removed_newlines)
            }
            Node::Internal(n) => {
                let left_chars = n.left_chars;
                let mut removed_chars = 0;
                let mut removed_newlines = 0;

                if start < left_chars {
                    let (dc, dn) = n.left.delete(start, end.min(left_chars));
                    n.left_chars -= dc;
                    n.left_newlines -= dn;
                    removed_chars += dc;
                    removed_newlines += dn;
                }

                if end > left_chars {
                    let right_start = start.saturating_sub(left_chars);
                    let right_end = end - left_chars;
                    let (dc, dn) = n.right.delete(right_start, right_end);
                    removed_chars += dc;
                    removed_newlines += dn;
                }

                n.chars -= removed_chars;

                self.rebalance_if_needed();

                (removed_chars, removed_newlines)
            }
        }
    }

    /// Character offset (relative to this node) where line `line` begins.
    /// The line that straddles a left/right split is addressed from the
    /// left side, since that's where its text starts.
    fn line_to_char(&self, line: usize) -> usize {
        match self {
            Node::Leaf(s) => {
                if line == 0 {
                    return 0;
                }

                let mut seen = 0;
                for (i, ch) in s.char_indices() {
                    if ch == '\n' {
                        seen += 1;
                        if seen == line {
                            return s[..=i].chars().count();
                        }
                    }
                }

                // Defensive fallback; callers only ask for lines that exist.
                s.chars().count()
            }
            Node::Internal(n) => {
                if line <= n.left_newlines {
                    n.left.line_to_char(line)
                } else {
                    n.left_chars + n.right.line_to_char(line - n.left_newlines)
                }
            }
        }
    }

    /// Appends the characters in `[start, end)` (relative to this node) to
    /// `buf`.
    fn append_range(&self, start: usize, end: usize, buf: &mut String) {
        if start >= end {
            return;
        }

        match self {
            Node::Leaf(s) => {
                let sb = char_to_byte(s, start);
                let eb = char_to_byte(s, end);
                buf.push_str(&s[sb..eb]);
            }
            Node::Internal(n) => {
                if start < n.left_chars {
                    n.left.append_range(start, end.min(n.left_chars), buf);
                }
                if end > n.left_chars {
                    n.right.append_range(
                        start.saturating_sub(n.left_chars),
                        end - n.left_chars,
                        buf,
                    );
                }
            }
        }
    }

    /// Appends every leaf's text, in order, to `buf`.
    fn flatten(&self, buf: &mut String) {
        match self {
            Node::Leaf(s) => buf.push_str(s),
            Node::Internal(n) => {
                n.left.flatten(buf);
                n.right.flatten(buf);
            }
        }
    }
}

/// Converts a character `idx` into a byte offset into `s`, the same
/// end-of-string-clamping convention `VecStorage` uses.
fn char_to_byte(s: &str, idx: usize) -> usize {
    s.char_indices().nth(idx).map(|(i, _)| i).unwrap_or(s.len())
}

/// [`TextStorage`] implementation backed by a rope.
///
/// `total_chars`/`total_lines` are maintained incrementally from the deltas
/// [`Node::insert`]/[`Node::delete`] report, so [`TextStorage::len_chars`]
/// and [`TextStorage::line_count`] are `O(1)` rather than requiring a walk
/// of the whole tree.
pub struct RopeStorage {
    root: Node,
    total_chars: usize,
    total_lines: usize,
}

impl RopeStorage {
    /// Creates an empty document: one line containing no text.
    pub fn new() -> Self {
        Self { root: Node::Leaf(String::new()), total_chars: 0, total_lines: 1 }
    }

    /// Checks that `pos` refers to an existing line and a column that is
    /// within (or exactly at the end of) that line's character count.
    fn validate_position(&self, pos: Position) -> Result<()> {
        if pos.line >= self.total_lines {
            return Err(BufferError::InvalidPosition);
        }

        let (start, end) = self.line_range(pos.line);
        if pos.column > end - start {
            return Err(BufferError::InvalidPosition);
        }

        Ok(())
    }

    /// The `[start, end)` character range spanning line `index`'s content,
    /// with any terminating `\n` excluded.
    fn line_range(&self, index: usize) -> (usize, usize) {
        let start = self.root.line_to_char(index);
        let end = if index + 1 < self.total_lines {
            self.root.line_to_char(index + 1) - 1
        } else {
            self.total_chars
        };

        (start, end)
    }

    /// Converts a validated `pos` into a global character offset into the
    /// rope.
    fn char_offset(&self, pos: Position) -> usize {
        self.root.line_to_char(pos.line) + pos.column
    }
}

impl Default for RopeStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl TextStorage for RopeStorage {
    type Line<'a> = String;

    fn line(&self, index: usize) -> Option<Self::Line<'_>> {
        if index >= self.total_lines {
            return None;
        }

        let (start, end) = self.line_range(index);
        let mut buf = String::with_capacity(end - start);
        self.root.append_range(start, end, &mut buf);
        Some(buf)
    }

    fn line_count(&self) -> usize {
        self.total_lines
    }

    fn len_chars(&self) -> usize {
        self.total_chars
    }

    fn to_text(&self) -> String {
        let mut buf = String::with_capacity(self.total_chars);
        self.root.flatten(&mut buf);
        buf
    }

    fn insert(&mut self, pos: Position, text: &str) -> Result<()> {
        self.validate_position(pos)?;

        let offset = self.char_offset(pos);
        let (chars_added, newlines_added) = self.root.insert(offset, text);

        self.total_chars += chars_added;
        self.total_lines += newlines_added;

        Ok(())
    }

    fn delete(&mut self, mut range: Range) -> Result<()> {
        self.validate_position(range.start)?;
        self.validate_position(range.end)?;

        // Callers may hand us a range built from an end-to-start drag, so
        // normalize direction before doing anything else.
        if range.start > range.end {
            std::mem::swap(&mut range.start, &mut range.end);
        }

        let start = self.char_offset(range.start);
        let end = self.char_offset(range.end);
        let (chars_removed, newlines_removed) = self.root.delete(start, end);

        self.total_chars -= chars_removed;
        self.total_lines -= newlines_removed;

        Ok(())
    }
}

#[cfg(test)]
impl Node {
    /// The tree's height (a single leaf has depth 1). Test-only: used to
    /// confirm the tree stays roughly balanced rather than degenerating
    /// under one-sided edit patterns.
    fn depth(&self) -> usize {
        match self {
            Node::Leaf(_) => 1,
            Node::Internal(n) => 1 + n.left.depth().max(n.right.depth()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a `RopeStorage` from lines the same way the `VecStorage`
    /// tests build one, but through the public API rather than a struct
    /// literal (the tree's internal shape isn't something tests should
    /// need to know about).
    fn storage(lines: &[&str]) -> RopeStorage {
        let mut storage = RopeStorage::new();
        let text = lines.join("\n");

        if !text.is_empty() {
            storage.insert(Position::new(0, 0), &text).unwrap();
        }

        storage
    }

    // ===== Constructor =====

    #[test]
    fn new_has_one_empty_line() {
        let storage = RopeStorage::new();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some(String::new()));
        assert_eq!(storage.len_chars(), 0);
    }

    // ===== len_chars =====

    #[test]
    fn len_chars_ascii() {
        let storage = storage(&["abc", "de"]);

        // "abc\nde"
        assert_eq!(storage.len_chars(), 6);
    }

    #[test]
    fn len_chars_unicode() {
        let storage = storage(&["😀😁", "😂"]);

        // 😀😁\n😂
        assert_eq!(storage.len_chars(), 4);
    }

    // ===== to_text =====

    #[test]
    fn to_text_single_line() {
        let storage = storage(&["Hello"]);

        assert_eq!(storage.to_text(), "Hello");
    }

    #[test]
    fn to_text_multiple_lines() {
        let storage = storage(&["one", "two", "three"]);

        assert_eq!(storage.to_text(), "one\ntwo\nthree");
    }

    #[test]
    fn to_text_empty_buffer() {
        let storage = RopeStorage::new();

        assert_eq!(storage.to_text(), "");
    }

    #[test]
    fn to_text_preserves_blank_lines() {
        let storage = storage(&["a", "", "b"]);

        assert_eq!(storage.to_text(), "a\n\nb");
    }

    #[test]
    fn to_text_roundtrips_with_insert() {
        let mut storage = RopeStorage::new();

        let original = "line one\nline two\nline three";

        storage.insert(Position::new(0, 0), original).unwrap();

        assert_eq!(storage.to_text(), original);
    }

    // ===== Insert =====

    #[test]
    fn insert_into_empty() {
        let mut storage = storage(&[""]);

        storage.insert(Position::new(0, 0), "Hello").unwrap();

        assert_eq!(storage.line(0), Some("Hello".to_string()));
    }

    #[test]
    fn insert_at_beginning() {
        let mut storage = storage(&["World"]);

        storage.insert(Position::new(0, 0), "Hello ").unwrap();

        assert_eq!(storage.line(0), Some("Hello World".to_string()));
    }

    #[test]
    fn insert_at_end() {
        let mut storage = storage(&["Hello"]);

        storage.insert(Position::new(0, 5), " World").unwrap();

        assert_eq!(storage.line(0), Some("Hello World".to_string()));
    }

    #[test]
    fn insert_in_middle() {
        let mut storage = storage(&["Helo"]);

        storage.insert(Position::new(0, 2), "l").unwrap();

        assert_eq!(storage.line(0), Some("Hello".to_string()));
    }

    #[test]
    fn insert_empty_string() {
        let mut storage = storage(&["Hello"]);

        storage.insert(Position::new(0, 2), "").unwrap();

        assert_eq!(storage.line(0), Some("Hello".to_string()));
    }

    #[test]
    fn insert_single_newline() {
        let mut storage = storage(&["HelloWorld"]);

        storage.insert(Position::new(0, 5), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some("Hello".to_string()));
        assert_eq!(storage.line(1), Some("World".to_string()));
    }

    #[test]
    fn insert_multiple_newlines() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 1), "\n123\n456\n").unwrap();

        assert_eq!(storage.line_count(), 4);
        assert_eq!(storage.line(0), Some("a".to_string()));
        assert_eq!(storage.line(1), Some("123".to_string()));
        assert_eq!(storage.line(2), Some("456".to_string()));
        assert_eq!(storage.line(3), Some("bc".to_string()));
    }

    #[test]
    fn insert_trailing_newline() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 3), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some("abc".to_string()));
        assert_eq!(storage.line(1), Some(String::new()));
    }

    #[test]
    fn insert_leading_newline() {
        let mut storage = storage(&["abc"]);

        storage.insert(Position::new(0, 0), "\n").unwrap();

        assert_eq!(storage.line_count(), 2);
        assert_eq!(storage.line(0), Some(String::new()));
        assert_eq!(storage.line(1), Some("abc".to_string()));
    }

    #[test]
    fn insert_unicode() {
        let mut storage = storage(&["😀😁"]);

        storage.insert(Position::new(0, 1), "😂").unwrap();

        assert_eq!(storage.line(0), Some("😀😂😁".to_string()));
    }

    // ===== Delete =====

    #[test]
    fn delete_empty_range() {
        let mut storage = storage(&["Hello"]);

        storage
            .delete(Range::new(Position::new(0, 2), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("Hello".to_string()));
    }

    #[test]
    fn delete_single_character() {
        let mut storage = storage(&["Hello"]);

        storage
            .delete(Range::new(Position::new(0, 1), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("Hllo".to_string()));
    }

    #[test]
    fn delete_middle() {
        let mut storage = storage(&["Hello World"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(0, 6)))
            .unwrap();

        assert_eq!(storage.line(0), Some("HelloWorld".to_string()));
    }

    #[test]
    fn delete_newline_between_lines() {
        let mut storage = storage(&["Hello", "World"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(1, 0)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some("HelloWorld".to_string()));
    }

    #[test]
    fn delete_across_lines() {
        let mut storage = storage(&["Hello", "Beautiful", "World"]);

        storage
            .delete(Range::new(Position::new(0, 2), Position::new(2, 3)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some("Held".to_string()));
    }

    #[test]
    fn delete_everything() {
        let mut storage = storage(&["Hello", "World"]);

        storage
            .delete(Range::new(Position::new(0, 0), Position::new(1, 5)))
            .unwrap();

        assert_eq!(storage.line_count(), 1);
        assert_eq!(storage.line(0), Some(String::new()));
    }

    #[test]
    fn delete_unicode() {
        let mut storage = storage(&["😀😁😂"]);

        storage
            .delete(Range::new(Position::new(0, 1), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("😀😂".to_string()));
    }

    #[test]
    fn delete_reversed_range() {
        let mut storage = storage(&["abcdef"]);

        storage
            .delete(Range::new(Position::new(0, 5), Position::new(0, 2)))
            .unwrap();

        assert_eq!(storage.line(0), Some("abf".to_string()));
    }

    // ===== Validation =====

    #[test]
    fn insert_invalid_line() {
        let mut storage = storage(&["abc"]);

        assert!(matches!(
            storage.insert(Position::new(5, 0), "x"),
            Err(BufferError::InvalidPosition)
        ));
    }

    #[test]
    fn insert_invalid_column() {
        let mut storage = storage(&["abc"]);

        assert!(matches!(
            storage.insert(Position::new(0, 10), "x"),
            Err(BufferError::InvalidPosition)
        ));
    }

    #[test]
    fn delete_invalid_position() {
        let mut storage = storage(&["abc"]);

        assert!(matches!(
            storage
                .delete(Range::new(Position::new(0, 0), Position::new(5, 0),)),
            Err(BufferError::InvalidPosition)
        ));
    }

    // ===== Leaf splitting =====
    //
    // These exercise behavior specific to the rope: an insert big enough to
    // push a leaf past `MAX_LEAF_LEN` rebuilds it into a small subtree, and
    // everything above (line lookups, deletes, `to_text`) has to keep
    // working transparently across that boundary.

    #[test]
    fn insert_past_leaf_cap_splits_and_stays_correct() {
        let mut storage = RopeStorage::new();

        // Comfortably over MAX_LEAF_LEN, spread across many short lines so
        // splitting can't help landing on a line boundary by luck.
        let line = "the quick brown fox jumps over the lazy dog";
        let lines: Vec<&str> = std::iter::repeat_n(line, 200).collect();
        let text = lines.join("\n");

        storage.insert(Position::new(0, 0), &text).unwrap();

        assert_eq!(storage.line_count(), 200);
        assert_eq!(storage.line(0), Some(line.to_string()));
        assert_eq!(storage.line(199), Some(line.to_string()));
        assert_eq!(storage.to_text(), text);
        assert_eq!(storage.len_chars(), text.chars().count());
    }

    #[test]
    fn sequential_typing_stays_balanced() {
        // Simulates holding down a key: many single-character inserts,
        // each at the end of the document. Before `Node::rebalance_if_needed`
        // existed, this pattern degenerated the tree into a right-leaning
        // spine that grew one level deeper roughly every `MAX_LEAF_LEN`
        // characters typed — i.e. O(n) depth instead of O(log n).
        let mut storage = RopeStorage::new();

        let n = 50_000;
        for i in 0..n {
            storage.insert(Position::new(0, i), "x").unwrap();
        }

        assert_eq!(storage.len_chars(), n);
        assert_eq!(storage.to_text(), "x".repeat(n));

        // A perfectly balanced tree over `n` leaves of MAX_LEAF_LEN each
        // has depth ~log2(n / MAX_LEAF_LEN). The bound below is deliberately
        // generous (the goal is ruling out linear growth, not chasing a
        // tight constant); the unpatched tree would blow past it by 10x+.
        let leaves = (n / MAX_LEAF_LEN).max(1);
        let bound = (leaves as f64).log2().ceil() as usize * 4 + 10;
        let depth = storage.root.depth();

        assert!(
            depth <= bound,
            "tree depth {depth} exceeds generous bound {bound} after {n} \
             sequential inserts — rebalancing may be broken"
        );
    }

    #[test]
    fn edit_across_split_leaves() {
        let mut storage = RopeStorage::new();

        let text = "x".repeat(3000);
        storage.insert(Position::new(0, 0), &text).unwrap();

        // Insert and delete right around the middle, which by now sits at
        // a leaf boundary rather than inside one contiguous `String`.
        storage.insert(Position::new(0, 1500), "MIDDLE").unwrap();
        assert_eq!(storage.len_chars(), 3006);

        storage
            .delete(Range::new(Position::new(0, 1500), Position::new(0, 1506)))
            .unwrap();

        assert_eq!(storage.to_text(), text);
    }
}
