use ropey::Rope;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BufferError {
    #[error("line {0} is out of bounds (buffer has {1} lines)")]
    LineOutOfBounds(usize, usize),

    #[error("column {0} is out of bounds for line {1} (line has {2} chars)")]
    ColumnOutOfBounds(usize, usize, usize),

    #[error("delete range is inverted: start {0:?} is after end {1:?}")]
    InvertedRange(Position, Position),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone)]
enum EditOp {
    Insert { at: usize, text: String },
    Delete { at: usize, text: String },
}

pub struct TextBuffer {
    rope: Rope,
    undo_stack: Vec<EditOp>,
    redo_stack: Vec<EditOp>,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self::from_str("")
    }

    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    fn position_to_char_idx(&self, pos: Position) -> Result<usize, BufferError> {
        let line_count = self.rope.len_lines();
        if pos.line >= line_count {
            return Err(BufferError::LineOutOfBounds(pos.line, line_count));
        }

        let line_char_start = self.rope.line_to_char(pos.line);
        let line_len_chars = self.rope.line(pos.line).len_chars();

        if pos.column > line_len_chars {
            return Err(BufferError::ColumnOutOfBounds(
                pos.column,
                pos.line,
                line_len_chars,
            ));
        }

        Ok(line_char_start + pos.column)
    }

    pub fn insert(&mut self, pos: Position, text: &str) -> Result<(), BufferError> {
        if text.is_empty() {
            return Ok(());
        }

        let idx = self.position_to_char_idx(pos)?;
        self.rope.insert(idx, text);

        self.undo_stack.push(EditOp::Insert {
            at: idx,
            text: text.to_string(),
        });
        self.redo_stack.clear();

        Ok(())
    }

    pub fn delete(&mut self, start: Position, end: Position) -> Result<(), BufferError> {
        if start > end {
            return Err(BufferError::InvertedRange(start, end));
        }

        let start_idx = self.position_to_char_idx(start)?;
        let end_idx = self.position_to_char_idx(end)?;

        if start_idx == end_idx {
            return Ok(());
        }

        let removed = self.rope.slice(start_idx..end_idx).to_string();
        self.rope.remove(start_idx..end_idx);

        self.undo_stack.push(EditOp::Delete {
            at: start_idx,
            text: removed,
        });
        self.redo_stack.clear();

        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(op) = self.undo_stack.pop() else {
            return false;
        };

        match &op {
            EditOp::Insert { at, text } => {
                let end = at + text.chars().count();
                self.rope.remove(*at..end);
            }
            EditOp::Delete { at, text } => {
                self.rope.insert(*at, text);
            }
        }

        self.redo_stack.push(op);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(op) = self.redo_stack.pop() else {
            return false;
        };

        match &op {
            EditOp::Insert { at, text } => {
                self.rope.insert(*at, text);
            }
            EditOp::Delete { at, text } => {
                let end = at + text.chars().count();
                self.rope.remove(*at..end);
            }
        }

        self.undo_stack.push(op);
        true
    }

    pub fn line(&self, idx: usize) -> Option<String> {
        if idx >= self.rope.len_lines() {
            return None;
        }
        Some(self.rope.line(idx).to_string())
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn char_len(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn to_string_full(&self) -> String {
        self.rope.to_string()
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_into_empty_buffer() {
        let mut buffer = TextBuffer::new();
        buffer.insert(Position::new(0, 0), "hello").unwrap();
        assert_eq!(buffer.to_string_full(), "hello");
    }

    #[test]
    fn insert_in_middle_of_line() {
        let mut buffer = TextBuffer::from_str("held");
        buffer.insert(Position::new(0, 2), "ll").unwrap();
        assert_eq!(buffer.to_string_full(), "hellld");
    }

    #[test]
    fn insert_on_second_line() {
        let mut buffer = TextBuffer::from_str("first\nsecond\n");
        buffer.insert(Position::new(1, 6), "!").unwrap();
        assert_eq!(buffer.to_string_full(), "first\nsecond!\n");
    }

    #[test]
    fn delete_range_within_line() {
        let mut buffer = TextBuffer::from_str("hello world");
        buffer
            .delete(Position::new(0, 5), Position::new(0, 11))
            .unwrap();
        assert_eq!(buffer.to_string_full(), "hello");
    }

    #[test]
    fn delete_across_lines() {
        let mut buffer = TextBuffer::from_str("first\nsecond\nthird\n");
        buffer
            .delete(Position::new(0, 5), Position::new(2, 0))
            .unwrap();
        assert_eq!(buffer.to_string_full(), "firstthird\n");
    }

    #[test]
    fn undo_reverses_insert() {
        let mut buffer = TextBuffer::from_str("hello");
        buffer.insert(Position::new(0, 5), " world").unwrap();
        assert_eq!(buffer.to_string_full(), "hello world");

        assert!(buffer.undo());
        assert_eq!(buffer.to_string_full(), "hello");
    }

    #[test]
    fn undo_reverses_delete() {
        let mut buffer = TextBuffer::from_str("hello world");
        buffer
            .delete(Position::new(0, 5), Position::new(0, 11))
            .unwrap();
        assert_eq!(buffer.to_string_full(), "hello");

        assert!(buffer.undo());
        assert_eq!(buffer.to_string_full(), "hello world");
    }

    #[test]
    fn redo_reapplies_undone_edit() {
        let mut buffer = TextBuffer::from_str("hello");
        buffer.insert(Position::new(0, 5), "!").unwrap();
        buffer.undo();
        assert_eq!(buffer.to_string_full(), "hello");

        assert!(buffer.redo());
        assert_eq!(buffer.to_string_full(), "hello!");
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut buffer = TextBuffer::from_str("hello");
        buffer.insert(Position::new(0, 5), "!").unwrap();
        buffer.undo();

        buffer.insert(Position::new(0, 5), "?").unwrap();
        assert_eq!(buffer.to_string_full(), "hello?");
        assert!(!buffer.redo());
        assert_eq!(buffer.to_string_full(), "hello?");
    }

    #[test]
    fn multiple_undo_redo_round_trip() {
        let mut buffer = TextBuffer::new();
        buffer.insert(Position::new(0, 0), "a").unwrap();
        buffer.insert(Position::new(0, 1), "b").unwrap();
        buffer.insert(Position::new(0, 2), "c").unwrap();
        assert_eq!(buffer.to_string_full(), "abc");

        assert!(buffer.undo());
        assert!(buffer.undo());
        assert_eq!(buffer.to_string_full(), "a");

        assert!(buffer.redo());
        assert!(buffer.redo());
        assert_eq!(buffer.to_string_full(), "abc");
    }

    #[test]
    fn out_of_bounds_line_returns_error() {
        let mut buffer = TextBuffer::from_str("only one line");
        let result = buffer.insert(Position::new(5, 0), "x");
        assert!(matches!(result, Err(BufferError::LineOutOfBounds(5, 1))));
    }

    #[test]
    fn out_of_bounds_column_returns_error() {
        let mut buffer = TextBuffer::from_str("short");
        let result = buffer.insert(Position::new(0, 100), "x");
        assert!(matches!(
            result,
            Err(BufferError::ColumnOutOfBounds(100, 0, 5))
        ));
    }

    #[test]
    fn inverted_delete_range_returns_error() {
        let mut buffer = TextBuffer::from_str("hello world");
        let result = buffer.delete(Position::new(0, 5), Position::new(0, 0));
        assert!(matches!(result, Err(BufferError::InvertedRange(_, _))));
    }

    #[test]
    fn line_and_line_count() {
        let buffer = TextBuffer::from_str("first\nsecond\nthird");
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line(0), Some("first\n".to_string()));
        assert_eq!(buffer.line(2), Some("third".to_string()));
        assert_eq!(buffer.line(3), None);
    }
}
