use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin},
    style::Style,
    text::{Line, Span},
    widgets::Clear,
};

use crate::ui::{self, ACCENT, DIM, SUBTEXT, TEXT, centered_rect};

#[derive(Debug)]
pub struct Confirm {
    msg: String,
}

pub enum ConfirmKind {
    Yes,
    No,
    Stale,
}

impl Confirm {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = centered_rect(30, 6, area);
        frame.render_widget(Clear, popup_area);

        let inner = popup_area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });

        let [msg_area, sep_area, _, confirm_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(inner);

        frame.render_widget(
            Line::styled(self.msg.as_str(), Style::default().fg(TEXT)),
            msg_area,
        );

        frame.render_widget(ui::full_separator(inner.width), sep_area);

        let confirm = Line::from(vec![
            Span::styled("Y", Style::default().fg(ACCENT)),
            Span::styled("es", Style::default().fg(SUBTEXT)),
            Span::styled(" | ", Style::default().fg(DIM)),
            Span::styled("N", Style::default().fg(ACCENT)),
            Span::styled("o", Style::default().fg(SUBTEXT)),
        ]);

        frame.render_widget(confirm, confirm_area);
    }

    pub fn handle_key(&self, key: KeyCode) -> ConfirmKind {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => ConfirmKind::Yes,
            KeyCode::Char('n') | KeyCode::Char('N') => ConfirmKind::No,
            _ => ConfirmKind::Stale,
        }
    }
}
