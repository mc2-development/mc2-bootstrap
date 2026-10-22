use std::path::PathBuf;

use super::{Os, PackageManager, Repo, Tool};

pub struct Machine {
    pub os: Os,
    pub os_version: Option<String>,
    pub arch: &'static str,
    pub hostname: Option<String>,
    pub admin: bool,
    pub desktop: bool,
    pub package_manager: Option<PackageManager>,
    pub mc2_dir: PathBuf,
    pub cloned: Vec<&'static str>,
    pub installed: Vec<&'static str>,
}

impl Machine {
    pub fn has(&self, tool: &Tool) -> bool {
        self.installed.contains(&tool.name)
    }

    pub fn is_cloned(&self, repo: &Repo) -> bool {
        self.cloned.contains(&repo.name)
    }
}
