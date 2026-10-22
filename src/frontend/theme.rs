use ratatui::style::{Color, Style};

pub const BLUE: Color = Color::Indexed(39);
pub const CYAN: Color = Color::Indexed(45);
pub const GREEN: Color = Color::Indexed(46);
pub const YELLOW: Color = Color::Indexed(220);
pub const RED: Color = Color::Indexed(203);
pub const PINK: Color = Color::Indexed(213);
pub const INK: Color = Color::Indexed(16);

pub fn pane_border(focused: bool) -> Style {
    if focused {
        Style::new().fg(CYAN).bold()
    } else {
        Style::new().dim()
    }
}
