use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

// A PATH lookup rather than `<bin> --version`: certutil has no such flag, and spawning
// twenty programs to draw one screen is slow.
pub fn on_path(bin: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|dir| executable(&dir.join(bin))))
}

#[cfg(windows)]
fn executable(path: &Path) -> bool {
    let exts = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    exts.split(';').filter(|e| !e.is_empty()).any(|ext| {
        let mut file = path.as_os_str().to_owned();
        file.push(ext);
        Path::new(&file).is_file()
    })
}

#[cfg(not(windows))]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

// Trimmed stdout of a program that exited zero; `None` for anything else.
pub fn capture(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// Stdout and stderr together, whatever the exit status: `ssh -T` exits 1 even on success.
pub fn output(prog: &str, args: &[&str], envs: &[(&str, &str)]) -> Option<(bool, String)> {
    let out = Command::new(prog)
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), text))
}

// The URL is on screen too, so a headless machine or a failed opener costs nothing.
pub fn open_browser(url: &str) -> bool {
    let (prog, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(windows) {
        ("rundll32", vec!["url.dll,FileProtocolHandler", url])
    } else {
        ("xdg-open", vec![url])
    };
    output(prog, &args, &[]).is_some_and(|(ok, _)| ok)
}

pub fn read(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok()
}

pub fn make_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

pub fn var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.is_empty())
}

pub fn home() -> Option<PathBuf> {
    env::home_dir()
}

pub fn program_files() -> Option<PathBuf> {
    env::var_os("ProgramFiles").map(PathBuf::from)
}
