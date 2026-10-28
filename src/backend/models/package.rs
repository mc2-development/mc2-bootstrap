use super::Os;

// Where a tool comes from on one OS. Every name was checked against its registry (formulae.brew.sh,
// the Scoop buckets, winget-pkgs, crates.io): a guessed name can install a different program.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    // A formula; `tap` first when it is not in homebrew-core.
    Brew {
        formula: &'static str,
        tap: Option<&'static str>,
    },
    // A macOS app.
    Cask(&'static str),
    // A Scoop app; `bucket` (name, git URL) is added first when it is not in main.
    Scoop {
        app: &'static str,
        bucket: Option<(&'static str, &'static str)>,
    },
    // A Windows app: needs the installer, not a shim.
    Winget(&'static str),
    Cargo(&'static str),
    // rustup.rs on macOS and Linux.
    Rustup,
    // A vendor's install script, piped to sh.
    Script(&'static str),
    // No package does it right; the person is told how.
    Manual(&'static str),
}

const EXTRAS: Option<(&str, &str)> = Some(("extras", "https://github.com/ScoopInstaller/Extras"));

const fn brew(formula: &'static str) -> Source {
    Source::Brew { formula, tap: None }
}

const fn scoop(app: &'static str) -> Source {
    Source::Scoop { app, bucket: None }
}

pub fn source(tool: &'static str, os: Os) -> Option<Source> {
    let windows = os == Os::Windows;
    Some(match tool {
        "rust" if windows => scoop("rustup"),
        "rust" => Source::Rustup,
        // Not in brew's core or Scoop's buckets.
        "cargo-machete" => Source::Cargo("cargo-machete"),
        "cargo-deny" if windows => Source::Cargo("cargo-deny"),
        "tailscale" => match os {
            Os::Mac => Source::Cask("tailscale-app"),
            Os::Linux => Source::Script("https://tailscale.com/install.sh"),
            Os::Windows => Source::Winget("Tailscale.Tailscale"),
        },
        // The local cluster is Docker Desktop's Kubernetes, which no Linux package manager carries.
        "docker" => match os {
            Os::Mac => Source::Cask("docker-desktop"),
            Os::Linux => Source::Manual(
                "Docker Desktop for Linux: https://docs.docker.com/desktop/setup/install/linux/",
            ),
            Os::Windows => Source::Winget("Docker.DockerDesktop"),
        },
        // The command line tools carry it, and Homebrew's installer puts those on first.
        "make" if os == Os::Mac => Source::Manual("xcode-select --install"),
        "infisical" if windows => Source::Scoop {
            app: "infisical",
            bucket: Some((
                "infisical",
                "https://github.com/Infisical/scoop-infisical.git",
            )),
        },
        "infisical" => Source::Brew {
            formula: "infisical/get-cli/infisical",
            tap: Some("infisical/get-cli"),
        },
        "stripe" if windows => Source::Scoop {
            app: "stripe",
            bucket: Some(("stripe", "https://github.com/stripe/scoop-stripe-cli.git")),
        },
        "stripe" => Source::Brew {
            formula: "stripe/stripe-cli/stripe",
            tap: Some("stripe/stripe-cli"),
        },
        "terraform" if windows => scoop("terraform"),
        "terraform" => Source::Brew {
            formula: "hashicorp/tap/terraform",
            tap: Some("hashicorp/tap"),
        },
        "kubectl" if windows => scoop("kubectl"),
        "kubectl" => brew("kubernetes-cli"),
        "node" if windows => scoop("nodejs-lts"),
        "mkcert" if windows => Source::Scoop {
            app: "mkcert",
            bucket: EXTRAS,
        },
        // Firefox's certificate store; Windows trusts through its own.
        "nss" if windows => return None,
        "git" | "gitleaks" | "shellcheck" | "shfmt" | "yamlfmt" | "pnpm" | "yq" | "taplo"
        | "uv" | "watchexec" | "sqruff" | "make" | "gh" | "helm" | "argo" | "argocd" | "hl"
            if windows =>
        {
            scoop(tool)
        }
        "git" | "gitleaks" | "shellcheck" | "shfmt" | "yamlfmt" | "node" | "pnpm" | "mkcert"
        | "nss" | "yq" | "taplo" | "uv" | "cargo-deny" | "watchexec" | "sqruff" | "make" | "gh"
        | "helm" | "argo" | "argocd" | "hl" => brew(tool),
        _ => return None,
    })
}
