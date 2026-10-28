use std::fmt;

use clap::ValueEnum;

#[derive(Clone, Copy, PartialEq, Debug, ValueEnum)]
pub enum Role {
    Frontend,
    Backend,
    Devops,
    Everything,
    // Given as --repos, never as --role.
    #[value(skip)]
    Custom,
}

impl Role {
    pub const ALL: [Role; 5] = [
        Role::Frontend,
        Role::Backend,
        Role::Devops,
        Role::Everything,
        Role::Custom,
    ];

    pub fn about(self) -> &'static str {
        match self {
            Role::Frontend => "the three apps and the shared UI layer. No backend, no database",
            Role::Backend => "the APIs, the gateway, migrations and the shared libraries",
            Role::Devops => "the cluster, the server, CI and this CLI. No apps",
            Role::Everything => "the whole platform",
            Role::Custom => "whatever is ticked; the tools follow the ticks",
        }
    }

    // What `mc2 agent --role` records for the coding agent; ticked repositories name no role.
    pub fn agent_role(self) -> Option<&'static str> {
        match self {
            Role::Frontend => Some("frontend"),
            Role::Backend => Some("backend"),
            Role::Devops => Some("devops"),
            Role::Everything => Some("fullstack"),
            Role::Custom => None,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Role::Frontend => "Frontend",
            Role::Backend => "Backend",
            Role::Devops => "DevOps",
            Role::Everything => "Everything",
            Role::Custom => "Choose repositories",
        })
    }
}
