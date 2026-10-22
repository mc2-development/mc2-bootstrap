use crate::backend::models::{Repo, Role, Setup};

#[derive(Clone, Copy, PartialEq)]
pub enum Row {
    Header(&'static str),
    Note(&'static str),
    Setup(Setup),
    Role(Role),
    Repo(&'static Repo),
    Check(usize),
    Gap,
    Start,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tab {
    Tools,
    Repos,
    Checks,
    Machine,
}

impl Tab {
    pub fn for_setup(setup: Setup) -> &'static [Tab] {
        match setup {
            Setup::Developer => &[Tab::Tools, Tab::Repos, Tab::Machine],
            Setup::Server => &[Tab::Checks, Tab::Machine],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Tools => "tools",
            Tab::Repos => "repositories",
            Tab::Checks => "checks",
            Tab::Machine => "machine",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Focus {
    Menu,
    Preview,
}

// LazyVim-style modes: normal, `:` command line, `/` search, the keymap help, and the
// hidden prompt that unlocks the server setup.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Normal,
    Command,
    Search,
    Help,
    Password,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Page {
    SignIn,
    Setup,
}

// Work the event loop does outside the key handler: it needs the terminal or a thread.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Request {
    SignIn,
    Recheck,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Running,
    Start,
    Cancel,
}
