use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::backend::{
    clients::system,
    models::{Machine, Os, PackageManager, REPOS, Setup, TOOLS, Tool},
};

pub fn machine() -> Result<Machine> {
    let os = Os::current();
    let mc2_dir = system::home()
        .context("this machine has no home directory")?
        .join("Work")
        .join("mc2");
    let cloned = REPOS
        .iter()
        .filter(|r| mc2_dir.join(r.name).is_dir())
        .map(|r| r.name)
        .collect();
    let installed = TOOLS
        .iter()
        .filter(|t| present(os, t))
        .map(|t| t.name)
        .collect();
    let package_manager = PackageManager::candidates(os)
        .iter()
        .copied()
        .find(|pm| system::on_path(pm.bin()));
    Ok(Machine {
        os,
        os_version: os_version(os),
        arch: std::env::consts::ARCH,
        hostname: hostname(os),
        admin: admin(os),
        desktop: desktop(os),
        package_manager,
        mc2_dir,
        cloned,
        installed,
    })
}

// Only a headless Linux box run as root is taken for a server; anything else is a person's.
pub fn suggested_setup(m: &Machine) -> Setup {
    if m.os == Os::Linux && !m.desktop && m.admin {
        Setup::Server
    } else {
        Setup::Developer
    }
}

fn os_version(os: Os) -> Option<String> {
    match os {
        Os::Mac => system::capture("sw_vers", &["-productVersion"]).map(|v| format!("macOS {v}")),
        Os::Linux => system::read("/etc/os-release")?
            .lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim_matches('"').to_string()),
        Os::Windows => system::capture("cmd", &["/C", "ver"]),
    }
}

fn hostname(os: Os) -> Option<String> {
    match os {
        Os::Windows => system::var("COMPUTERNAME"),
        Os::Linux => system::read("/proc/sys/kernel/hostname")
            .map(|h| h.trim().to_string())
            .or_else(|| system::capture("hostname", &[])),
        Os::Mac => system::capture("hostname", &[]),
    }
}

// `net session` succeeds only in an elevated shell.
fn admin(os: Os) -> bool {
    match os {
        Os::Windows => system::capture("net", &["session"]).is_some(),
        Os::Mac | Os::Linux => system::capture("id", &["-u"]).as_deref() == Some("0"),
    }
}

fn desktop(os: Os) -> bool {
    os != Os::Linux || system::var("DISPLAY").is_some() || system::var("WAYLAND_DISPLAY").is_some()
}

// A GUI app installed by hand puts no command on PATH until it is opened once.
fn present(os: Os, tool: &Tool) -> bool {
    system::on_path(tool.bin) || app_paths(os, tool.name).iter().any(|p| p.exists())
}

fn app_paths(os: Os, name: &str) -> Vec<PathBuf> {
    let program_files = |rel: &str| {
        system::program_files()
            .map(|p| p.join(rel))
            .into_iter()
            .collect()
    };
    match (os, name) {
        (Os::Mac, "tailscale") => vec!["/Applications/Tailscale.app".into()],
        (Os::Mac, "docker") => vec!["/Applications/Docker.app".into()],
        (Os::Windows, "tailscale") => program_files("Tailscale\\tailscale.exe"),
        (Os::Windows, "docker") => program_files("Docker\\Docker\\Docker Desktop.exe"),
        _ => vec![],
    }
}
