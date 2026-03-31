use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};
use zeroize::Zeroizing;

use crate::{
    screen::{Screen, account_list::AccountList, vault_list::VaultList},
    ui::{ACCENT, RED, SUBTEXT, TEXT, centered_rect, full_separator, key_hint},
    vault::{Vault, VaultError},
};

#[derive(Debug)]
pub struct PasswordPrompt {
    pub vault_name: String,
    pub input: Zeroizing<String>,
    pub error: Option<&'static str>,
}

impl PasswordPrompt {
    pub fn new(vault_name: String) -> Self {
        Self {
            vault_name,
            input: Zeroizing::new(String::new()),
            error: None,
        }
    }

    pub fn with_error(vault_name: String, error: Option<&'static str>) -> Self {
        Self {
            vault_name,
            input: Zeroizing::new(String::new()),
            error,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let height = if self.error.is_some() { 9 } else { 7 };
        let area = frame.area();
        let popup_area = centered_rect(44, height, area);
        frame.render_widget(Clear, popup_area);

        let inner = popup_area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });

        let [label_area, sep_area, field_area, hint_area, _, error_area] = Layout::vertical([
            Constraint::Length(1), // "unlock <vault>"
            Constraint::Length(1), // separator
            Constraint::Length(1), // masked input
            Constraint::Length(1), // hint
            Constraint::Length(1), // spacer
            Constraint::Length(if self.error.is_some() { 1 } else { 0 }),
        ])
        .areas(inner);

        frame.render_widget(
            Line::from(vec![
                Span::styled("unlock ", Style::default().fg(SUBTEXT)),
                Span::styled(self.vault_name.clone(), Style::default().fg(TEXT).bold()),
            ]),
            label_area,
        );

        frame.render_widget(full_separator(popup_area.width), sep_area);

        let masked = format!("{}▌", "•".repeat(self.input.len()));
        frame.render_widget(
            Paragraph::new(masked).style(Style::default().fg(ACCENT)),
            field_area,
        );

        let mut hints: Vec<Span> = vec![];
        hints.extend(key_hint("↵", "unlock"));
        hints.extend(key_hint("esc", "cancel"));
        frame.render_widget(Line::from(hints), hint_area);

        if let Some(msg) = self.error {
            frame.render_widget(Line::styled(msg, Style::default().fg(RED)), error_area);
        }
    }

    pub fn handle_key(mut self, key: KeyCode) -> Screen {
        match key {
            KeyCode::Esc => Screen::VaultList(VaultList::new()),
            KeyCode::Backspace => {
                self.input.pop();
                Screen::PasswordPrompt(self)
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                Screen::PasswordPrompt(self)
            }
            KeyCode::Enter => match Vault::new(&self.vault_name).load(self.input.as_bytes()) {
                Ok(entries) => Screen::AccountList(AccountList::new(self.vault_name, entries)),
                Err(VaultError::WrongPassword) => {
                    Screen::password_error(self.vault_name, "wrong password")
                }
                Err(_) => Screen::password_error(self.vault_name, "Failed to open vault"),
            },
            _ => Screen::PasswordPrompt(PasswordPrompt::new(self.vault_name)),
        }
    }
}
