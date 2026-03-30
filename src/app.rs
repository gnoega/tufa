use std::{
    io,
    ops::Deref,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap,
    },
};
use zeroize::Zeroizing;

use crate::{
    totp::{TotpEntry, totp_ttl},
    vault::{Vault, VaultError},
};

const DIM: Color = Color::Rgb(100, 100, 100);
const SUBTEXT: Color = Color::Rgb(150, 150, 150);
const TEXT: Color = Color::Rgb(220, 220, 220);
const ACCENT: Color = Color::Rgb(250, 200, 80);
const GREEN: Color = Color::Rgb(100, 200, 120);
const YELLOW: Color = Color::Rgb(240, 180, 60);
const RED: Color = Color::Rgb(220, 80, 80);

fn key_hint<'a>(key: &'a str, desc: &'a str) -> Vec<Span<'a>> {
    vec![
        Span::raw("  "),
        Span::styled(key, Style::default().fg(ACCENT).bold()),
        Span::raw(" "),
        Span::styled(desc, Style::default().fg(DIM)),
    ]
}

fn full_separator(width: u16) -> Line<'static> {
    Line::styled(
        "─".repeat(width as usize),
        Style::default().fg(Color::Rgb(50, 50, 50)),
    )
}

fn ttl_color(ttl: u64) -> Color {
    match ttl {
        0..=4 => RED,
        5..=9 => YELLOW,
        _ => GREEN,
    }
}

#[derive(Debug, Default)]
enum Screen {
    #[default]
    VaultList,
    PasswordPrompt {
        vault_name: String,
        input: Zeroizing<String>,
        error: Option<&'static str>,
    },
    AccountList {
        vault_name: String,
        entries: Vec<TotpEntry>,
        copied: Option<String>,
    },
    ExportPopUp {
        vault_name: String,
        entries: Vec<TotpEntry>,
        copied: Option<String>,
    },
}

#[derive(Debug, Default)]
pub struct App {
    screen: Screen,
    vault_list: Vec<String>,
    vault_list_state: ListState,
    totp_list_state: TableState,
    exit: bool,
}

impl App {
    pub fn new() -> Self {
        let mut vault_list_state = ListState::default();
        vault_list_state.select(Some(0));

        let mut raw = Vault::list_all();
        raw.sort();
        let vault_list = raw
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
            screen: Screen::default(),
            vault_list,
            vault_list_state,
            totp_list_state: TableState::default(),
            exit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        match &self.screen {
            Screen::VaultList => self.render_vault_list(frame, area),
            Screen::PasswordPrompt {
                vault_name,
                input,
                error,
            } => {
                let vn = vault_name.clone();
                let input = input.deref().clone();
                let err = *error;
                self.render_vault_list(frame, area);
                render_password_popup(frame, area, &vn, &input, err);
            }
            Screen::AccountList {
                vault_name,
                entries,
                copied,
            } => {
                let entries = entries.clone();
                let vault_name = vault_name.clone();
                self.render_account_list(frame, area, vault_name, entries, copied.clone());
            }
            Screen::ExportPopUp {
                entries,
                vault_name,
                copied,
            } => {
                let selected = self.totp_list_state.selected().unwrap_or(0);
                let entries = entries.clone();
                let vault_name = vault_name.clone();
                let copied = copied.clone();
                self.render_account_list(frame, area, vault_name, entries.clone(), None);
                if let Some(entry) = entries.get(selected) {
                    let _ = render_export_popup(frame, area, entry, copied.clone());
                }
            }
        }
    }

    fn render_vault_list(&mut self, frame: &mut Frame, area: Rect) {
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
            .vault_list
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

        frame.render_stateful_widget(list, list_area, &mut self.vault_list_state);

        let mut hints: Vec<Span> = vec![];
        hints.extend(key_hint("↵", "open"));
        hints.extend(key_hint("↑↓ / jk", "navigate"));
        hints.extend(key_hint("q", "quit"));
        frame.render_widget(Line::from(hints), hint_area);
    }

