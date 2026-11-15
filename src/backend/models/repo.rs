#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Group {
    Base,
    Frontend,
    Backend,
    // The machine, DNS, GitHub, the shared workflows and this CLI: DevOps and "everything".
    Infra,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Base, Group::Frontend, Group::Backend, Group::Infra];

    pub fn title(self) -> &'static str {
        match self {
            Group::Base => "Base",
            Group::Frontend => "Frontend",
            Group::Backend => "Backend",
            Group::Infra => "Infra",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Group::Base => "every machine: the CLIs and cluster config",
            Group::Frontend => "the three apps and the shared UI",
            Group::Backend => "APIs, gateway, Keycloak, shared libraries",
            Group::Infra => "server, DNS, GitHub, shared CI, this CLI",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Toolchain {
    Node,
    Python,
    Rust,
    // Maven runs inside Docker, so a checkout needs no JDK.
    Java,
    Terraform,
}

#[derive(PartialEq, Debug)]
pub struct Repo {
    pub name: &'static str,
    pub group: Group,
    pub toolchain: Option<Toolchain>,
}

const fn repo(name: &'static str, group: Group, toolchain: Option<Toolchain>) -> Repo {
    Repo {
        name,
        group,
        toolchain,
    }
}

const NODE: Option<Toolchain> = Some(Toolchain::Node);
const PYTHON: Option<Toolchain> = Some(Toolchain::Python);
const RUST: Option<Toolchain> = Some(Toolchain::Rust);
const JAVA: Option<Toolchain> = Some(Toolchain::Java);
const TERRAFORM: Option<Toolchain> = Some(Toolchain::Terraform);

// mc2-rust is not optional for a backend: the Rust services declare it as a path dependency.
pub const REPOS: &[Repo] = &[
    repo("mc2-wrappers", Group::Base, None),
    repo("mc2-k8s", Group::Base, None),
    // The agent's house rules, and each developer's agent memory: every machine runs an agent.
    repo("mc2-skills", Group::Base, None),
    repo("mc2-memory", Group::Base, None),
    repo("mc2-ui", Group::Frontend, NODE),
    repo("mc2-operation-frontend", Group::Frontend, NODE),
    repo("mc2-accounting-frontend", Group::Frontend, NODE),
    repo("mc2-platform-frontend", Group::Frontend, NODE),
    repo("mc2-core", Group::Backend, RUST),
    repo("mc2-python", Group::Backend, PYTHON),
    repo("mc2-rust", Group::Backend, RUST),
    repo("mc2-gateway", Group::Backend, RUST),
    repo("mc2-account-api", Group::Backend, RUST),
    repo("mc2-platform-api", Group::Backend, RUST),
    repo("mc2-mailer-api", Group::Backend, RUST),
    repo("mc2-webhooks", Group::Backend, RUST),
    repo("mc2-payments-api", Group::Backend, RUST),
    repo("mc2-activity-api", Group::Backend, RUST),
    repo("mc2-realtime", Group::Backend, RUST),
    repo("mc2-crons", Group::Backend, RUST),
    repo("mc2-keycloak", Group::Backend, JAVA),
    repo("mc2-operation-api", Group::Backend, PYTHON),
    repo("mc2-accounting-api", Group::Backend, PYTHON),
    repo("mc2-agent-api", Group::Backend, PYTHON),
    repo("mc2-terraform", Group::Infra, TERRAFORM),
    repo("mc2-actions", Group::Infra, None),
    repo("mc2-bootstrap", Group::Infra, RUST),
];

pub fn find(name: &str) -> Option<&'static Repo> {
    REPOS.iter().find(|r| r.name == name)
}
