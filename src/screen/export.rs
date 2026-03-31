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
    screen::{Screen, account_list::AccountList},
    totp::TotpEntry,
    ui::{DIM, GREEN, SUBTEXT, TEXT, centered_rect, key_hint},
};

#[derive(Debug)]
pub struct ExportTotp {
    pub parent: AccountList,

    pub entry: TotpEntry,
    pub copied: Option<String>,
}

impl ExportTotp {
    pub fn new(parent: AccountList, entry: TotpEntry) -> Self {
        Self {
            parent,
            entry,
            copied: None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        self.parent.render(frame);
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

        let hint_line = match &self.copied {
            Some(name) => Line::from(vec![
                Span::styled("✓ ", Style::default().fg(GREEN).bold()),
                Span::styled("uri ", Style::default().fg(TEXT)),
                Span::styled(name.clone(), Style::default().fg(TEXT)),
                Span::styled(" copied to clipboard", Style::default().fg(SUBTEXT)),
            ]),
            None => {
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
    pub fn handle_key(mut self, key: KeyCode) -> Screen {
        match key {
            KeyCode::Esc => Screen::AccountList(self.parent),
            KeyCode::Char('y') => {
                if clipboard::copy_to_clipboard(self.entry.to_uri().as_str()) {
                    self.copied = Some(self.entry.display_name())
                }

                Screen::ExportPopUp(self)
            }
            _ => Screen::ExportPopUp(self),
        }
    }
}
