use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::{detect, github};
use crate::backend::{
    clients::{packages, system},
    models::{Machine, Need, Os, PackageManager, Plan, Role, Source, github::OWNER, package},
};

// How the run talks to the person: a line of progress, and a pause until Enter.
pub trait Io {
    fn say(&mut self, line: String);
    fn pause(&mut self, prompt: &str);
}

// Each step skips what is already done, so running again resumes where a failure stopped it.
// Returns what is left for the person to do by hand.
pub fn run(machine: &Machine, plan: &Plan, io: &mut dyn Io) -> Result<Vec<String>> {
    let os = machine.os;
    let home = system::home().unwrap_or_default();
    let mut by_hand = Vec::new();
    known_paths(os, &home);

    io.say("[1/7] Package manager".into());
    package_manager(machine, plan, io, &mut by_hand)?;

    io.say("[2/7] Tools".into());
    tools(machine, plan, io, &mut by_hand)?;

    io.say("[3/7] GitHub".into());
    let login = github_access(machine, io)?;

    io.say(format!("[4/7] Cloning into {}", machine.mc2_dir.display()));
    clone(machine, plan, io)?;

    io.say("[5/7] mc2".into());
    let mc2 = install_mc2(machine, &home, io)?;

    let cluster = plan.tools.iter().any(|t| t.need == Need::Cluster);
    io.say("[6/7] This machine".into());
    mc2_setup(&mc2, cluster, io);

    io.say("[7/7] The coding agent".into());
    agent(&mc2, machine, plan.role, login.as_deref(), io);

    left_by_hand(os, cluster, &mut by_hand);
    Ok(by_hand)
}

// Where each installer puts its commands, so later steps find them in this same run.
fn known_paths(os: Os, home: &Path) {
    let cargo = system::var("CARGO_HOME").map_or_else(|| home.join(".cargo"), PathBuf::from);
    system::add_to_path(cargo.join("bin"));
    match os {
        Os::Mac => {
            system::add_to_path("/opt/homebrew/bin".into());
            system::add_to_path("/usr/local/bin".into());
        }
        Os::Linux => system::add_to_path("/home/linuxbrew/.linuxbrew/bin".into()),
        Os::Windows => {
            let scoop = system::var("SCOOP").map_or_else(|| home.join("scoop"), PathBuf::from);
            system::add_to_path(scoop.join("shims"));
        }
    }
}

fn package_manager(
    machine: &Machine,
    plan: &Plan,
    io: &mut dyn Io,
    by_hand: &mut Vec<String>,
) -> Result<()> {
    match machine.os {
        Os::Mac | Os::Linux if system::on_path("brew") => io.say("  ✓ Homebrew".into()),
        Os::Mac | Os::Linux => {
            if machine.os == Os::Linux {
                linux_prerequisites(machine, io)?;
            }
            io.say("  installing Homebrew (it asks for your password)".into());
            if !packages::install_homebrew() || !system::on_path("brew") {
                bail!("Homebrew did not install: https://brew.sh, then run this again");
            }
            shell_env(machine.os, io);
        }
        Os::Windows => {
            if system::on_path("scoop") {
                io.say("  ✓ Scoop".into());
            } else {
                io.say("  installing Scoop (command-line tools, no admin needed)".into());
                if !packages::install_scoop() || !system::on_path("scoop") {
                    bail!("Scoop did not install: https://scoop.sh, then run this again");
                }
            }
            // Scoop adds buckets with git, so git comes before anything else.
            if !system::on_path("git") && !packages::scoop_install(&["git"]) {
                bail!("git did not install through Scoop: scoop install git");
            }
            let rust = plan.tools.iter().any(|t| t.name == "rust");
            if rust && !msvc_present() {
                io.say(
                    "  installing the Visual Studio C++ build tools (Rust links with them)".into(),
                );
                let workload = "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended";
                if !packages::winget_install(
                    "Microsoft.VisualStudio.2022.BuildTools",
                    Some(workload),
                ) {
                    bail!(
                        "the C++ build tools did not install: winget install Microsoft.VisualStudio.2022.BuildTools"
                    );
                }
            }
            if !developer_mode() {
                by_hand.push(
                    "Developer Mode, so mc2 can link the agent's skills and memory: Settings → System → For developers → Developer Mode"
                        .into(),
                );
            }
        }
    }
    Ok(())
}

