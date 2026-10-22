use crate::backend::models::{Group, Need, Os, Plan, REPOS, Repo, Role, TOOLS, Tool, Toolchain};

pub fn preset(role: Role) -> Vec<&'static Repo> {
    let groups: &[Group] = match role {
        Role::Frontend => &[Group::Base, Group::Frontend],
        Role::Backend => &[Group::Base, Group::Backend],
        Role::Devops => &[Group::Base, Group::Infra],
        Role::Everything | Role::Custom => {
            &[Group::Base, Group::Frontend, Group::Backend, Group::Infra]
        }
    };
    REPOS.iter().filter(|r| groups.contains(&r.group)).collect()
}

// The tools come from the repositories, not the role, so a custom choice behaves like a preset.
pub fn plan(role: Role, repos: Vec<&'static Repo>, os: Os) -> Plan {
    let uses = |t: Toolchain| repos.iter().any(|r| r.toolchain == Some(t));
    let node = uses(Toolchain::Node);
    let python = uses(Toolchain::Python);
    let rust = uses(Toolchain::Rust);
    let java = uses(Toolchain::Java);
    let terraform = uses(Toolchain::Terraform);
    // Working on the server and CI means operating the cluster too.
    let cluster = python || rust || java || terraform;
    let mut tools: Vec<&'static Tool> = Vec::new();
    for t in TOOLS.iter().filter(|t| !t.skip_on.contains(&os)) {
        let needed = match t.need {
            Need::Everyone => true,
            Need::Node => node,
            Need::NodeWithoutCluster => node && !cluster,
            Need::Toml => python || rust,
            Need::Python => python,
            Need::Rust => rust,
            Need::Java => java,
            Need::Terraform => terraform,
            Need::Cluster => cluster,
        };
        // A tool listed for two reasons is installed once, under the first.
        if needed && !tools.iter().any(|have| have.name == t.name) {
            tools.push(t);
        }
    }
    Plan { role, repos, tools }
}
