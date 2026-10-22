use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::{
    backend::models::SignIn,
    frontend::{
        components::{github_panel, logo},
        stores::wizard::Wizard,
        theme::{CYAN, GREEN, RED, YELLOW, pane_border},
    },
};

fn progress(step: &SignIn) -> Vec<Line<'static>> {
    match step {
        SignIn::Starting => vec![Line::from(Span::styled(
            "  asking GitHub for a code…",
            Style::new().fg(YELLOW),
        ))],
        SignIn::Code { user_code, uri } => vec![
            Line::from(vec![
                Span::raw("  Open "),
                Span::styled(uri.clone(), Style::new().fg(CYAN).underlined()),
                Span::raw(" and enter"),
            ]),
            Line::raw(""),
            Line::from(Span::styled(
                format!("      {user_code}"),
                Style::new().fg(GREEN).bold(),
            )),
            Line::raw(""),
            Line::from(Span::styled(
                "  waiting for the browser…",
                Style::new().dim(),
            )),
        ],
        SignIn::Step(what) => vec![Line::from(Span::styled(
            format!("  {what}…"),
            Style::new().fg(YELLOW),
        ))],
        SignIn::Done(login) => vec![Line::from(Span::styled(
            format!("  signed in as {login}"),
            Style::new().fg(GREEN),
        ))],
        SignIn::Failed(why) => vec![Line::from(Span::styled(
            format!("  ✗ {why}"),
            Style::new().fg(RED),
        ))],
    }
}

pub fn render(f: &mut Frame, area: Rect, w: &Wizard) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Every repository is private, so this machine signs in first.",
            Style::new().dim(),
        )),
        Line::raw(""),
    ];
    lines.extend(github_panel::lines(w.github.as_ref()));
    if let Some(step) = &w.sign_in {
        lines.push(Line::raw(""));
        lines.extend(progress(step));
    }
    lines.push(Line::raw(""));
    let keys = if w.signed_in() {
        " <Enter> continue · r check again · q quit "
    } else if w.signing_in() {
        " r check again · q quit "
    } else {
        " s sign in with GitHub · r check again · q quit "
    };
    lines.push(Line::from(Span::styled(keys, Style::new().fg(CYAN).bold())));

    let [col] = Layout::horizontal([Constraint::Length(80)])
        .flex(Flex::Center)
        .areas(area);
    let height = lines.len() as u16 + 2;
    let [top, _, panel] = Layout::vertical([
        Constraint::Length(logo::HEIGHT),
        Constraint::Length(1),
        Constraint::Length(height),
    ])
    .flex(Flex::Center)
    .areas(col);
    let [mark] = Layout::horizontal([Constraint::Length(logo::WIDTH)])
        .flex(Flex::Center)
        .areas(top);
    f.render_widget(Paragraph::new(logo::lines(w.tick)), mark);
    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_style(pane_border(true))
                .title(" mc2 · bootstrap · GitHub "),
        ),
        panel,
    );
}
