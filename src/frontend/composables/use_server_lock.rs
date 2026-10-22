use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::{
    backend::{models::Setup, services::server},
    frontend::{
        stores::wizard::Wizard,
        types::{Mode, Page},
    },
};

// An accident guard: the server setup turns this machine into a k3s node.
impl Wizard {
    pub fn server_unlocked(&self) -> bool {
        server::passphrase(self.sel.passphrase.as_deref())
            .is_some_and(|p| server::passphrase_matches(&p))
    }

    pub fn ask_for_passphrase(&mut self) {
        self.mode = Mode::Password;
        self.input.clear();
    }

    pub fn password_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input.clear();
                self.stay_locked("the server setup stays locked");
            }
            KeyCode::Enter => {
                let typed = std::mem::take(&mut self.input);
                if server::passphrase_matches(&typed) {
                    self.mode = Mode::Normal;
                    self.unlock(typed);
                } else {
                    self.stay_locked("wrong passphrase: the server setup stays locked");
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    // Back to the developer setup, which signs in first like any other start.
    fn stay_locked(&mut self, why: &str) {
        self.mode = Mode::Normal;
        if self.sel.setup == Setup::Server {
            self.choose_setup(Setup::Developer);
            if !self.signed_in() {
                self.page = Page::SignIn;
            }
        }
        self.status = why.into();
    }

    fn unlock(&mut self, passphrase: String) {
        let check = server::passphrase_check(Some(&passphrase));
        if let Some(slot) = self.checks.iter_mut().find(|c| c.label == check.label) {
            *slot = check;
        }
        self.sel.passphrase = Some(passphrase);
        self.choose_setup(Setup::Server);
    }
}
