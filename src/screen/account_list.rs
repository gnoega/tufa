use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Cell, Row, Table, TableState},
};
use zeroize::Zeroizing;

use crate::{
    clipboard,
    screen::{
        Screen,
        confirm::{Confirm, ConfirmKind},
        export::{ExportState, ExportTotp},
        notification::Notification,
        vault_list::VaultList,
    },
    totp::{TotpEntry, totp_ttl},
    ui::{ACCENT, DIM, TEXT, full_separator, key_hint, render_version, ttl_color},
    vault::Vault,
};

#[derive(Debug, Default)]
pub struct AccountList {
    vault_name: String,
    password: Zeroizing<String>,
    entries: Vec<TotpEntry>,
    state: TableState,
    bottom_bar: BottomBarKind,
    popup: Option<PopUpKind>,
}

#[derive(Debug, Default)]
enum BottomBarKind {
    #[default]
    Hint,
    Notification(Notification),
    Search(String),
}

#[derive(Debug)]
enum PopUpKind {
    ExportTotp(ExportTotp),
    ConfirmDelete { confirm: Confirm, index: usize },
}

impl AccountList {
    pub fn new(vault_name: impl Into<String>, entries: Vec<TotpEntry>) -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        Self {
            vault_name: vault_name.into(),
            entries,
            state,
            ..Default::default()
        }
    }

    pub fn with_password(mut self, password: Zeroizing<String>) -> Self {
        self.password = password;
        self
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });

        let [title_area, sep_area, table_area, gauge_area, _, bottom_area] = Layout::vertical([
            Constraint::Length(1), // vault name
            Constraint::Length(1), // separator
            Constraint::Fill(1),   // table
            Constraint::Length(1), // gauge
            Constraint::Length(1), // spacer
            Constraint::Length(1), // bottom_area
        ])
        .areas(inner);

        let ttl = totp_ttl();
        let tc = ttl_color(ttl);

        frame.render_widget(
            Line::from(vec![
                Span::raw("  "),
                Span::styled(self.vault_name.clone(), Style::default().fg(TEXT).bold()),
            ]),
            title_area,
        );

        frame.render_widget(full_separator(area.width), sep_area);

        let col_constraints = [
            Constraint::Fill(1),   // name
            Constraint::Length(7), // code
            Constraint::Length(4), // ttl
        ];

        let header = Row::new(vec![
            Cell::from("  NAME").style(Style::default().fg(DIM).bold()),
            Cell::from("CODE").style(Style::default().fg(DIM).bold()),
            Cell::from("TTL").style(Style::default().fg(DIM).bold()),
        ]);

        let rows: Vec<Row> = if self.entries.is_empty() {
            vec![Row::new(vec![
                Cell::from("  no accounts").style(Style::default().fg(DIM)),
            ])]
        } else {
            self.entries
                .iter()
                .map(|e| {
                    let code = e.generate_otp().unwrap_or_else(|_| "error".into());
                    Row::new(vec![
                        Cell::from(format!("  {}", e.display_name()))
                            .style(Style::default().fg(TEXT)),
                        Cell::from(code).style(Style::default().fg(tc).bold()),
                        Cell::from(format!("{ttl}s")).style(Style::default().fg(DIM)),
                    ])
                })
                .collect()
        };

        let table = Table::new(rows, col_constraints)
            .header(header.bottom_margin(1))
            .column_spacing(2)
            .highlight_symbol("▸ ")
            .row_highlight_style(Style::default().fg(ACCENT).bold());

        frame.render_stateful_widget(table, table_area, &mut self.state);

        let filled = ((ttl as f64 / 30.0) * gauge_area.width as f64) as usize;
        let empty = gauge_area.width as usize - filled;
        let bar = Line::from(vec![
            Span::styled("█".repeat(filled), Style::default().fg(tc)),
            Span::styled(
                "░".repeat(empty),
                Style::default().fg(Color::Rgb(40, 40, 40)),
            ),
            Span::styled(format!("  {ttl}s"), Style::default().fg(DIM)),
        ]);
        frame.render_widget(bar, gauge_area);

        match &self.bottom_bar {
            BottomBarKind::Hint => {
                let [hint, version] = Layout::horizontal([Constraint::Fill(1), Constraint::Max(8)])
                    .areas(bottom_area);
                let mut hints: Vec<Span> = vec![];
                hints.extend(key_hint("↵", "copy"));
                hints.extend(key_hint("↑↓ / jk", "navigate"));
                hints.extend(key_hint("e", "export"));
                hints.extend(key_hint("d", "delete"));
                hints.extend(key_hint("esc", "back"));
                hints.extend(key_hint("q", "quit"));
                hints.extend(key_hint("/", "search"));
                let hint_line = Line::from(hints);

                frame.render_widget(hint_line, hint);
                render_version(frame, version);
            }
            BottomBarKind::Notification(n) => {
                frame.render_widget(n.draw(), bottom_area);
            }
            BottomBarKind::Search(query) => {
                let input = format!("{}▌", query);
                frame.render_widget(
                    Paragraph::new(input).style(Style::default().fg(TEXT)),
                    bottom_area,
                );
            }
        };

        if let Some(kind) = &self.popup {
            match kind {
                PopUpKind::ExportTotp(popup) => popup.render(frame),
                PopUpKind::ConfirmDelete { confirm, .. } => confirm.render(frame),
            }
        }
    }

    pub fn handle_key(mut self, key: KeyCode) -> Screen {
        if let BottomBarKind::Search(ref mut query) = self.bottom_bar {
            match key {
                KeyCode::Char(c) => query.push(c),
                KeyCode::Backspace => {
                    query.pop();
                }
                KeyCode::Esc => self.bottom_bar = BottomBarKind::Hint,
                KeyCode::Enter => {}
                _ => {}
            }
            return Screen::AccountList(self);
        }

        if let Some(kind) = self.popup.take() {
            match kind {
                PopUpKind::ExportTotp(popup) => {
                    return match popup.handle_key(key) {
                        ExportState::Active(export_totp) => {
                            self.popup = Some(PopUpKind::ExportTotp(export_totp));
                            Screen::AccountList(self)
                        }
                        ExportState::Closed => Screen::AccountList(self),
                    };
                }
                PopUpKind::ConfirmDelete { confirm, index } => {
                    return match confirm.handle_key(key) {
                        ConfirmKind::Yes => {
                            self.entries.remove(index);
                            match Vault::new(&self.vault_name)
                                .save(&self.entries, self.password.as_bytes())
                            {
                                Ok(_) => self.notify(Notification::info("item deleted")),
                                Err(e) => self.notify(Notification::danger(e.to_string())),
                            }

                            Screen::AccountList(self)
                        }
                        ConfirmKind::No | ConfirmKind::Stale => Screen::AccountList(self),
                    };
                }
            };
        }

        match key {
            KeyCode::Char('q') => Screen::Exit,
            KeyCode::Esc => Screen::VaultList(VaultList::new()),
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.state.selected().unwrap_or(0);
                self.state
                    .select(Some((i + 1).min(self.entries.len().saturating_sub(1))));
                Screen::AccountList(self)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.state.selected().unwrap_or(0);
                self.state.select(Some(i.saturating_sub(1)));
                Screen::AccountList(self)
            }
            KeyCode::Char('e') => {
                let entry = self.get_selected();
                match entry {
                    Some(entry) => {
                        self.popup = Some(PopUpKind::ExportTotp(ExportTotp::new(entry)));
                        Screen::AccountList(self)
                    }
                    None => Screen::AccountList(self),
                }
            }
            KeyCode::Char('d') => {
                if let Some(i) = self.state.selected()
                    && let Some(entry) = self.entries.get(i)
                {
                    self.popup = Some(PopUpKind::ConfirmDelete {
                        confirm: Confirm::new(format!("delete {}?", entry.display_name())),
                        index: i,
                    });
                }
                Screen::AccountList(self)
            }
            KeyCode::Char('/') => {
                self.bottom_bar = BottomBarKind::Search(String::default());
                Screen::AccountList(self)
            }
            KeyCode::Enter => {
                if let Some(i) = self.state.selected()
                    && let Some(entry) = self.entries.get(i)
                    && let Ok(code) = entry.generate_otp()
                    && clipboard::copy_to_clipboard(&code)
                {
                    self.notify(Notification::success(format!(
                        "{} copied to the clipboard",
                        entry.display_name(),
                    )));
                }

                Screen::AccountList(self)
            }
            _ => Screen::AccountList(self),
        }
    }
    pub fn get_selected(&self) -> Option<TotpEntry> {
        self.entries.get(self.state.selected()?).cloned()
    }

    pub fn cleanup(&mut self) {
        if let BottomBarKind::Notification(n) = &self.bottom_bar
            && n.is_expired()
        {
            self.bottom_bar = BottomBarKind::Hint
        }

        if let Some(kind) = &mut self.popup {
            match kind {
                PopUpKind::ExportTotp(popup) => popup.cleanup(),
                PopUpKind::ConfirmDelete { .. } => (),
            }
        }
    }

    fn notify(&mut self, n: Notification) {
        self.bottom_bar = BottomBarKind::Notification(n)
    }
}