// What Homebrew's Linux docs list before its installer runs.
fn linux_prerequisites(machine: &Machine, io: &mut dyn Io) -> Result<()> {
    let (prog, args): (&str, &[&str]) = match machine.package_manager {
        Some(PackageManager::Apt) => (
            "sudo",
            &[
                "apt-get",
                "install",
                "-y",
                "build-essential",
                "procps",
                "curl",
                "file",
                "git",
            ],
        ),
        Some(PackageManager::Dnf) => (
            "sudo",
            &[
                "dnf",
                "install",
                "-y",
                "@development-tools",
                "procps-ng",
                "curl",
                "file",
                "git",
            ],
        ),
        Some(PackageManager::Pacman) => (
            "sudo",
            &[
                "pacman",
                "-S",
                "--needed",
                "--noconfirm",
                "base-devel",
                "procps-ng",
                "curl",
                "file",
                "git",
            ],
        ),
        _ => bail!(
            "no apt, dnf or pacman here for Homebrew's prerequisites: https://docs.brew.sh/Homebrew-on-Linux"
        ),
    };
    io.say("  Homebrew's prerequisites (compilers, curl, git)".into());
    if !system::run(prog, args, &[]) {
        bail!(
            "the prerequisites did not install: {prog} {}",
            args.join(" ")
        );
    }
    Ok(())
}

// A new terminal finds brew only once its profile says where it is.
fn shell_env(os: Os, io: &mut dyn Io) {
    let Some(brew) = system::which("brew") else {
        return;
    };
    let Some(home) = system::home() else { return };
    let profile = home.join(if os == Os::Mac {
        ".zprofile"
    } else {
        ".bashrc"
    });
    let line = format!("eval \"$({} shellenv)\"", brew.display());
    if system::append_line(&profile, &line).unwrap_or(false) {
        io.say(format!("  brew added to {}", profile.display()));
    }
}

fn msvc_present() -> bool {
    let Some(vswhere) = system::program_files_x86()
        .map(|p| p.join("Microsoft Visual Studio\\Installer\\vswhere.exe"))
        .filter(|p| p.is_file())
    else {
        return false;
    };
    system::capture(
        &vswhere.display().to_string(),
        &[
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ],
    )
    .is_some_and(|p| !p.is_empty())
}

fn developer_mode() -> bool {
    system::capture(
        "reg",
        &[
            "query",
            "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock",
            "/v",
            "AllowDevelopmentWithoutDevLicense",
        ],
    )
    .is_some_and(|out| out.contains("0x1"))
}

