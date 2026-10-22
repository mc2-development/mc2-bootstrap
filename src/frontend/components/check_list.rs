use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    backend::models::Check,
    frontend::theme::{CYAN, GREEN, RED, YELLOW},
};

pub fn lines(checks: &[Check]) -> Vec<Line<'static>> {
    let mut out = vec![
        Line::from(Span::styled(
            "Nothing is installed until every check passes",
            Style::new().fg(CYAN).bold(),
        )),
        Line::raw(""),
    ];
    for c in checks {
        let (mark, color) = if c.ok { ("✓", GREEN) } else { ("✗", RED) };
        out.push(Line::from(vec![
            Span::styled(format!("  {mark} "), Style::new().fg(color)),
            Span::raw(format!("{:<14}", c.label)),
            Span::styled(c.value.clone(), Style::new().dim()),
        ]));
        if !c.ok {
            out.push(Line::from(Span::styled(
                format!("      {}", c.fix),
                Style::new().fg(YELLOW),
            )));
        }
    }
    out
}
