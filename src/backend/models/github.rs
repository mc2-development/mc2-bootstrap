use std::path::PathBuf;

pub const OWNER: &str = "mc2-development";

// Every valid key passes "successfully authenticated", even a personal account's that
// cannot see the org, so reaching a private repo is what counts.
pub struct GitHub {
    pub key: Option<PathBuf>,
    pub ssh_user: Option<String>,
    pub org_access: bool,
}

impl GitHub {
    pub fn ready(&self) -> bool {
        self.org_access
    }
}

// Progress of the device sign-in, reported from its thread to the screen.
#[derive(Clone, PartialEq, Debug)]
pub enum SignIn {
    Starting,
    Code { user_code: String, uri: String },
    Step(&'static str),
    Done(String),
    Failed(String),
}
