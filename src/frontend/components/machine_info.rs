use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    backend::{
        models::{GitHub, Machine, REPOS},
        services::detect::suggested_setup,
    },
    frontend::{components::github_panel, theme::CYAN, utils::tilde},
};

pub fn lines(m: &Machine, github: Option<&GitHub>) -> Vec<Line<'static>> {
    let unknown = || "unknown".to_string();
    let rows = [
        (
            "system",
            m.os_version.clone().unwrap_or_else(|| m.os.to_string()),
        ),
        ("architecture", m.arch.to_string()),
        ("hostname", m.hostname.clone().unwrap_or_else(unknown)),
        ("admin", if m.admin { "yes" } else { "no" }.to_string()),
        (
            "session",
            if m.desktop { "desktop" } else { "headless" }.to_string(),
        ),
        (
            "packages",
            m.package_manager
                .map_or_else(|| "none found".to_string(), |pm| pm.to_string()),
        ),
        ("workspace", tilde(&m.mc2_dir)),
        (
            "cloned",
            format!("{} of {} repositories", m.cloned.len(), REPOS.len()),
        ),
    ];
    let mut out = vec![
        Line::from(Span::styled(
            "What was detected",
            Style::new().fg(CYAN).bold(),
        )),
        Line::raw(""),
    ];
    out.extend(rows.into_iter().map(|(k, v)| {
        Line::from(vec![
            Span::styled(format!("  {k:<14}"), Style::new().dim()),
            Span::raw(v),
        ])
    }));
    out.push(Line::raw(""));
    out.push(Line::from(vec![
        Span::styled("  suggested     ", Style::new().dim()),
        Span::styled(suggested_setup(m).to_string(), Style::new().fg(CYAN)),
    ]));
    out.push(Line::raw(""));
    out.push(Line::from(Span::styled(
        "GitHub",
        Style::new().fg(CYAN).bold(),
    )));
    out.extend(github_panel::lines(github));
    out
}
