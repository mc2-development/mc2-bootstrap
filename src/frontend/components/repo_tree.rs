use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    backend::models::{Group, Machine, Plan},
    frontend::{
        theme::{BLUE, CYAN, GREEN},
        utils::tilde,
    },
};

pub fn lines(plan: &Plan, machine: &Machine) -> Vec<Line<'static>> {
    let mut out = vec![Line::from(Span::styled(
        format!("{}/", tilde(&machine.mc2_dir)),
        Style::new().fg(BLUE).bold(),
    ))];
    for group in Group::ALL {
        let repos: Vec<_> = plan.repos.iter().filter(|r| r.group == group).collect();
        if repos.is_empty() {
            continue;
        }
        out.push(Line::raw(""));
        out.push(Line::from(vec![
            Span::styled(format!("{} ", group.title()), Style::new().fg(CYAN).bold()),
            Span::styled(format!("· {}", group.about()), Style::new().dim()),
        ]));
        for (i, repo) in repos.iter().enumerate() {
            let branch = if i + 1 == repos.len() {
                "└─ "
            } else {
                "├─ "
            };
            let state = if machine.is_cloned(repo) {
                Span::styled("cloned", Style::new().dim())
            } else {
                Span::styled("will clone", Style::new().fg(GREEN))
            };
            out.push(Line::from(vec![
                Span::raw(branch),
                Span::styled(format!("{:<26}", repo.name), Style::new().fg(BLUE)),
                state,
            ]));
        }
    }
    out
}
