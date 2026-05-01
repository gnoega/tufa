use crossterm::event::KeyCode;
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::{ACCENT, TEXT};

#[derive(Debug)]
pub struct Search {
    pub query: String,
    cursor: usize,
}

pub enum SearchState {
    Active,
    Dismiss,
    Confirm,
}

impl Search {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
        }
    }

    pub fn draw(&self) -> Paragraph<'_> {
        let before: String = self.query.chars().take(self.cursor).collect();
        let under: String = self
            .query
            .chars()
            .nth(self.cursor)
            .map(|c| c.to_string())
            .unwrap_or(" ".to_string());
        let after: String = self.query.chars().skip(self.cursor + 1).collect();

        let line = Line::from(vec![
            Span::styled("filter: ", Style::default().fg(ACCENT)),
            Span::styled(before, Style::default().fg(TEXT)),
            Span::styled(under, Style::default().fg(TEXT).reversed()),
            Span::styled(after, Style::default().fg(TEXT)),
        ]);

        Paragraph::new(line)
    }

    fn insert(&mut self, c: char) {
        let byte_idx = self.char_to_byte(self.cursor);
        self.query.insert(byte_idx, c);
        self.cursor += 1;
    }

    fn delete_before(&mut self) {
        if self.cursor > 0 {
            let byte_idx = self.char_to_byte(self.cursor - 1);
            self.query.remove(byte_idx);
            self.cursor -= 1;
        }
    }

    fn delete_after(&mut self) {
        if self.cursor < self.char_count() {
            let byte_idx = self.char_to_byte(self.cursor);
            self.query.remove(byte_idx);
        }
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.char_count());
    }

    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.query
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.query.len())
    }

    fn char_count(&self) -> usize {
        self.query.chars().count()
    }

    pub fn handle_key(&mut self, key: KeyCode) -> SearchState {
        match key {
            KeyCode::Esc => SearchState::Dismiss,
            KeyCode::Left => {
                self.move_left();
                SearchState::Active
            }
            KeyCode::Right => {
                self.move_right();
                SearchState::Active
            }
            KeyCode::Char(c) => {
                self.insert(c);
                SearchState::Active
            }
            KeyCode::Backspace => {
                self.delete_before();
                SearchState::Active
            }
            KeyCode::Delete => {
                self.delete_after();
                SearchState::Active
            }
            KeyCode::Enter => SearchState::Confirm,
            _ => SearchState::Active,
        }
    }
}
