use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Padding, Paragraph},
};

use crate::frontend::theme::{CYAN, YELLOW};

const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "Move",
        &[
            ("↑ ↓   j k", "up and down (5j moves five)"),
            ("{ }", "previous / next section"),
            ("gg   G", "top / bottom"),
            ("Ctrl-d   Ctrl-u", "half a page down / up"),
        ],
    ),
    (
        "Panes",
        &[
            ("← →   h l", "the menu / the preview"),
            ("Tab   H L", "the next preview tab"),
        ],
    ),
    (
        "Choose",
        &[
            ("Space   Enter", "pick a role, tick a repository"),
            (":wq", "start, same as the ▶ button"),
        ],
    ),
    (
        "Search",
        &[
            ("/", "type, then Enter"),
            ("n   N", "next / previous match"),
        ],
    ),
    ("Leave", &[("q   Ctrl-c", "quit without changing anything")]),
];

pub fn render(f: &mut Frame, screen: Rect) {
    let mut lines = Vec::new();
    for (i, (title, keys)) in HELP.iter().enumerate() {
        if i > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(Span::styled(
            *title,
            Style::new().fg(CYAN).bold(),
        )));
        for (key, what) in *keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<18}"), Style::new().fg(YELLOW).bold()),
                Span::raw(*what),
            ]));
        }
    }
    let width = 60.min(screen.width);
    let height = (lines.len() as u16 + 4).min(screen.height);
    let area = Rect {
        x: screen.x + (screen.width.saturating_sub(width)) / 2,
        y: screen.y + (screen.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    f.render_widget(Clear, area.outer(Margin::new(1, 0)).clamp(screen));
    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_style(Style::new().fg(CYAN))
                .padding(Padding::new(2, 2, 1, 0))
                .title(" help ")
                .title_bottom(" any key closes "),
        ),
        area,
    );
}