// Grouped by source, so brew and scoop each run once; Rust first, since cargo installs need it.
fn tools(machine: &Machine, plan: &Plan, io: &mut dyn Io, by_hand: &mut Vec<String>) -> Result<()> {
    let os = machine.os;
    let missing = plan.missing(machine);
    if missing.is_empty() {
        io.say("  ✓ every tool is already installed".into());
        return Ok(());
    }
    let sources: Vec<(&str, Source)> = missing
        .iter()
        .filter_map(|t| package::source(t.name, os).map(|s| (t.name, s)))
        .collect();
    for (name, _) in &sources {
        io.say(format!("  {name}"));
    }
    if sources.iter().any(|(_, s)| *s == Source::Rustup) {
        io.say("  Rust, from rustup.rs".into());
        packages::rustup(os == Os::Windows, machine.arch);
    }
    let mut formulas = Vec::new();
    let mut casks = Vec::new();
    let mut apps = Vec::new();
    for (name, source) in &sources {
        match *source {
            Source::Brew { formula, tap } => {
                if let Some(tap) = tap {
                    packages::brew_tap(tap);
                }
                formulas.push(formula);
            }
            Source::Cask(cask) => casks.push(cask),
            Source::Scoop { app, bucket } => {
                if let Some((bucket, url)) = bucket {
                    packages::scoop_bucket(bucket, url);
                }
                apps.push(app);
            }
            Source::Winget(id) => {
                packages::winget_install(id, None);
            }
            Source::Script(url) => {
                packages::script(url);
            }
            Source::Manual(how) => by_hand.push(format!("{name}: {how}")),
            Source::Cargo(_) | Source::Rustup => {}
        }
    }
    if !formulas.is_empty() {
        packages::brew_install(&formulas);
    }
    if !casks.is_empty() {
        packages::brew_cask(&casks);
    }
    if !apps.is_empty() {
        packages::scoop_install(&apps);
    }
    for (_, source) in &sources {
        if let Source::Cargo(krate) = *source {
            packages::cargo_install(krate);
        }
    }

    // The pass that proves it: whatever did not land is named, with where it comes from.
    let after = detect::machine()?;
    let still: Vec<String> = plan
        .missing(&after)
        .iter()
        .filter(|t| !matches!(package::source(t.name, os), Some(Source::Manual(_))))
        .map(|t| t.name.to_string())
        .collect();
    if !still.is_empty() {
        bail!(
            "still missing: {} (the installer's output above says why); run this again once fixed",
            still.join(", ")
        );
    }
    io.say("  ✓ every tool is installed".into());
    Ok(())
}

// Reaching a private repository, not merely authenticating: a personal key passes that too.
fn github_access(machine: &Machine, io: &mut dyn Io) -> Result<Option<String>> {
    let Some(key) = github::ssh_key() else {
        bail!("this machine has no home directory");
    };
    if !key.is_file() {
        let host = machine.hostname.clone().unwrap_or_else(|| "mc2".into());
        github::create_key(&key, &host)?;
        io.say(format!("  created the SSH key {}", key.display()));
    }
    loop {
        let status = github::status();
        if status.org_access {
            let who = status.ssh_user.clone().unwrap_or_default();
            io.say(format!("  ✓ {OWNER} reachable as {who}"));
            return Ok(status.ssh_user);
        }
        let public = github::public_key(&key).unwrap_or_default();
        match status.ssh_user {
            Some(who) => io.say(format!(
                "  this key signs in as '{who}', which cannot see {OWNER}: that account needs an invitation, or set MC2_SSH_KEY to the right key"
            )),
            None => io.say(format!(
                "  add this key to your GitHub account at https://github.com/settings/ssh/new\n\n  {public}\n"
            )),
        }
        io.pause("  press Enter to check again");
    }
}

fn clone(machine: &Machine, plan: &Plan, io: &mut dyn Io) -> Result<()> {
    system::make_dir(&machine.mc2_dir)?;
    if machine.os == Os::Mac {
        ignore_finder_files();
    }
    let to_clone = plan.to_clone(machine);
    if to_clone.is_empty() {
        io.say("  ✓ every repository is already here".into());
    }
    let ssh = github::ssh_key().map(|k| github::git_ssh_command(&k));
    let envs: Vec<(&str, &str)> = ssh
        .iter()
        .map(|s| ("GIT_SSH_COMMAND", s.as_str()))
        .collect();
    for repo in to_clone {
        let url = format!("git@github.com:{OWNER}/{}.git", repo.name);
        let dest = machine.mc2_dir.join(repo.name).display().to_string();
        io.say(format!("  {}", repo.name));
        if !system::run("git", &["clone", "--quiet", &url, &dest], &envs) {
            bail!("could not clone {}: git's message is above", repo.name);
        }
    }
    Ok(())
}

