use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::frontend::theme::{BLUE, CYAN};

const LOGO: [&str; 7] = [
    "                            ░██████ ",
    "                           ░██   ░██",
    "░█████████████   ░███████        ░██",
    "░██   ░██   ░██ ░██    ░██   ░█████ ",
    "░██   ░██   ░██ ░██         ░██     ",
    "░██   ░██   ░██ ░██    ░██ ░██      ",
    "░██   ░██   ░██  ░███████  ░████████",
];

pub const HEIGHT: u16 = LOGO.len() as u16;
pub const WIDTH: u16 = 36;

const WHITE: Color = Color::Indexed(231);
const SKY: Color = Color::Indexed(51);

// A bright band sweeps diagonally across the letters; `tick` moves it one column.
fn shade(x: usize, y: usize, tick: u64) -> Color {
    let span = WIDTH as i64 + HEIGHT as i64 + 24;
    let band = (tick as i64 % span) - 12;
    match ((x + y) as i64 - band).abs() {
        0 => WHITE,
        1..=2 => SKY,
        3..=5 => CYAN,
        _ => BLUE,
    }
}

pub fn lines(tick: u64) -> Vec<Line<'static>> {
    LOGO.iter()
        .enumerate()
        .map(|(y, row)| {
            Line::from(
                row.chars()
                    .enumerate()
                    .map(|(x, c)| match c {
                        '█' => Span::styled("█", Style::new().fg(shade(x, y, tick))),
                        '░' => Span::styled("░", Style::new().fg(BLUE).dim()),
                        _ => Span::raw(" "),
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}
