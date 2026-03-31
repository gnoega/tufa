use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Cell, Row, Table, TableState},
};

use crate::{
    clipboard,
    screen::{Screen, export::ExportTotp, vault_list::VaultList},
    totp::{TotpEntry, totp_ttl},
    ui::{ACCENT, DIM, GREEN, SUBTEXT, TEXT, full_separator, key_hint, ttl_color},
};

#[derive(Debug)]
pub struct AccountList {
    vault_name: String,
    entries: Vec<TotpEntry>,
    state: TableState,
    pub copied: Option<String>,
}

impl AccountList {
    pub fn new(vault_name: impl Into<String>, entries: Vec<TotpEntry>) -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        Self {
            vault_name: vault_name.into(),
            entries,
            state,
            copied: None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });

        let [title_area, sep_area, table_area, gauge_area, _, hint_area] = Layout::vertical([
            Constraint::Length(1), // vault name
            Constraint::Length(1), // separator
            Constraint::Fill(1),   // table
            Constraint::Length(1), // gauge
            Constraint::Length(1), // spacer
            Constraint::Length(1), // hints
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

        let hint_line = match &self.copied {
            Some(name) => Line::from(vec![
                Span::styled("✓ ", Style::default().fg(GREEN).bold()),
                Span::styled(name.clone(), Style::default().fg(TEXT)),
                Span::styled(" copied to clipboard", Style::default().fg(SUBTEXT)),
            ]),
            None => {
                let mut spans: Vec<Span> = vec![];
                spans.extend(key_hint("↵", "copy"));
                spans.extend(key_hint("↑↓ / jk", "navigate"));
                spans.extend(key_hint("e", "export"));
                spans.extend(key_hint("esc", "back"));
                spans.extend(key_hint("q", "quit"));
                Line::from(spans)
            }
        };
        frame.render_widget(hint_line, hint_area);
    }

    pub fn handle_key(mut self, key: KeyCode) -> Screen {
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
                    Some(entry) => Screen::ExportPopUp(ExportTotp::new(self, entry)),
                    None => Screen::AccountList(self),
                }
            }
            KeyCode::Enter => {
                if let Some(i) = self.state.selected()
                    && let Some(entry) = self.entries.get(i)
                    && let Ok(code) = entry.generate_otp()
                    && clipboard::copy_to_clipboard(&code)
                {
                    self.copied = Some(entry.display_name());
                }

                Screen::AccountList(self)
            }
            _ => Screen::AccountList(self),
        }
    }
    pub fn get_selected(&self) -> Option<TotpEntry> {
        self.entries.get(self.state.selected()?).cloned()
    }
}
