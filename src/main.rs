// Layers, top down: `frontend` (the wizard) -> `backend` (services -> clients, speaking in models).
mod backend;
mod frontend;

use std::{
    io::{IsTerminal, Write},
    process::ExitCode,
};

use anyhow::{Result, bail};
use clap::Parser;

use backend::{
    models::{Check, Machine, REPOS, Role, Setup, github::OWNER, repo},
    services::{detect, github, install, plan::plan, server},
};
use frontend::Selection;

// Its own binary, not `mc2`: it runs before mc2-wrappers exists, and the two would overwrite each other.
/// Set up this machine for mc2. Anything not given as a flag is asked for.
#[derive(Parser)]
#[command(name = "mc2-bootstrap", version)]
struct Cli {
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
    /// Developer: run the plan without asking first.
    #[arg(long)]
    yes: bool,
}

// The run's progress on this terminal, and Enter read from it.
struct Terminal;

impl install::Io for Terminal {
    fn say(&mut self, line: String) {
        println!("{line}");
    }

    fn pause(&mut self, prompt: &str) {
        print!("{prompt} ");
        let _ = std::io::stdout().flush();
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

fn confirmed(question: &str) -> bool {
    print!("{question} [Y/n] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    let _ = std::io::stdin().read_line(&mut answer);
    !matches!(answer.trim(), "n" | "N" | "no")
}

fn main() -> Result<ExitCode> {
    let Cli {
        mode,
        role,
        repos,
        ip,
        yes,
    } = Cli::parse();
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
    // Pressing Start in the wizard is the confirmation; flags ask once, unless --yes.
    let (sel, asked) = match given {
        Some(sel) => (sel, yes),
        None if !terminal => bail!("not a terminal: pass --role or --repos"),
        None => match frontend::run(machine, checks, Selection::new(setup, Role::Everything))? {
            Some(sel) => (sel, true),
            None => return Ok(ExitCode::SUCCESS),
        },
    };
    // The wizard owned what it was handed; detecting again is cheap.
    let machine = detect::machine()?;
    Ok(match sel.setup {
        Setup::Developer => {
            developer(&machine, &sel);
            let go = asked || (terminal && confirmed("\nInstall and clone all of this now?"));
            if go {
                run_setup(&machine, &sel)
            } else {
                println!("\nNothing was installed or cloned. --yes runs it without asking.");
                ExitCode::SUCCESS
            }
        }
        Setup::Server => server_report(&server::checks(&machine, ip, sel.passphrase.as_deref())),
    })
}

fn run_setup(machine: &Machine, sel: &Selection) -> ExitCode {
    let plan = plan(sel.role, sel.repos(machine), machine.os);
    println!();
    match install::run(machine, &plan, &mut Terminal) {
        Ok(by_hand) => {
            println!("\n✓ This machine is set up.");
            if !by_hand.is_empty() {
                println!("\nLeft for you, in this order:");
                for (i, step) in by_hand.iter().enumerate() {
                    println!("  {}. {step}", i + 1);
                }
            }
            println!("\nThen: mc2 virtualize --doctor, and `mc2` for the app.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("\n✗ {e:#}\n\nRunning mc2-bootstrap again picks up where this stopped.");
            ExitCode::FAILURE
        }
    }
}

fn developer(machine: &Machine, sel: &Selection) {
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
    match sel.role.agent_role() {
        Some(role) => println!(
            "\n  The coding agent\n    mc2 agent --role {role}   what it treats as this developer's, read at every start"
        ),
        None => println!(
            "\n  The coding agent\n    mc2 agent --role <frontend|backend|devops|fullstack>   ticked repositories name no role"
        ),
    }
    println!(
        "    mc2 agent --memory-link {}   its memory kept in mc2-memory, under this name",
        gh.ssh_user.as_deref().unwrap_or("<your-github-login>")
    );
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
