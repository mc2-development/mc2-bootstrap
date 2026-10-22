use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{
    backend::models::{Machine, Os, Plan, Tool},
    frontend::theme::{CYAN, GREEN, RED, YELLOW},
};

pub fn lines(plan: &Plan, machine: &Machine) -> Vec<Line<'static>> {
    let mut out = vec![
        Line::from(Span::styled(
            format!("{} · {}", plan.role, plan.role.about()),
            Style::new().fg(CYAN).bold(),
        )),
        Line::raw(""),
    ];
    if let Some(warning) = package_manager_warning(machine) {
        out.push(Line::from(Span::styled(warning, Style::new().fg(YELLOW))));
        out.push(Line::raw(""));
    }
    let (missing, installed): (Vec<_>, Vec<_>) = plan.tools.iter().partition(|t| !machine.has(t));
    section(&mut out, "This machine needs", &missing, ("✗", RED));
    section(&mut out, "Already installed", &installed, ("✓", GREEN));
    out
}

// The tag says which choice brought the tool in, so `terraform` on the list explains itself.
fn section(out: &mut Vec<Line<'static>>, title: &str, tools: &[&&Tool], mark: (&str, Color)) {
    if tools.is_empty() {
        return;
    }
    out.push(Line::from(Span::styled(
        format!("{title} ({})", tools.len()),
        Style::new().bold(),
    )));
    for t in tools {
        out.push(Line::from(vec![
            Span::styled(format!("  {} ", mark.0), Style::new().fg(mark.1)),
            Span::raw(format!("{:<14}", t.name)),
            Span::styled(format!("{:<10}", t.need.label()), Style::new().fg(CYAN)),
            Span::styled(t.why, Style::new().dim()),
        ]));
    }
    out.push(Line::raw(""));
}

fn package_manager_warning(machine: &Machine) -> Option<String> {
    if machine.package_manager.is_some() {
        return None;
    }
    Some(match machine.os {
        Os::Mac => {
            "! Homebrew is not installed; every tool comes through it, so it goes first".into()
        }
        Os::Linux => "! No apt, dnf or pacman found to install the tools with".into(),
        Os::Windows => "! winget is not installed: App Installer, from the Microsoft Store".into(),
    })
}
