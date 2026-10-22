use crate::{
    backend::models::Setup,
    frontend::{
        stores::wizard::Wizard,
        types::{Outcome, Page},
    },
};

impl Wizard {
    pub fn try_start(&mut self) {
        if self.sel.setup == Setup::Developer && !self.signed_in() {
            self.page = Page::SignIn;
            self.status = "sign in to GitHub first: nothing can be cloned without it".into();
            return;
        }
        let blocked = match self.sel.setup {
            Setup::Developer if self.plan().repos.is_empty() => {
                Some("tick at least one repository".to_string())
            }
            Setup::Developer => None,
            Setup::Server => {
                let failing: Vec<&str> = self.failing().iter().map(|c| c.label).collect();
                (!failing.is_empty()).then(|| format!("fix first: {}", failing.join(", ")))
            }
        };
        match blocked {
            Some(why) => self.status = why,
            None => self.outcome = Outcome::Start,
        }
    }
}
