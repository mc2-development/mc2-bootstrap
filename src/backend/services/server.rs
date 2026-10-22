use sha2::{Digest, Sha256};

use crate::backend::{
    clients::system,
    models::{Check, Machine, Os},
};

// An accident guard, not a security control. Change it: printf '%s' 'x' | shasum -a 256
const PASSPHRASE_SHA256: &str = "f8c87098a3fdd32415f48701a4d497433f0beaad98654a873b8755d424158ded";

fn check(label: &'static str, value: impl Into<String>, ok: bool, fix: impl Into<String>) -> Check {
    Check {
        label,
        value: value.into(),
        ok,
        fix: fix.into(),
    }
}

pub fn passphrase_matches(passphrase: &str) -> bool {
    sha256(passphrase) == PASSPHRASE_SHA256
}

// Typed into the wizard, or handed over by Terraform and the ssh repair.
pub fn passphrase(typed: Option<&str>) -> Option<String> {
    typed
        .map(str::to_string)
        .or_else(|| system::var("MC2_SERVER_PASSPHRASE"))
}

pub fn passphrase_check(typed: Option<&str>) -> Check {
    match passphrase(typed) {
        None => check(
            "passphrase",
            "not set",
            false,
            "MC2_SERVER_PASSPHRASE: Terraform passes it, and so does the ssh repair",
        ),
        Some(p) => {
            let ok = passphrase_matches(&p);
            check(
                "passphrase",
                if ok { "matches" } else { "does not match" },
                ok,
                "the shared passphrase, from Vaultwarden",
            )
        }
    }
}

// The guards bootstrap.sh --server refused on, in its order. Nothing here changes the machine.
pub fn checks(m: &Machine, ip: Option<String>, typed: Option<&str>) -> Vec<Check> {
    let mut out = vec![check(
        "platform",
        m.os.to_string(),
        m.os == Os::Linux,
        "k3s runs on Linux only; this machine wants the developer setup",
    )];
    if m.os != Os::Linux {
        return out;
    }
    out.push(check(
        "root",
        if m.admin { "yes" } else { "no" },
        m.admin,
        "run as root: k3s installs as a systemd service",
    ));
    out.push(passphrase_check(typed));
    let host = m.hostname.clone().unwrap_or_default();
    let confirmed = system::var("MC2_CONFIRM_HOSTNAME");
    out.push(check(
        "hostname",
        match &confirmed {
            Some(c) if *c == host => format!("{host}, confirmed"),
            Some(c) => format!("{host}, but MC2_CONFIRM_HOSTNAME={c}"),
            None => format!("{host}, not confirmed"),
        },
        confirmed.as_deref() == Some(host.as_str()) && !host.is_empty(),
        format!("MC2_CONFIRM_HOSTNAME={host} names the target in advance"),
    ));
    let ip = ip.or_else(public_ip);
    out.push(check(
        "public IP",
        ip.clone().unwrap_or_else(|| "unknown".into()),
        ip.is_some(),
        "pass it explicitly: --ip <address>",
    ));
    let missing: Vec<&str> = ["curl", "systemctl"]
        .into_iter()
        .filter(|b| !system::on_path(b))
        .collect();
    out.push(check(
        "prerequisites",
        if missing.is_empty() {
            "curl, systemd".to_string()
        } else {
            format!("missing {}", missing.join(", "))
        },
        missing.is_empty(),
        "install curl and run on a systemd distribution",
    ));
    // Without a key `tailscale up` blocks on a browser URL nobody is watching.
    let joined = system::capture("tailscale", &["status"]).is_some();
    let key = system::var("MC2_TS_AUTHKEY").is_some();
    out.push(check(
        "tailnet key",
        if joined {
            "already joined"
        } else if key {
            "set"
        } else {
            "not set"
        },
        joined || key,
        "MC2_TS_AUTHKEY, minted by Terraform or `mc2 --tailnet key server`",
    ));
    // A headscale key offered to tailscale.com is rejected, and the node joins nothing.
    let login = system::var("MC2_TS_LOGIN_SERVER");
    out.push(check(
        "login server",
        login.clone().unwrap_or_else(|| "not set".into()),
        joined || !key || login.is_some(),
        "MC2_TS_LOGIN_SERVER=https://headscale.mc2-dev.com:8443",
    ));
    let ufw_active =
        system::capture("ufw", &["status"]).is_some_and(|s| s.contains("Status: active"));
    let may_disable = system::var("MC2_DISABLE_UFW").is_some();
    out.push(check(
        "ufw",
        match (ufw_active, may_disable) {
            (false, _) => "inactive",
            (true, true) => "active, will be disabled",
            (true, false) => "active",
        },
        !ufw_active || may_disable,
        "MC2_DISABLE_UFW=1: ufw filters flannel and breaks pod networking",
    ));
    out
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// No provider metadata probe: it cost three seconds of timeout on every other host.
fn public_ip() -> Option<String> {
    system::capture("curl", &["-sf", "--max-time", "5", "https://ifconfig.me"])
        .filter(|ip| !ip.is_empty())
        .or_else(|| {
            let route = system::capture("ip", &["-4", "route", "get", "1.1.1.1"])?;
            let mut words = route.split_whitespace();
            words.find(|w| *w == "src")?;
            words.next().map(str::to_string)
        })
}

#[cfg(test)]
mod tests {
    // The FIPS 180-2 vector, and the same hex `shasum -a 256` prints, so the lock opens for
    // exactly the passphrase whose hash is pinned above.
    #[test]
    fn hashes_like_shasum() {
        assert_eq!(
            super::sha256("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(!super::passphrase_matches(""));
    }
}
