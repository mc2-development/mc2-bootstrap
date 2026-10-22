use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use ureq::Error;

const API: &str = "https://api.github.com";
const USER_AGENT: &str = concat!("mc2/", env!("CARGO_PKG_VERSION"));

#[derive(Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

pub enum Poll {
    Pending,
    SlowDown,
    Token(String),
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct User {
    login: String,
}

#[derive(Serialize)]
struct NewKey<'a> {
    title: &'a str,
    key: &'a str,
}

pub fn device_code(client_id: &str, scope: &str) -> Result<DeviceCode> {
    ureq::post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .send_form([("client_id", client_id), ("scope", scope)])
        .context("could not reach github.com")?
        .body_mut()
        .read_json()
        .context("GitHub did not hand out a device code")
}

// The token endpoint answers 200 either way; the outcome is in the body.
pub fn poll(client_id: &str, device_code: &str) -> Result<Poll> {
    let reply: TokenReply = ureq::post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .send_form([
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .context("could not reach github.com")?
        .body_mut()
        .read_json()?;
    match (reply.access_token, reply.error.as_deref()) {
        (Some(token), _) => Ok(Poll::Token(token)),
        (None, Some("authorization_pending")) => Ok(Poll::Pending),
        (None, Some("slow_down")) => Ok(Poll::SlowDown),
        (None, Some("access_denied")) => bail!("the sign-in was cancelled in the browser"),
        (None, Some("expired_token")) => bail!("the code expired before it was entered"),
        (None, Some(other)) => bail!(
            "{}",
            reply.error_description.unwrap_or_else(|| other.to_string())
        ),
        (None, None) => bail!("GitHub answered with neither a token nor an error"),
    }
}

pub fn login(token: &str) -> Result<String> {
    let user: User = ureq::get(format!("{API}/user"))
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", USER_AGENT)
        .call()
        .context("could not read the signed-in account")?
        .body_mut()
        .read_json()?;
    Ok(user.login)
}

// 422 is GitHub saying this exact key is already on an account.
pub fn add_ssh_key(token: &str, title: &str, key: &str) -> Result<()> {
    let sent = ureq::post(format!("{API}/user/keys"))
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", USER_AGENT)
        .send_json(NewKey { title, key });
    match sent {
        Ok(_) | Err(Error::StatusCode(422)) => Ok(()),
        Err(e) => Err(e).context("could not add the SSH key to the account"),
    }
}