    fn render_account_list(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        vault_name: impl Into<String>,
        entries: Vec<TotpEntry>,
        copied: Option<String>,
    ) {
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
                Span::styled(vault_name.into(), Style::default().fg(TEXT).bold()),
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

        let rows: Vec<Row> = if entries.is_empty() {
            vec![Row::new(vec![
                Cell::from("  no accounts").style(Style::default().fg(DIM)),
            ])]
        } else {
            entries
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

        frame.render_stateful_widget(table, table_area, &mut self.totp_list_state);

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

        let hint_line = match copied {
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

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(time_until_next_second())? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => self.handle_key_event(k),
                _ => {}
            }
        } else {
            match &mut self.screen {
                Screen::AccountList { copied, .. } => *copied = None,
                Screen::ExportPopUp { copied, .. } => *copied = None,
                _ => {}
            }
        }
        Ok(())
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        let screen = std::mem::take(&mut self.screen);

        self.screen = match screen {
            Screen::VaultList => match key.code {
                KeyCode::Char('q') => {
                    self.exit = true;
                    Screen::VaultList
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = self.vault_list_state.selected().unwrap_or(0);
                    self.vault_list_state
                        .select(Some((i + 1).min(self.vault_list.len().saturating_sub(1))));
                    Screen::VaultList
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = self.vault_list_state.selected().unwrap_or(0);
                    self.vault_list_state.select(Some(i.saturating_sub(1)));
                    Screen::VaultList
                }
                KeyCode::Enter => {
                    if let Some(idx) = self.vault_list_state.selected()
                        && let Some(name) = self.vault_list.get(idx).cloned()
                    {
                        return self.screen = Screen::PasswordPrompt {
                            vault_name: name,
                            input: Zeroizing::new(String::new()),
                            error: None,
                        };
                    }
                    Screen::VaultList
                }
                _ => Screen::VaultList,
            },
            Screen::PasswordPrompt {
                vault_name,
                mut input,
                error,
            } => match key.code {
                KeyCode::Esc => Screen::VaultList,
                KeyCode::Backspace => {
                    input.pop();
                    Screen::PasswordPrompt {
                        vault_name,
                        input,
                        error: None,
                    }
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    Screen::PasswordPrompt {
                        vault_name,
                        input,
                        error: None,
                    }
                }
                KeyCode::Enter => {
                    let password = input.as_bytes().to_vec();
                    match Vault::new(&vault_name).load(&password) {
                        Ok(entries) => {
                            self.totp_list_state.select(Some(0));
                            Screen::AccountList {
                                vault_name,
                                entries,
                                copied: None,
                            }
                        }
                        Err(VaultError::WrongPassword) => Screen::PasswordPrompt {
                            vault_name,
                            input: Zeroizing::new(String::new()),
                            error: Some("wrong password"),
                        },
                        Err(_) => Screen::PasswordPrompt {
                            vault_name,
                            input: Zeroizing::new(String::new()),
                            error: Some("failed to open vault"),
                        },
                    }
                }
                _ => Screen::PasswordPrompt {
                    vault_name,
                    input,
                    error,
                },
            },
            Screen::AccountList {
                vault_name,
                entries,
                copied,
            } => match key.code {
                KeyCode::Char('q') => {
                    self.exit = true;
                    Screen::AccountList {
                        vault_name,
                        entries,
                        copied,
                    }
                }
                KeyCode::Esc => Screen::VaultList,
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = self.totp_list_state.selected().unwrap_or(0);
                    self.totp_list_state
                        .select(Some((i + 1).min(entries.len().saturating_sub(1))));
                    Screen::AccountList {
                        vault_name,
                        entries,
                        copied,
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = self.totp_list_state.selected().unwrap_or(0);
                    self.totp_list_state.select(Some(i.saturating_sub(1)));
                    Screen::AccountList {
                        vault_name,
                        entries,
                        copied,
                    }
                }
                KeyCode::Char('e') => Screen::ExportPopUp {
                    vault_name,
                    entries,
                    copied,
                },
                KeyCode::Enter => {
                    let mut copied = copied.clone();
                    if let Some(i) = self.totp_list_state.selected()
                        && let Some(entry) = entries.get(i)
                        && let Ok(code) = entry.generate_otp()
                        && copy_to_clipboard(&code)
                    {
                        copied = Some(entry.display_name());
                    }

                    Screen::AccountList {
                        vault_name,
                        entries,
                        copied,
                    }
                }
                _ => Screen::AccountList {
                    vault_name,
                    entries,
                    copied,
                },
            },
            Screen::ExportPopUp {
                vault_name,
                entries,
                copied,
            } => match key.code {
                KeyCode::Esc => Screen::AccountList {
                    vault_name,
                    entries,
                    copied,
                },
                KeyCode::Char('y') => {
                    let mut copied = copied.clone();
                    if let Some(i) = self.totp_list_state.selected()
                        && let Some(entry) = entries.get(i)
                        && copy_to_clipboard(entry.to_uri().as_str())
                    {
                        copied = Some(entry.display_name());
                    };

                    Screen::ExportPopUp {
                        vault_name,
                        entries,
                        copied,
                    }
                }
                _ => Screen::AccountList {
                    vault_name,
                    entries,
                    copied,
                },
            },
        };
    }
}

