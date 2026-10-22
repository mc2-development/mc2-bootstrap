use crate::{
    backend::{
        models::{Machine, REPOS, Repo, Role, Setup},
        services::plan::preset,
    },
    frontend::utils::toggle,
};

#[derive(Clone)]
pub struct Selection {
    pub setup: Setup,
    pub role: Role,
    pub ticked: Vec<&'static str>,
    // The server passphrase as typed into the lock, so the run need not ask again.
    pub passphrase: Option<String>,
}

impl Selection {
    pub fn new(setup: Setup, role: Role) -> Self {
        Self {
            setup,
            role,
            ticked: names(&preset(role)),
            passphrase: None,
        }
    }

    pub fn custom(ticked: Vec<&'static str>) -> Self {
        Self {
            setup: Setup::Developer,
            role: Role::Custom,
            ticked,
            passphrase: None,
        }
    }

    // A preset resets the ticks; choosing repositories starts from whatever was ticked.
    pub fn choose(&mut self, role: Role) {
        if role != Role::Custom {
            self.ticked = names(&preset(role));
        }
        self.role = role;
    }

    pub fn toggle(&mut self, repo: &'static Repo) {
        self.role = Role::Custom;
        toggle(&mut self.ticked, repo.name);
    }

    // A preset is exactly its repositories; choosing them keeps the cloned ones locked on.
    pub fn repos(&self, machine: &Machine) -> Vec<&'static Repo> {
        REPOS
            .iter()
            .filter(|r| {
                self.ticked.contains(&r.name) || (self.role == Role::Custom && machine.is_cloned(r))
            })
            .collect()
    }
}

fn names(repos: &[&'static Repo]) -> Vec<&'static str> {
    repos.iter().map(|r| r.name).collect()
}
