use super::Os;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Need {
    Everyone,
    Node,
    // A frontend-only machine runs against the deployed dev tier and reads its settings from mc2-k8s.
    NodeWithoutCluster,
    // Cargo.toml and pyproject.toml; a frontend checkout has no TOML at all.
    Toml,
    Python,
    Rust,
    Java,
    Terraform,
    Cluster,
}

impl Need {
    pub fn label(self) -> &'static str {
        match self {
            Need::Everyone => "everyone",
            Need::Node | Need::NodeWithoutCluster => "frontend",
            Need::Toml => "backend",
            Need::Python => "python",
            Need::Rust => "rust",
            Need::Java => "keycloak",
            Need::Terraform => "infra",
            Need::Cluster => "cluster",
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct Tool {
    pub name: &'static str,
    pub bin: &'static str,
    pub why: &'static str,
    pub need: Need,
    pub skip_on: &'static [Os],
}

const fn tool(name: &'static str, why: &'static str, need: Need) -> Tool {
    Tool {
        name,
        bin: name,
        why,
        need,
        skip_on: &[],
    }
}

pub const TOOLS: &[Tool] = &[
    tool(
        "git",
        "clones the repositories and runs the hooks",
        Need::Everyone,
    ),
    tool(
        "tailscale",
        "the dev tier answers only on the tailnet",
        Need::Everyone,
    ),
    tool(
        "gitleaks",
        "pre-commit: refuses a commit that stages a credential",
        Need::Everyone,
    ),
    tool(
        "shellcheck",
        "pre-push: lints the shell scripts",
        Need::Everyone,
    ),
    tool(
        "shfmt",
        "pre-commit: formats the shell scripts",
        Need::Everyone,
    ),
    tool("yamlfmt", "pre-commit: formats YAML", Need::Everyone),
    tool("node", "runs the Nuxt apps", Need::Node),
    tool("pnpm", "installs and runs every JS repo", Need::Node),
    tool(
        "mkcert",
        "a trusted certificate for a frontend on the dev tier",
        Need::Node,
    ),
    // The package ships no `nss` command; certutil is what mkcert uses from it.
    Tool {
        bin: "certutil",
        skip_on: &[Os::Windows],
        ..tool("nss", "lets Firefox trust that certificate", Need::Node)
    },
    tool(
        "kubectl",
        "reads a frontend's tier settings from mc2-k8s",
        Need::NodeWithoutCluster,
    ),
    tool(
        "yq",
        "reads a frontend's tier settings from mc2-k8s",
        Need::NodeWithoutCluster,
    ),
    tool(
        "taplo",
        "pre-commit: formats Cargo.toml and pyproject.toml",
        Need::Toml,
    ),
    tool(
        "uv",
        "the Python toolchain; installs the interpreter each repo asks for",
        Need::Python,
    ),
    Tool {
        bin: "cargo",
        ..tool(
            "rust",
            "the Rust toolchain, from rustup.rs; each repo pins its version",
            Need::Rust,
        )
    },
    tool(
        "cargo-deny",
        "pre-push: the advisory and licence check CI fails on",
        Need::Rust,
    ),
    tool(
        "cargo-machete",
        "pre-push: a dependency declared and never used",
        Need::Rust,
    ),
    tool("watchexec", "hot reload for the Rust services", Need::Rust),
    tool(
        "make",
        "builds the Keycloak theme, providers and image",
        Need::Java,
    ),
    tool(
        "terraform",
        "plans and applies the server, GitHub and Infisical roots",
        Need::Terraform,
    ),
    tool(
        "gh",
        "the GitHub token the github root applies with",
        Need::Terraform,
    ),
    tool(
        "infisical",
        "the credentials each root applies with",
        Need::Terraform,
    ),
    tool("docker", "runs the local cluster", Need::Cluster),
    tool("kubectl", "talks to the cluster", Need::Cluster),
    tool("helm", "installs Traefik locally", Need::Cluster),
    tool(
        "yq",
        "renders each service's settings from mc2-k8s",
        Need::Cluster,
    ),
    tool(
        "infisical",
        "hands each service its credentials at start",
        Need::Cluster,
    ),
    tool("argo", "reads the build pipeline", Need::Cluster),
    tool("argocd", "reads what is deployed", Need::Cluster),
    tool(
        "hl",
        "renders the services' JSON logs readably",
        Need::Cluster,
    ),
    tool(
        "stripe",
        "forwards Stripe's test events to mc2-webhooks on this machine",
        Need::Cluster,
    ),
];
