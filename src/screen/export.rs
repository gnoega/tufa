use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
};

use crate::{
    clipboard,
    notification::Notification,
    totp::TotpEntry,
    ui::{DIM, SUBTEXT, TEXT, centered_rect, key_hint},
};

#[derive(Debug, Clone)]
pub struct ExportTotp {
    entry: TotpEntry,
    bottom_bar: BottomBarKind,
}

#[derive(Debug, Clone)]
pub enum BottomBarKind {
    Hint,
    Notification(Notification),
}

pub enum ExportState {
    Active(ExportTotp),
    Closed,
}

impl ExportTotp {
    pub fn new(entry: TotpEntry) -> Self {
        Self {
            entry,
            bottom_bar: BottomBarKind::Hint,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let _ = self.render_popup(frame);
    }

    fn render_popup(&self, frame: &mut Frame) -> Result<(), Box<dyn std::error::Error>> {
        let area = frame.area();
        let entry_uri = self.entry.to_uri();
        let uri_str = entry_uri.to_string();
        let (qrcode_string, _qr_width, qr_height) = entry_uri.to_qrcode_rendered()?;

        let popup_width = (area.width as u32 * 80 / 100) as u16;
        let inner_width = popup_width.saturating_sub(2).max(1);
        let uri_lines = (uri_str.len() as u16).div_ceil(inner_width).max(1);

        let ideal_height = 2 + 1 + 1 + qr_height + 1 + uri_lines;
        let popup_height = ideal_height.min(area.height.saturating_sub(2));

        let popup_area = centered_rect(80, popup_height, area);

        let hint_line = match &self.bottom_bar {
            BottomBarKind::Notification(notification) => notification.draw(),
            BottomBarKind::Hint => {
                let mut hints: Vec<Span> = vec![];
                hints.extend(key_hint("y", "yank uri"));
                hints.extend(key_hint("esc", "back"));
                Line::from(hints)
            }
        };

        let border = Block::bordered()
            .border_style(Style::default().fg(DIM))
            .title(Line::styled(" export ", Style::default().fg(TEXT).bold()).centered())
            .title_bottom(hint_line);

        frame.render_widget(Clear, popup_area);
        frame.render_widget(&border, popup_area);

        let inner = border.inner(popup_area);

        let [name_area, _, qr_area, _, uri_area] = Layout::vertical([
            Constraint::Length(1),      // account name
            Constraint::Length(1),      // spacer
            Constraint::Min(qr_height), // qr code
            Constraint::Length(1),      // spacer
            Constraint::Min(uri_lines), // uri
        ])
        .areas(inner);

        frame.render_widget(
            Paragraph::new(self.entry.display_name()).style(Style::default().fg(TEXT)),
            name_area,
        );
        frame.render_widget(Paragraph::new(qrcode_string), qr_area);
        frame.render_widget(
            Paragraph::new(uri_str)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(SUBTEXT)),
            uri_area,
        );

        Ok(())
    }
    pub fn handle_key(mut self, key: KeyCode) -> ExportState {
        match key {
            KeyCode::Esc => ExportState::Closed,
            KeyCode::Char('y') => {
                if clipboard::copy_to_clipboard(self.entry.to_uri().as_str()) {
                    self.bottom_bar = BottomBarKind::Notification(Notification::success(
                        "uri copied to the clipboard",
                    ))
                }

                ExportState::Active(self)
            }
            _ => ExportState::Active(self),
        }
    }

    pub fn cleanup(&mut self) {
        if let BottomBarKind::Notification(n) = &self.bottom_bar
            && n.is_expired()
        {
            self.bottom_bar = BottomBarKind::Hint
        }
    }
}
