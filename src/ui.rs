use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
};

pub const DIM: Color = Color::Rgb(100, 100, 100);
pub const SUBTEXT: Color = Color::Rgb(150, 150, 150);
pub const TEXT: Color = Color::Rgb(220, 220, 220);
pub const ACCENT: Color = Color::Rgb(250, 200, 80);
pub const GREEN: Color = Color::Rgb(100, 200, 120);
pub const YELLOW: Color = Color::Rgb(240, 180, 60);
pub const RED: Color = Color::Rgb(220, 80, 80);

pub fn key_hint<'a>(key: &'a str, desc: &'a str) -> Vec<Span<'a>> {
    vec![
        Span::raw("  "),
        Span::styled(key, Style::default().fg(ACCENT).bold()),
        Span::raw(" "),
        Span::styled(desc, Style::default().fg(DIM)),
    ]
}

pub fn full_separator(width: u16) -> Line<'static> {
    Line::styled(
        "─".repeat(width as usize),
        Style::default().fg(Color::Rgb(50, 50, 50)),
    )
}

pub fn ttl_color(ttl: u64) -> Color {
    match ttl {
        0..=4 => RED,
        5..=9 => YELLOW,
        _ => GREEN,
    }
}

pub fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
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
