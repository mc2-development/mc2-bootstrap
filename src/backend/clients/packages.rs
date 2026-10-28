use super::system;

// Every call runs on this terminal and says only whether it worked; the installer's own output
// is what the person reads when it did not.

pub fn brew_tap(tap: &str) -> bool {
    system::run("brew", &["tap", tap], &[])
}

pub fn brew_install(formulas: &[&str]) -> bool {
    let mut args = vec!["install"];
    args.extend(formulas);
    system::run("brew", &args, &[])
}

pub fn brew_cask(casks: &[&str]) -> bool {
    let mut args = vec!["install", "--cask"];
    args.extend(casks);
    system::run("brew", &args, &[])
}

pub fn scoop_bucket(name: &str, url: &str) -> bool {
    system::run("scoop", &["bucket", "add", name, url], &[])
}

pub fn scoop_install(apps: &[&str]) -> bool {
    let mut args = vec!["install"];
    args.extend(apps);
    system::run("scoop", &args, &[])
}

// `override_args` replaces the installer's own switches: Build Tools needs its workload named.
pub fn winget_install(id: &str, override_args: Option<&str>) -> bool {
    let mut args = vec![
        "install",
        "--id",
        id,
        "--exact",
        "--accept-source-agreements",
        "--accept-package-agreements",
    ];
    if let Some(o) = override_args {
        args.extend(["--override", o]);
    }
    system::run("winget", &args, &[])
}

pub fn cargo_install(krate: &str) -> bool {
    system::run("cargo", &["install", "--locked", krate], &[])
}

pub fn cargo_install_path(path: &str) -> bool {
    system::run(
        "cargo",
        &["install", "--locked", "--quiet", "--path", path],
        &[],
    )
}

// A vendor's script, as its docs give it: fetched over HTTPS and piped to sh.
pub fn script(url: &str) -> bool {
    system::run("sh", &["-c", &format!("curl -fsSL {url} | sh")], &[])
}

// Homebrew's documented one-liner; it asks for the password itself.
pub fn install_homebrew() -> bool {
    system::run(
        "bash",
        &[
            "-c",
            "/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"",
        ],
        &[],
    )
}

// Scoop's documented one-liner; it refuses an elevated shell on purpose.
pub fn install_scoop() -> bool {
    system::run(
        "powershell",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "RemoteSigned",
            "-Command",
            "irm get.scoop.sh | iex",
        ],
        &[],
    )
}

// rustup.rs, unattended; on Windows its own installer from win.rustup.rs.
pub fn rustup(windows: bool, arch: &str) -> bool {
    if windows {
        let installer = std::env::temp_dir().join("rustup-init.exe");
        let file = installer.display().to_string();
        let url = format!("https://win.rustup.rs/{arch}");
        system::run("curl.exe", &["-fsSL", "-o", &file, &url], &[])
            && system::run(&file, &["-y", "--default-toolchain", "stable"], &[])
    } else {
        system::run(
            "sh",
            &[
                "-c",
                "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
            ],
            &[],
        )
    }
}
