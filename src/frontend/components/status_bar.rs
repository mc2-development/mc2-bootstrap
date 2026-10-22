use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    backend::models::Setup,
    frontend::{
        stores::wizard::Wizard,
        theme::{BLUE, CYAN, GREEN, INK, PINK, RED, YELLOW},
        types::{Focus, Mode},
    },
};

pub fn render(f: &mut Frame, area: Rect, w: &Wizard) {
    let (mode, color) = match w.mode {
        Mode::Normal => (" NORMAL ", GREEN),
        Mode::Command => (" COMMAND ", YELLOW),
        Mode::Search => (" SEARCH ", PINK),
        Mode::Help => (" HELP ", BLUE),
        Mode::Password => (" LOCKED ", RED),
    };
    let focus = if w.focus == Focus::Menu {
        "menu"
    } else {
        "preview"
    };
    let mut spans = vec![
        Span::styled(mode, Style::new().fg(INK).bg(color).bold()),
        Span::raw(format!(" {focus} ")),
    ];
    if !w.count.is_empty() || !w.seq.is_empty() {
        spans.push(Span::styled(
            format!(" {}{} ", w.count, w.seq),
            Style::new().fg(INK).bg(CYAN),
        ));
    }
    let pm = w
        .machine
        .package_manager
        .map_or_else(|| "no package manager".to_string(), |pm| pm.to_string());
    spans.push(Span::styled(
        format!("  {} {} · {pm}  ", w.machine.os, w.machine.arch),
        Style::new().dim(),
    ));

    match w.sel.setup {
        Setup::Developer => {
            spans.push(match &w.github {
                None => Span::styled("github …  ", Style::new().fg(YELLOW)),
                Some(gh) if gh.ready() => Span::styled(
                    format!("github ✓ {}  ", gh.ssh_user.as_deref().unwrap_or_default()),
                    Style::new().fg(GREEN),
                ),
                Some(_) => Span::styled("github ✗  ", Style::new().fg(RED)),
            });
            let plan = w.plan();
            let missing = plan.missing(&w.machine).len();
            spans.push(if missing == 0 {
                Span::styled("tools ✓  ", Style::new().fg(GREEN))
            } else {
                Span::styled(format!("{missing} tools missing  "), Style::new().fg(RED))
            });
            spans.push(Span::styled(
                format!("{} to clone", plan.to_clone(&w.machine).len()),
                Style::new().dim(),
            ));
        }
        Setup::Server => {
            let failing = w.failing().len();
            spans.push(if failing == 0 {
                Span::styled("checks ✓", Style::new().fg(GREEN))
            } else {
                Span::styled(format!("{failing} checks failing"), Style::new().fg(RED))
            });
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
