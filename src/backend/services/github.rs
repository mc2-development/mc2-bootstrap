use std::{
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};

use crate::backend::{
    clients::{
        github_api::{self, Poll},
        system,
    },
    models::{GitHub, SignIn, github::OWNER},
};

// The mc2 OAuth App on the mc2-development org, with device flow on. A client ID is public
// by design and the device flow takes no secret, so it can live in this public repo.
const CLIENT_ID: Option<&str> = None;

// Only enough to put this machine's key on the account; the token is dropped afterwards.
const SCOPE: &str = "write:public_key";

// The default name, so ssh finds it with no ~/.ssh/config. A machine with a work and a
// personal account has one key each: MC2_SSH_KEY=~/.ssh/id_work mc2-bootstrap
pub fn ssh_key() -> Option<PathBuf> {
    let home = system::home()?;
    Some(match system::var("MC2_SSH_KEY") {
        Some(k) => match k.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(k),
        },
        None => home.join(".ssh").join("id_ed25519"),
    })
}

// BatchMode: a missing or wrong key answers no instead of prompting into a hidden terminal.
fn ssh_options(key: &Path) -> Vec<String> {
    [
        "-i",
        &key.display().to_string(),
        "-o",
        "IdentitiesOnly=yes",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        "ConnectTimeout=5",
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}

pub fn status() -> GitHub {
    let key = ssh_key().filter(|k| k.is_file());
    // Two independent round trips to GitHub, so side by side rather than one after another.
    let (ssh_user, org_access) = thread::scope(|s| {
        let ssh = s.spawn(|| key.as_deref().and_then(ssh_user));
        let org = s.spawn(|| key.as_deref().is_some_and(reaches_org));
        (
            ssh.join().unwrap_or_default(),
            org.join().unwrap_or_default(),
        )
    });
    GitHub {
        key,
        ssh_user,
        org_access,
    }
}

fn ssh_user(key: &Path) -> Option<String> {
    let mut args = vec!["-T".to_string()];
    args.extend(ssh_options(key));
    args.push("git@github.com".into());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (_, text) = system::output("ssh", &args, &[])?;
    let rest = text.split("Hi ").nth(1)?;
    rest.split('!').next().map(str::to_string)
}

// What git runs as ssh, so a clone uses this key and never prompts.
pub fn git_ssh_command(key: &Path) -> String {
    format!(
        "ssh {}",
        ssh_options(key)
            .iter()
            .map(|o| format!("\"{o}\""))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

pub fn public_key(key: &Path) -> Option<String> {
    let mut path = key.as_os_str().to_owned();
    path.push(".pub");
    system::read(&path).map(|k| k.trim().to_string())
}

fn reaches_org(key: &Path) -> bool {
    let ssh = git_ssh_command(key);
    let url = format!("git@github.com:{OWNER}/mc2-wrappers.git");
    system::output(
        "git",
        &["ls-remote", &url, "HEAD"],
        &[("GIT_SSH_COMMAND", &ssh), ("GIT_TERMINAL_PROMPT", "0")],
    )
    .is_some_and(|(ok, _)| ok)
}

// GitHub's device flow: a code entered in the browser, then this machine's key on the account.
pub fn sign_in(hostname: &str, report: impl Fn(SignIn)) {
    report(SignIn::Starting);
    match run_sign_in(hostname, &report) {
        Ok(login) => report(SignIn::Done(login)),
        Err(e) => report(SignIn::Failed(format!("{e:#}"))),
    }
}

fn run_sign_in(hostname: &str, report: &impl Fn(SignIn)) -> Result<String> {
    let Some(client_id) = CLIENT_ID else {
        bail!("the mc2 OAuth App is not registered yet, so there is nothing to sign in with");
    };
    let code = github_api::device_code(client_id, SCOPE)?;
    report(SignIn::Code {
        user_code: code.user_code.clone(),
        uri: code.verification_uri.clone(),
    });
    system::open_browser(&code.verification_uri);

    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval.max(1));
    let token = loop {
        thread::sleep(interval);
        if Instant::now() > deadline {
            bail!("the code expired before it was entered");
        }
        match github_api::poll(client_id, &code.device_code)? {
            Poll::Token(token) => break token,
            Poll::Pending => {}
            Poll::SlowDown => interval += Duration::from_secs(5),
        }
    };

    let login = github_api::login(&token)?;
    let key = ssh_key().context("this machine has no home directory")?;
    if !key.is_file() {
        report(SignIn::Step("creating this machine's SSH key"));
        create_key(&key, &format!("{login}@{hostname}"))?;
    }
    report(SignIn::Step("adding the key to the account"));
    let public = public_key(&key).context("the SSH key has no .pub beside it")?;
    github_api::add_ssh_key(&token, &format!("mc2 · {hostname}"), &public)?;
    Ok(login)
}

pub fn create_key(key: &Path, comment: &str) -> Result<()> {
    if let Some(dir) = key.parent() {
        system::make_dir(dir)?;
    }
    let path = key.display().to_string();
    match system::output(
        "ssh-keygen",
        &["-t", "ed25519", "-N", "", "-C", comment, "-f", &path, "-q"],
        &[],
    ) {
        Some((true, _)) => Ok(()),
        Some((false, text)) => bail!("ssh-keygen failed: {}", text.trim()),
        None => bail!("ssh-keygen is not installed"),
    }
}
