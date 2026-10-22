use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{
    backend::models::{GitHub, github::OWNER},
    frontend::{
        theme::{GREEN, RED, YELLOW},
        utils::tilde,
    },
};

fn row(ok: Option<bool>, label: &str, value: String) -> Line<'static> {
    let (mark, color): (&str, Color) = match ok {
        Some(true) => ("✓", GREEN),
        Some(false) => ("✗", RED),
        None => ("…", YELLOW),
    };
    Line::from(vec![
        Span::styled(format!("  {mark} "), Style::new().fg(color)),
        Span::raw(format!("{label:<16}")),
        Span::styled(value, Style::new().dim()),
    ])
}

fn hint(text: String) -> Line<'static> {
    Line::from(Span::styled(
        format!("      {text}"),
        Style::new().fg(YELLOW),
    ))
}

pub fn lines(github: Option<&GitHub>) -> Vec<Line<'static>> {
    let Some(gh) = github else {
        return ["SSH key", "signed in as", OWNER]
            .into_iter()
            .map(|l| row(None, l, "checking…".into()))
            .collect();
    };
    let mut out = vec![row(
        Some(gh.key.is_some()),
        "SSH key",
        gh.key
            .as_deref()
            .map_or_else(|| "none yet".to_string(), tilde),
    )];
    if gh.key.is_none() {
        out.push(hint(
            "the sign-in creates one and adds it to the account".into(),
        ));
    }
    out.push(row(
        Some(gh.ssh_user.is_some()),
        "signed in as",
        gh.ssh_user
            .clone()
            .unwrap_or_else(|| "nobody: the key is not on GitHub".into()),
    ));
    out.push(row(
        Some(gh.org_access),
        OWNER,
        if gh.org_access {
            "reachable"
        } else {
            "not reachable"
        }
        .into(),
    ));
    if let (Some(user), false) = (&gh.ssh_user, gh.org_access) {
        out.push(hint(format!(
            "'{user}' cannot see {OWNER}: ask for an invite, or use"
        )));
        out.push(hint(
            "the other key: MC2_SSH_KEY=~/.ssh/id_work mc2-bootstrap".into(),
        ));
    }
    out
}
