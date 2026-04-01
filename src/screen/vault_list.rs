use std::path::Path;

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin},
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};

use crate::{
    screen::{
        Screen,
        password::{self, PasswordState},
    },
    ui::{ACCENT, TEXT, full_separator, key_hint},
    vault::Vault,
};
use crate::{
    screen::{account_list::AccountList, password::PasswordPrompt},
    ui::render_version,
};

#[derive(Debug, Default)]
pub struct VaultList {
    vaults: Vec<String>,
    state: ListState,
    popup: Option<PasswordPrompt>,
}

impl VaultList {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        let mut raw = Vault::list_all();
        raw.sort();
        let vaults = raw
            .iter()
            .map(|v| {
                Path::new(v)
                    .file_stem()
                    .and_then(|f| f.to_str())
                    .unwrap_or(v)
                    .to_string()
            })
            .collect();

        Self {
            vaults,
            state,
            ..Default::default()
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });

        let [title_area, sep_area, list_area, _, hint_area] = Layout::vertical([
            Constraint::Length(1), // title
            Constraint::Length(1), // separator
            Constraint::Fill(1),   // list
            Constraint::Length(1), // spacer
            Constraint::Length(1), // hints
        ])
        .areas(inner);

        frame.render_widget(
            Line::styled("  vaults", Style::default().fg(TEXT).bold()),
            title_area,
        );

        frame.render_widget(full_separator(area.width), sep_area);

        let items: Vec<ListItem> = self
            .vaults
            .iter()
            .map(|name| {
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(name, Style::default().fg(TEXT)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .highlight_symbol("▸ ")
            .highlight_style(Style::default().fg(ACCENT).bold());

        frame.render_stateful_widget(list, list_area, &mut self.state);

        let [left_hint, version] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Min(1)]).areas(hint_area);

        let mut hints: Vec<Span> = vec![];
        hints.extend(key_hint("↵", "open"));
        hints.extend(key_hint("↑↓ / jk", "navigate"));
        hints.extend(key_hint("q", "quit"));
        frame.render_widget(Line::from(hints), left_hint);

        render_version(frame, version);

        if let Some(popup) = &self.popup {
            popup.render(frame);
        }
    }

    pub fn handle_key(mut self, key: KeyCode) -> Screen {
        if let Some(popup) = self.popup.take() {
            return match popup.handle_key(key) {
                PasswordState::Active(p) => {
                    self.popup = Some(p);
                    Screen::VaultList(self)
                }
                PasswordState::Cancelled => {
                    self.popup = None;
                    Screen::VaultList(self)
                }
                PasswordState::Unlocked(vault_name, pw, entries) => {
                    Screen::AccountList(AccountList::new(vault_name, entries).with_password(pw))
                }
                PasswordState::Error(name, msg) => {
                    self.popup = Some(PasswordPrompt::with_error(name, Some(msg)));
                    Screen::VaultList(self)
                }
            };
        }

        match key {
            KeyCode::Char('q') => Screen::Exit,
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.state.selected().unwrap_or(0);
                self.state
                    .select(Some((i + 1).min(self.vaults.len().saturating_sub(1))));
                Screen::VaultList(self)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.state.selected().unwrap_or(0);
                self.state.select(Some(i.saturating_sub(1)));
                Screen::VaultList(self)
            }
            KeyCode::Enter => {
                if let Some(idx) = self.state.selected()
                    && let Some(name) = self.vaults.get(idx).cloned()
                {
                    self.popup = Some(password::PasswordPrompt::new(name))
                }
                Screen::VaultList(self)
            }
            _ => Screen::VaultList(self),
        }
    }
}
