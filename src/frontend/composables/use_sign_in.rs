use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::{
    backend::models::SignIn,
    frontend::{
        stores::wizard::Wizard,
        types::{Mode, Outcome, Page, Request},
    },
};

impl Wizard {
    pub fn signing_in(&self) -> bool {
        matches!(
            self.sign_in,
            Some(SignIn::Starting | SignIn::Code { .. } | SignIn::Step(_))
        )
    }

    pub fn sign_in_key(&mut self, key: KeyEvent) {
        self.status.clear();
        match key.code {
            KeyCode::Enter | KeyCode::Char('c') => self.continue_to_setup(),
            KeyCode::Char('s') if self.signing_in() => self.status = "already signing in".into(),
            KeyCode::Char('s') => self.request = Some(Request::SignIn),
            KeyCode::Char('r') => self.request = Some(Request::Recheck),
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('q') | KeyCode::Esc => self.outcome = Outcome::Cancel,
            _ => {}
        }
    }

    fn continue_to_setup(&mut self) {
        match &self.github {
            None => self.status = "still checking GitHub".into(),
            Some(gh) if !gh.ready() => self.status = "sign in first: press s".into(),
            Some(_) => self.page = Page::Setup,
        }
    }
}