fn render_password_popup(
    frame: &mut Frame,
    area: Rect,
    vault_name: &str,
    input: &str,
    error: Option<&str>,
) {
    let height = if error.is_some() { 9 } else { 7 };
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
        Constraint::Length(if error.is_some() { 1 } else { 0 }),
    ])
    .areas(inner);

    frame.render_widget(
        Line::from(vec![
            Span::styled("unlock ", Style::default().fg(SUBTEXT)),
            Span::styled(vault_name, Style::default().fg(TEXT).bold()),
        ]),
        label_area,
    );

    frame.render_widget(full_separator(popup_area.width), sep_area);

    let masked = format!("{}▌", "•".repeat(input.len()));
    frame.render_widget(
        Paragraph::new(masked).style(Style::default().fg(ACCENT)),
        field_area,
    );

    let mut hints: Vec<Span> = vec![];
    hints.extend(key_hint("↵", "unlock"));
    hints.extend(key_hint("esc", "cancel"));
    frame.render_widget(Line::from(hints), hint_area);

    if let Some(msg) = error {
        frame.render_widget(Line::styled(msg, Style::default().fg(RED)), error_area);
    }
}

fn render_export_popup(
    frame: &mut Frame,
    area: Rect,
    entry: &TotpEntry,
    copied: Option<String>,
) -> Result<(), qrcode::types::QrError> {
    let entry_uri = entry.to_uri();
    let uri_str = entry_uri.to_string();
    let (qrcode_string, _qr_width, qr_height) = entry_uri.to_qrcode_rendered()?;

    let popup_width = (area.width as u32 * 80 / 100) as u16;
    let inner_width = popup_width.saturating_sub(2).max(1);
    let uri_lines = (uri_str.len() as u16).div_ceil(inner_width).max(1);

    let ideal_height = 2 + 1 + 1 + qr_height + 1 + uri_lines;
    let popup_height = ideal_height.min(area.height.saturating_sub(2));

    let popup_area = centered_rect(80, popup_height, area);

    let hint_line = match copied {
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
        Paragraph::new(entry.display_name()).style(Style::default().fg(TEXT)),
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

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let [_, mid, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(area);

    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Percentage(percent_x),
        Constraint::Fill(1),
    ])
    .split(mid)[1]
}

pub fn time_until_next_second() -> Duration {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let ns = now.subsec_nanos() as u64;
    Duration::from_nanos(1_000_000_000u64.saturating_sub(ns)).max(Duration::from_millis(1))
}

fn copy_to_clipboard(text: &str) -> bool {
    use std::io::Write;

    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),                        // Wayland
        ("xclip", &["-selection", "clipboard"]), // X11
        ("xsel", &["-bi"]),                      // X11 alt
        ("pbcopy", &[]),                         // macOS
    ];

    for (cmd, args) in candidates {
        if let Ok(mut child) = std::process::Command::new(cmd)
            .args(*args)
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok() {
                return true;
            }
        }
    }

    false
}
