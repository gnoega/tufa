use std::{
    io,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};

use crate::screen::{Screen, vault_list::VaultList};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Default)]
pub struct App {
    pub screen: Screen,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::default(),
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            if matches!(self.screen, Screen::Exit) {
                break;
            }
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        match &mut self.screen {
            Screen::VaultList(s) => s.render(frame),
            Screen::PasswordPrompt(s) => {
                VaultList::new().render(frame);
                s.render(frame);
            }
            Screen::AccountList(s) => s.render(frame),
            Screen::ExportPopUp(s) => s.render(frame),
            Screen::Exit => {}
        }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(time_until_next_second())? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => self.handle_key_event(k),
                _ => {}
            }
        } else {
            match &mut self.screen {
                Screen::AccountList(s) => s.copied = None,
                Screen::ExportPopUp(s) => s.copied = None,
                _ => {}
            }
        }
        Ok(())
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        let screen = std::mem::take(&mut self.screen);

        self.screen = match screen {
            Screen::VaultList(s) => s.handle_key(key.code),
            Screen::PasswordPrompt(s) => s.handle_key(key.code),
            Screen::AccountList(s) => s.handle_key(key.code),
            Screen::ExportPopUp(s) => s.handle_key(key.code),
            Screen::Exit => Screen::Exit,
        };
    }
}

pub fn time_until_next_second() -> Duration {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let ns = now.subsec_nanos() as u64;
    Duration::from_nanos(1_000_000_000u64.saturating_sub(ns)).max(Duration::from_millis(1))
}
