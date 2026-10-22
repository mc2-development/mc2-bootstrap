use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::frontend::{
    stores::wizard::Wizard,
    theme::{RED, YELLOW},
    types::Mode,
};

pub fn render(f: &mut Frame, area: Rect, w: &Wizard) {
    let line = match w.mode {
        Mode::Command => Line::from(format!(":{}█", w.input)),
        Mode::Search => Line::from(format!("/{}█", w.input)),
        // Dots, not the characters: only how many have been typed shows.
        Mode::Password => Line::from(vec![
            Span::styled(" server passphrase: ", Style::new().fg(YELLOW).bold()),
            Span::raw(format!("{}█", "•".repeat(w.input.chars().count()))),
            Span::styled("   Enter unlocks · Esc cancels", Style::new().dim()),
        ]),
        _ if !w.status.is_empty() => Line::from(Span::styled(
            format!(" {}", w.status),
            Style::new().fg(RED).bold(),
        )),
        _ => Line::from(Span::styled(
            " <C-h>/<C-l> panes · j/k move · <space> tick · / search · : command · ? keymaps",
            Style::new().dim(),
        )),
    };
    f.render_widget(Paragraph::new(line), area);
}
