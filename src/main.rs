// Layers, top down: `frontend` (the wizard) -> `backend` (services -> clients, speaking in models).
mod backend;
mod frontend;

use std::{io::IsTerminal, process::ExitCode};

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

use backend::{
    models::{Check, Machine, REPOS, Role, Setup, github::OWNER, repo},
    services::{detect, github, plan::plan, server},
};
use frontend::Selection;

#[derive(Parser)]
#[command(version, about = "The mc2 platform, from one command")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Set up this machine for mc2. Anything not given as a flag is asked for.
    Bootstrap {
        /// What this machine is. Default: detected (a headless Linux box run as root is a server).
        #[arg(long, value_enum)]
        mode: Option<Setup>,
        /// Developer: a preset, the repositories and the tools they need.
        #[arg(long, value_enum, conflicts_with = "repos")]
        role: Option<Role>,
        /// Developer: the repositories to clone instead of a preset, comma-separated.
        #[arg(long, value_delimiter = ',')]
        repos: Option<Vec<String>>,
        /// Server: the public IP, when the detected one is wrong.
        #[arg(long)]
        ip: Option<String>,
    },
}

fn main() -> Result<ExitCode> {
    let Cmd::Bootstrap {
        mode,
        role,
        repos,
        ip,
    } = Cli::parse().cmd;
    let machine = detect::machine()?;
    let checks = server::checks(&machine, ip.clone(), None);
    let terminal = std::io::stdout().is_terminal();

    let setup = mode.unwrap_or_else(|| detect::suggested_setup(&machine));
    if setup == Setup::Server && (role.is_some() || repos.is_some()) {
        bail!("--role and --repos are for the developer setup; a server clones nothing");
    }
    let given = match (setup, role, repos) {
        (Setup::Developer, Some(role), _) => Some(Selection::new(setup, role)),
        (Setup::Developer, None, Some(names)) => Some(Selection::custom(known_repos(&names)?)),
        // Asked for by name, or unattended: the Terraform hook has no terminal.
        (Setup::Server, ..) if mode.is_some() || !terminal => {
            Some(Selection::new(setup, Role::Everything))
        }
        _ => None,
    };
    let sel = match given {
        Some(sel) => sel,
        None if !terminal => bail!("not a terminal: pass --role or --repos"),
        None => match frontend::run(machine, checks, Selection::new(setup, Role::Everything))? {
            Some(sel) => sel,
            None => return Ok(ExitCode::SUCCESS),
        },
    };
    // The wizard owned what it was handed; detecting again is cheap.
    let machine = detect::machine()?;
    Ok(match sel.setup {
        Setup::Developer => developer(&machine, &sel),
        Setup::Server => server_report(&server::checks(&machine, ip, sel.passphrase.as_deref())),
    })
}

fn developer(machine: &Machine, sel: &Selection) -> ExitCode {
    let plan = plan(sel.role, sel.repos(machine), machine.os);
    println!("{} on {}\n", plan.role, machine.os);
    let missing = plan.missing(machine);
    if missing.is_empty() {
        println!("  ✓ every tool is already installed");
    } else {
        println!("  This machine needs:");
        for t in missing {
            println!("    {:<14} {:<10} {}", t.name, t.need.label(), t.why);
        }
    }
    let gh = github::status();
    println!("\n  GitHub");
    println!(
        "    SSH key        {}",
        gh.key
            .as_deref()
            .map_or_else(|| "none".into(), |k| k.display().to_string())
    );
    println!(
        "    signed in as   {}",
        gh.ssh_user.as_deref().unwrap_or("nobody")
    );
    println!(
        "    {OWNER}  {}",
        if gh.org_access {
            "reachable"
        } else {
            "not reachable"
        }
    );
    let to_clone = plan.to_clone(machine);
    println!(
        "\n  {} repositories to clone into {}",
        to_clone.len(),
        machine.mc2_dir.display()
    );
    for r in to_clone {
        println!("    {:<26} {}", r.name, r.group.title());
    }
    println!("\nNothing was installed or cloned: running the setup is the next step to build.");
    ExitCode::SUCCESS
}

// A refusal is what someone reads the provider's console for, so every check is printed.
fn server_report(checks: &[Check]) -> ExitCode {
    println!("Server (k3s host)\n");
    for c in checks {
        println!(
            "  {} {:<14} {}",
            if c.ok { "✓" } else { "✗" },
            c.label,
            c.value
        );
        if !c.ok {
            println!("      {}", c.fix);
        }
    }
    if checks.iter().any(|c| !c.ok) {
        println!("\nRefusing: fix the checks above, then re-run.");
        return ExitCode::FAILURE;
    }
    println!("\nEvery check passes. Installing k3s is the next step to build.");
    ExitCode::SUCCESS
}

fn known_repos(names: &[String]) -> Result<Vec<&'static str>> {
    names
        .iter()
        .map(|n| match repo::find(n) {
            Some(r) => Ok(r.name),
            None => {
                let known: Vec<&str> = REPOS.iter().map(|r| r.name).collect();
                bail!("unknown repository `{n}`; known: {}", known.join(", "))
            }
        })
        .collect()
}
