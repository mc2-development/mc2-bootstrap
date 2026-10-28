use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};

// Folders an installer just created: this process's PATH predates them, and a new terminal
// is not something a setup run can ask for halfway.
static ADDED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub fn add_to_path(dir: PathBuf) {
    if let Ok(mut added) = ADDED.lock()
        && !added.contains(&dir)
    {
        added.push(dir);
    }
}

fn path() -> OsString {
    let mut dirs: Vec<PathBuf> = ADDED.lock().map(|a| a.clone()).unwrap_or_default();
    if let Some(paths) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&paths));
    }
    env::join_paths(dirs).unwrap_or_default()
}

// A PATH lookup rather than `<bin> --version`: certutil has no such flag, and spawning
// twenty programs to draw one screen is slow.
pub fn on_path(bin: &str) -> bool {
    which(bin).is_some()
}

// The file a bare name runs: Windows' Command finds only .exe, and scoop's shims are .cmd.
pub fn which(bin: &str) -> Option<PathBuf> {
    env::split_paths(&path()).find_map(|dir| executable(&dir.join(bin)))
}

#[cfg(windows)]
fn executable(path: &Path) -> Option<PathBuf> {
    let exts = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    exts.split(';').filter(|e| !e.is_empty()).find_map(|ext| {
        let mut file = path.as_os_str().to_owned();
        file.push(ext);
        let file = PathBuf::from(file);
        file.is_file().then_some(file)
    })
}

#[cfg(not(windows))]
fn executable(path: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .then(|| path.to_path_buf())
}

fn command(prog: &str) -> Command {
    let mut cmd = Command::new(which(prog).unwrap_or_else(|| PathBuf::from(prog)));
    cmd.env("PATH", path());
    cmd
}

// On this terminal: installers ask for passwords, and a hidden prompt hangs.
pub fn run(prog: &str, args: &[&str], envs: &[(&str, &str)]) -> bool {
    command(prog)
        .args(args)
        .envs(envs.iter().copied())
        .status()
        .is_ok_and(|s| s.success())
}

// Trimmed stdout of a program that exited zero; `None` for anything else.
pub fn capture(prog: &str, args: &[&str]) -> Option<String> {
    let out = command(prog)
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
    let out = command(prog)
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

pub fn program_files_x86() -> Option<PathBuf> {
    env::var_os("ProgramFiles(x86)").map(PathBuf::from)
}

// Adds `line` unless the file already has it, creating the file and its folder when missing.
pub fn append_line(path: &Path, line: &str) -> std::io::Result<bool> {
    let old = fs::read_to_string(path).unwrap_or_default();
    if old.lines().any(|l| l == line) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let sep = if old.is_empty() || old.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    fs::write(path, format!("{old}{sep}{line}\n"))?;
    Ok(true)
}
