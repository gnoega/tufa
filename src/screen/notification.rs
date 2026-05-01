use std::time::{Duration, Instant};

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::ui::{GREEN, RED, TEXT};

#[derive(Debug, Clone)]
pub struct Notification {
    pub msg: String,
    kind: NotificationKind,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
enum NotificationKind {
    Success,
    Danger,
    Info,
}

impl Notification {
    fn new(msg: impl Into<String>, kind: NotificationKind) -> Self {
        Self {
            msg: msg.into(),
            kind,
            expires_at: Instant::now() + Duration::from_secs(2),
        }
    }

    pub fn success(msg: impl Into<String>) -> Self {
        Self::new(msg, NotificationKind::Success)
    }

    pub fn info(msg: impl Into<String>) -> Self {
        Self::new(msg, NotificationKind::Info)
    }

    pub fn danger(msg: impl Into<String>) -> Self {
        Self::new(msg, NotificationKind::Danger)
    }

    pub fn draw(&self) -> Line<'_> {
        let (prefix, prefix_style) = match self.kind {
            NotificationKind::Success => (Some("✓ "), Style::default().fg(GREEN).bold()),
            NotificationKind::Danger => (Some("! "), Style::default().fg(RED).bold()),
            NotificationKind::Info => (None, Style::default()),
        };

        match prefix {
            Some(p) => Line::from(vec![
                Span::styled(p, prefix_style),
                Span::styled(format!("{} ", self.msg), Style::default().fg(TEXT)),
            ]),
            None => Line::styled(&self.msg, Style::default().fg(TEXT)),
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}
