use std::{
    io,
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event},
};

use super::{
    components::help_dialog,
    composables::use_terminal::with_terminal,
    layouts, pages,
    stores::{selection::Selection, wizard::Wizard},
    types::{Mode, Outcome, Page, Request},
};
use crate::backend::{
    models::{Check, GitHub, Machine, SignIn},
    services::github,
};

const FRAME: Duration = Duration::from_millis(60);

// `None` means the wizard was quit without starting anything.
pub fn run(machine: Machine, checks: Vec<Check>, initial: Selection) -> Result<Option<Selection>> {
    let mut wizard = Wizard::new(machine, checks, initial);
    with_terminal(|terminal| event_loop(terminal, &mut wizard))?;
    Ok((wizard.outcome == Outcome::Start).then_some(wizard.sel))
}

// Both talk to GitHub over the network, so they run beside the screen and report back.
enum Update {
    Status(GitHub),
    SignIn(SignIn),
}

fn check_github(tx: &Sender<Update>) {
    let tx = tx.clone();
    thread::spawn(move || {
        let _ = tx.send(Update::Status(github::status()));
    });
}

fn start_sign_in(tx: &Sender<Update>, hostname: String) {
    let tx = tx.clone();
    thread::spawn(move || {
        github::sign_in(&hostname, |step| {
            let _ = tx.send(Update::SignIn(step));
        });
    });
}

fn event_loop(terminal: &mut DefaultTerminal, wizard: &mut Wizard) -> io::Result<()> {
    let (tx, rx) = mpsc::channel();
    check_github(&tx);
    let opened = Instant::now();
    while wizard.outcome == Outcome::Running {
        // From the clock, not the loop count: a key press redraws early and must not speed it up.
        wizard.tick = (opened.elapsed().as_millis() / FRAME.as_millis()) as u64;
        while let Ok(update) = rx.try_recv() {
            match update {
                Update::Status(status) => wizard.github = Some(status),
                // A finished sign-in has put a key on the account; checking again proves it.
                Update::SignIn(SignIn::Done(_)) => {
                    wizard.sign_in = None;
                    wizard.github = None;
                    check_github(&tx);
                }
                Update::SignIn(step) => wizard.sign_in = Some(step),
            }
        }
        terminal.draw(|f| draw(f, wizard))?;
        if event::poll(FRAME)?
            && let Event::Key(key) = event::read()?
        {
            wizard.handle_key(key);
        }
        match wizard.request.take() {
            Some(Request::SignIn) => {
                let host = wizard
                    .machine
                    .hostname
                    .clone()
                    .unwrap_or_else(|| "this machine".into());
                start_sign_in(&tx, host);
            }
            Some(Request::Recheck) => {
                wizard.github = None;
                check_github(&tx);
            }
            None => {}
        }
    }
    Ok(())
}

pub fn draw(f: &mut Frame, wizard: &Wizard) {
    layouts::default::render(f, wizard, |f, area| match wizard.page {
        Page::SignIn => pages::sign_in::render(f, area, wizard),
        Page::Setup => pages::index::render(f, area, wizard),
    });
    if wizard.mode == Mode::Help {
        help_dialog::render(f, f.area());
    }
}
