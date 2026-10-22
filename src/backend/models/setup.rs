use std::fmt;

use clap::ValueEnum;

#[derive(Clone, Copy, PartialEq, Debug, ValueEnum)]
pub enum Setup {
    Developer,
    Server,
}

impl Setup {
    pub const ALL: [Setup; 2] = [Setup::Developer, Setup::Server];

    pub fn about(self) -> &'static str {
        match self {
            Setup::Developer => "tools, repositories and CLIs to work on mc2",
            Setup::Server => "k3s on a Linux host, joined to the tailnet",
        }
    }
}

impl fmt::Display for Setup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Setup::Developer => "Developer machine",
            Setup::Server => "Server (k3s host)",
        })
    }
}
