use std::cell::Cell;

use super::selection::Selection;
use crate::{
    backend::{
        models::{Check, GitHub, Machine, Plan, Setup, SignIn},
        services::plan::plan,
    },
    frontend::{
        composables::use_menu::{menu_rows, selectable},
        types::{Focus, Mode, Outcome, Page, Request, Row, Tab},
    },
};

// Everything the wizard remembers; the logic that changes it lives in `composables`.
pub struct Wizard {
    pub machine: Machine,
    pub checks: Vec<Check>,
    pub page: Page,
    // `None` while the background check is still running.
    pub github: Option<GitHub>,
    // The device sign-in, while one is running or after it failed.
    pub sign_in: Option<SignIn>,
    pub request: Option<Request>,
    // Animation frames since the wizard opened; the event loop advances it.
    pub tick: u64,
    pub sel: Selection,
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub tab: Tab,
    pub scroll: u16,
    pub status: String,
    pub outcome: Outcome,
    pub focus: Focus,
    pub mode: Mode,
    // Digits typed before a motion: `5j`.
    pub count: String,
    // A half-typed key sequence: the first `g` of `gg`.
    pub seq: String,
    pub input: String,
    pub last_search: Option<String>,
    // Set while drawing: the preview cannot scroll past its last line.
    pub max_scroll: Cell<u16>,
}

impl Wizard {
    pub fn new(machine: Machine, checks: Vec<Check>, sel: Selection) -> Self {
        let rows = menu_rows(sel.setup, checks.len());
        let cursor = rows.iter().position(selectable).unwrap_or(0);
        // A server clones nothing, so only a developer machine signs in first.
        let page = if sel.setup == Setup::Developer {
            Page::SignIn
        } else {
            Page::Setup
        };
        let mut wizard = Self {
            machine,
            checks,
            page,
            github: None,
            sign_in: None,
            request: None,
            tick: 0,
            tab: Tab::for_setup(sel.setup)[0],
            sel,
            rows,
            cursor,
            scroll: 0,
            status: String::new(),
            outcome: Outcome::Running,
            focus: Focus::Menu,
            mode: Mode::Normal,
            count: String::new(),
            seq: String::new(),
            input: String::new(),
            last_search: None,
            max_scroll: Cell::new(0),
        };
        // A machine detected as a server still opens locked.
        if wizard.sel.setup == Setup::Server && !wizard.server_unlocked() {
            wizard.ask_for_passphrase();
        }
        wizard
    }

    pub fn plan(&self) -> Plan {
        plan(
            self.sel.role,
            self.sel.repos(&self.machine),
            self.machine.os,
        )
    }

    pub fn failing(&self) -> Vec<&Check> {
        self.checks.iter().filter(|c| !c.ok).collect()
    }

    pub fn signed_in(&self) -> bool {
        self.github.as_ref().is_some_and(GitHub::ready)
    }
}
