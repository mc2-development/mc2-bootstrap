use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use super::{check_list, machine_info, repo_tree, tool_list};
use crate::frontend::{
    stores::wizard::Wizard,
    theme::pane_border,
    types::{Focus, Tab},
};

pub fn render(f: &mut Frame, area: Rect, w: &Wizard) {
    let title = Line::from(
        Tab::for_setup(w.sel.setup)
            .iter()
            .map(|t| {
                let style = if *t == w.tab {
                    Style::new().add_modifier(Modifier::REVERSED)
                } else {
                    Style::new().dim()
                };
                Span::styled(format!(" {} ", t.label()), style)
            })
            .collect::<Vec<_>>(),
    );
    let lines = match w.tab {
        Tab::Tools => tool_list::lines(&w.plan(), &w.machine),
        Tab::Repos => repo_tree::lines(&w.plan(), &w.machine),
        Tab::Checks => check_list::lines(&w.checks),
        Tab::Machine => machine_info::lines(&w.machine, w.github.as_ref()),
    };
    // Never scroll past the last line, and let key handling know how far that is.
    let inner = area.height.saturating_sub(2) as usize;
    let max = lines.len().saturating_sub(inner) as u16;
    w.max_scroll.set(max);
    let p = Paragraph::new(lines).scroll((w.scroll.min(max), 0)).block(
        Block::bordered()
            .border_style(pane_border(w.focus == Focus::Preview))
            .title(title),
    );
    f.render_widget(p, area);
}