// Finder writes .DS_Store into every folder it opens; ignored once, in git's global file.
fn ignore_finder_files() {
    let file = system::capture("git", &["config", "--global", "--get", "core.excludesFile"])
        .map(|p| match p.strip_prefix("~/") {
            Some(rest) => system::home().unwrap_or_default().join(rest),
            None => PathBuf::from(p),
        })
        .or_else(|| system::home().map(|h| h.join(".config/git/ignore")));
    if let Some(file) = file {
        let _ = system::append_line(&file, ".DS_Store");
    }
}

fn install_mc2(machine: &Machine, home: &Path, io: &mut dyn Io) -> Result<PathBuf> {
    let wrappers = machine.mc2_dir.join("mc2-wrappers");
    if !wrappers.join("Cargo.toml").is_file() {
        bail!("mc2-wrappers is not cloned, so mc2 cannot be built");
    }
    io.say("  building mc2 (a few minutes the first time)".into());
    if !packages::cargo_install_path(&wrappers.display().to_string()) {
        bail!("mc2 did not build: cargo's message is above");
    }
    let cargo = system::var("CARGO_HOME").map_or_else(|| home.join(".cargo"), PathBuf::from);
    let exe = if machine.os == Os::Windows {
        "mc2.exe"
    } else {
        "mc2"
    };
    io.say("  ✓ mc2 installed; pull mc2-wrappers and run this again to update it".into());
    Ok(cargo.join("bin").join(exe))
}

fn mc2(mc2: &Path, args: &[&str], io: &mut dyn Io) {
    let prog = mc2.display().to_string();
    if !system::run(&prog, args, &[]) {
        io.say(format!(
            "  ! mc2 {} failed: its message is above",
            args.join(" ")
        ));
    }
}

// A cluster machine's --setup needs the tailnet and Infisical first, so it is left for later.
fn mc2_setup(bin: &Path, cluster: bool, io: &mut dyn Io) {
    mc2(bin, &["virtualize", "--install-shell"], io);
    if cluster {
        mc2(bin, &["virtualize", "--fwd", "install"], io);
    } else {
        mc2(bin, &["virtualize", "--setup"], io);
    }
}

fn agent(bin: &Path, machine: &Machine, role: Role, login: Option<&str>, io: &mut dyn Io) {
    if let Some(role) = role.agent_role() {
        mc2(bin, &["agent", "--role", role], io);
    } else {
        io.say("  no role for ticked repositories: mc2 agent --role <frontend|backend|devops|fullstack>".into());
    }
    if let Some(login) = login
        && machine.mc2_dir.join("mc2-memory").is_dir()
    {
        mc2(bin, &["agent", "--memory-link", login], io);
    }
    if machine.mc2_dir.join("mc2-skills").is_dir() {
        mc2(bin, &["agent", "--skills"], io);
    }
}

// What no program can do for the person: a login, a switch in an app's settings.
fn left_by_hand(os: Os, cluster: bool, by_hand: &mut Vec<String>) {
    if !system::output("tailscale", &["status"], &[]).is_some_and(|(ok, _)| ok) {
        by_hand.push(
            "join the tailnet: open Tailscale → use a custom coordination server → https://headscale.mc2-dev.com:8443, then send the /register/<id> to an admin (mc2 ops --tailnet register <id> <user>)"
                .into(),
        );
    }
    if cluster {
        by_hand.push("sign in to Infisical once on the tailnet: mc2 virtualize --login".into());
        if system::capture("kubectl", &["config", "get-contexts", "docker-desktop"]).is_none() {
            let wsl = if os == Os::Windows {
                " (Docker Desktop needs WSL 2)"
            } else {
                ""
            };
            by_hand.push(format!(
                "turn on Kubernetes: Docker Desktop → Settings → Kubernetes → Enable{wsl}"
            ));
        }
        by_hand.push(
            "then bring the local cluster up: mc2 ops --install, mc2 ops --secrets, mc2 ops --reboot, mc2 virtualize --setup"
                .into(),
        );
    }
}
