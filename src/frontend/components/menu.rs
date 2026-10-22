use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState},
};

use crate::{
    backend::models::{Group, Os, Setup},
    frontend::{
        stores::wizard::Wizard,
        theme::{CYAN, GREEN, RED, pane_border},
        types::{Focus, Row},
    },
};

pub fn render(f: &mut Frame, area: Rect, w: &Wizard) {
    let focused = w.focus == Focus::Menu;
    let items: Vec<ListItem> = w
        .rows
        .iter()
        .map(|r| ListItem::new(row_line(w, r)))
        .collect();
    let list = List::new(items)
        .block(
            Block::bordered()
                .border_style(pane_border(focused))
                .title(" mc2 · bootstrap "),
        )
        .highlight_style(if focused {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new().add_modifier(Modifier::BOLD)
        });
    let mut state = ListState::default().with_selected(Some(w.cursor));
    f.render_stateful_widget(list, area, &mut state);
}

fn mark(on: bool, marks: (&'static str, &'static str)) -> Span<'static> {
    if on {
        Span::styled(marks.0, Style::new().fg(GREEN).bold())
    } else {
        Span::raw(marks.1)
    }
}

fn note(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::new().dim())
}

fn row_line(w: &Wizard, row: &Row) -> Line<'static> {
    match *row {
        Row::Header(t) => Line::from(Span::styled(t, Style::new().fg(CYAN).bold())),
        Row::Note(t) => Line::from(note(t)),
        Row::Gap => Line::raw(""),
        Row::Setup(s) => {
            let mut line = Line::from(vec![
                mark(w.sel.setup == s, ("(•) ", "( ) ")),
                Span::raw(s.to_string()),
            ]);
            if s == Setup::Server && w.machine.os != Os::Linux {
                line.push_span(note("  · Linux only"));
            }
            line
        }
        Row::Role(r) => Line::from(vec![
            mark(w.sel.role == r, ("(•) ", "( ) ")),
            Span::raw(r.to_string()),
        ]),
        Row::Repo(r) => {
            let on = w.sel.repos(&w.machine).contains(&r);
            let mut line = Line::from(vec![mark(on, ("[x] ", "[ ] ")), Span::raw(r.name)]);
            if w.machine.is_cloned(r) {
                line.push_span(note("  · cloned"));
            } else if r.group == Group::Placeholder {
                line.push_span(note("  · placeholder"));
            }
            line
        }
        Row::Check(i) => {
            let c = &w.checks[i];
            let (m, color) = if c.ok { ("✓ ", GREEN) } else { ("✗ ", RED) };
            Line::from(vec![
                Span::styled(m, Style::new().fg(color)),
                Span::raw(format!("{:<14}", c.label)),
                note(c.value.clone()),
            ])
        }
        Row::Start => Line::from(Span::styled(
            match w.sel.setup {
                Setup::Developer => "▶ Set up this machine",
                Setup::Server => "▶ Install k3s",
            },
            Style::new().fg(GREEN).bold(),
        )),
    }
}
