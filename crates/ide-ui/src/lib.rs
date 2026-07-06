use ide_buffer::{Position, TextBuffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    Delete,
    Enter,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub key: Key,
}

impl KeyEvent {
    pub fn new(key: Key) -> Self {
        Self { key }
    }
}

pub struct EditorWidget {
    buffer: TextBuffer,
    cursor: Position,
}

impl EditorWidget {
    pub fn new(buffer: TextBuffer) -> Self {
        Self {
            buffer,
            cursor: Position::new(0, 0),
        }
    }

    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn cursor(&self) -> Position {
        self.cursor
    }

    fn line_len(&self, line: usize) -> usize {
        self.buffer
            .line(line)
            .map(|l| l.trim_end_matches(['\n', '\r']).chars().count())
            .unwrap_or(0)
    }

    pub fn handle_key_event(&mut self, event: KeyEvent) {
        match event.key {
            Key::Char(c) => self.insert_char(c),
            Key::Enter => self.insert_char('\n'),
            Key::Backspace => self.backspace(),
            Key::Delete => self.delete_forward(),
            Key::ArrowLeft => self.move_left(),
            Key::ArrowRight => self.move_right(),
            Key::ArrowUp => self.move_up(),
            Key::ArrowDown => self.move_down(),
            Key::Home => self.cursor.column = 0,
            Key::End => self.cursor.column = self.line_len(self.cursor.line),
        }
    }

    fn insert_char(&mut self, c: char) {
        let mut encode_buf = [0u8; 4];
        let s = c.encode_utf8(&mut encode_buf);

        if self.buffer.insert(self.cursor, s).is_err() {
            return;
        }

        if c == '\n' {
            self.cursor = Position::new(self.cursor.line + 1, 0);
        } else {
            self.cursor = Position::new(self.cursor.line, self.cursor.column + 1);
        }
    }

    fn backspace(&mut self) {
        if self.cursor.column > 0 {
            let start = Position::new(self.cursor.line, self.cursor.column - 1);
            if self.buffer.delete(start, self.cursor).is_ok() {
                self.cursor = start;
            }
        } else if self.cursor.line > 0 {
            let prev_line_len = self.line_len(self.cursor.line - 1);
            let start = Position::new(self.cursor.line - 1, prev_line_len);
            if self.buffer.delete(start, self.cursor).is_ok() {
                self.cursor = start;
            }
        }
    }

    fn delete_forward(&mut self) {
        let current_line_len = self.line_len(self.cursor.line);

        if self.cursor.column < current_line_len {
            let end = Position::new(self.cursor.line, self.cursor.column + 1);
            let _ = self.buffer.delete(self.cursor, end);
        } else if self.cursor.line + 1 < self.buffer.line_count() {
            let end = Position::new(self.cursor.line + 1, 0);
            let _ = self.buffer.delete(self.cursor, end);
        }
    }

    fn move_left(&mut self) {
        if self.cursor.column > 0 {
            self.cursor.column -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.column = self.line_len(self.cursor.line);
        }
    }

    fn move_right(&mut self) {
        let len = self.line_len(self.cursor.line);
        if self.cursor.column < len {
            self.cursor.column += 1;
        } else if self.cursor.line + 1 < self.buffer.line_count() {
            self.cursor.line += 1;
            self.cursor.column = 0;
        }
    }

    fn move_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            let len = self.line_len(self.cursor.line);
            self.cursor.column = self.cursor.column.min(len);
        }
    }

    fn move_down(&mut self) {
        if self.cursor.line + 1 < self.buffer.line_count() {
            self.cursor.line += 1;
            let len = self.line_len(self.cursor.line);
            self.cursor.column = self.cursor.column.min(len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(editor: &mut EditorWidget, key: Key) {
        editor.handle_key_event(KeyEvent::new(key));
    }

    #[test]
    fn typing_characters_inserts_and_advances_cursor() {
        let mut editor = EditorWidget::new(TextBuffer::new());
        press(&mut editor, Key::Char('h'));
        press(&mut editor, Key::Char('i'));

        assert_eq!(editor.buffer().to_string_full(), "hi");
        assert_eq!(editor.cursor(), Position::new(0, 2));
    }

    #[test]
    fn enter_inserts_newline_and_moves_to_next_line_start() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("ab"));
        editor.cursor = Position::new(0, 2);
        press(&mut editor, Key::Enter);

        assert_eq!(editor.buffer().to_string_full(), "ab\n");
        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn backspace_within_line_deletes_previous_char() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("hello"));
        editor.cursor = Position::new(0, 5);
        press(&mut editor, Key::Backspace);

        assert_eq!(editor.buffer().to_string_full(), "hell");
        assert_eq!(editor.cursor(), Position::new(0, 4));
    }

    #[test]
    fn backspace_at_line_start_merges_with_previous_line() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("first\nsecond"));
        editor.cursor = Position::new(1, 0);
        press(&mut editor, Key::Backspace);

        assert_eq!(editor.buffer().to_string_full(), "firstsecond");
        assert_eq!(editor.cursor(), Position::new(0, 5));
    }

    #[test]
    fn delete_forward_within_line_removes_next_char() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("hello"));
        editor.cursor = Position::new(0, 0);
        press(&mut editor, Key::Delete);

        assert_eq!(editor.buffer().to_string_full(), "ello");
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn delete_forward_at_line_end_merges_next_line() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("first\nsecond"));
        editor.cursor = Position::new(0, 5);
        press(&mut editor, Key::Delete);

        assert_eq!(editor.buffer().to_string_full(), "firstsecond");
        assert_eq!(editor.cursor(), Position::new(0, 5));
    }

    #[test]
    fn arrow_left_and_right_move_within_line() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("abc"));
        editor.cursor = Position::new(0, 1);

        press(&mut editor, Key::ArrowRight);
        assert_eq!(editor.cursor(), Position::new(0, 2));

        press(&mut editor, Key::ArrowLeft);
        press(&mut editor, Key::ArrowLeft);
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn arrow_left_at_line_start_wraps_to_previous_line_end() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("first\nsecond"));
        editor.cursor = Position::new(1, 0);
        press(&mut editor, Key::ArrowLeft);

        assert_eq!(editor.cursor(), Position::new(0, 5));
    }

    #[test]
    fn arrow_right_at_line_end_wraps_to_next_line_start() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("first\nsecond"));
        editor.cursor = Position::new(0, 5);
        press(&mut editor, Key::ArrowRight);

        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn arrow_up_and_down_clamp_column_to_shorter_line() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("longline\nhi"));
        editor.cursor = Position::new(0, 8);

        press(&mut editor, Key::ArrowDown);
        assert_eq!(editor.cursor(), Position::new(1, 2));

        press(&mut editor, Key::ArrowUp);
        assert_eq!(editor.cursor(), Position::new(0, 2));
    }

    #[test]
    fn home_and_end_move_to_line_boundaries() {
        let mut editor = EditorWidget::new(TextBuffer::from_str("hello"));
        editor.cursor = Position::new(0, 3);

        press(&mut editor, Key::Home);
        assert_eq!(editor.cursor(), Position::new(0, 0));

        press(&mut editor, Key::End);
        assert_eq!(editor.cursor(), Position::new(0, 5));
    }
}
