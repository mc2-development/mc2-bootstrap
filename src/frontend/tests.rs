use std::path::PathBuf;

use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};

use super::{
    app::draw,
    composables::use_menu::selectable,
    stores::{selection::Selection, wizard::Wizard},
    types::{Focus, Mode, Outcome, Page, Request, Row, Tab},
};
use crate::backend::{
    models::{Check, GitHub, Machine, Os, PackageManager, REPOS, Role, Setup, SignIn, TOOLS},
    services::detect::suggested_setup,
};

fn github(ssh_user: Option<&str>, org_access: bool) -> GitHub {
    GitHub {
        key: ssh_user.map(|_| PathBuf::from("/home/dev/.ssh/id_ed25519")),
        ssh_user: ssh_user.map(str::to_string),
        org_access,
    }
}

fn machine(cloned: Vec<&'static str>, installed: Vec<&'static str>) -> Machine {
    Machine {
        os: Os::Mac,
        os_version: Some("macOS 26.1".into()),
        arch: "aarch64",
        hostname: Some("devbox".into()),
        admin: false,
        desktop: true,
        package_manager: Some(PackageManager::Brew),
        mc2_dir: PathBuf::from("/home/dev/Work/mc2"),
        cloned,
        installed,
    }
}

fn check(label: &'static str, ok: bool) -> Check {
    Check {
        label,
        value: if ok { "fine" } else { "not set" }.into(),
        ok,
        fix: format!("set {label}"),
    }
}

// Signed in and past the sign-in page, which has tests of its own below.
fn wizard(m: Machine, sel: Selection) -> Wizard {
    let mut w = Wizard::new(
        m,
        vec![check("root", true), check("passphrase", false)],
        sel,
    );
    w.github = Some(github(Some("dev"), true));
    w.page = Page::Setup;
    w
}

fn fresh(gh: Option<GitHub>) -> Wizard {
    let mut w = Wizard::new(
        machine(vec![], vec![]),
        vec![],
        Selection::new(Setup::Developer, Role::Frontend),
    );
    w.github = gh;
    w
}

#[test]
fn a_developer_machine_signs_in_first() {
    let mut a = fresh(None);
    assert_eq!(a.page, Page::SignIn);
    assert!(screen(&a).contains("checking…") && screen(&a).contains("github …"));
    keys(&mut a, "\n");
    assert_eq!(a.page, Page::SignIn);
    assert!(a.status.contains("still checking"));
    a.github = Some(github(Some("dev"), true));
    let s = screen(&a);
    assert!(
        s.contains("signed in as") && s.contains("reachable") && s.contains("<Enter> continue")
    );
    keys(&mut a, "\n");
    assert_eq!(a.page, Page::Setup);
}

#[test]
fn the_logo_sweeps_over_the_sign_in() {
    let mut t = Terminal::new(TestBackend::new(140, 70)).unwrap();
    let mut a = fresh(None);
    let mut frame = |tick| {
        a.tick = tick;
        t.draw(|f| draw(f, &a)).unwrap();
        t.backend().buffer().clone()
    };
    let (first, later) = (frame(10), frame(30));
    let text = |b: &ratatui::buffer::Buffer| {
        b.content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>()
    };
    assert!(text(&first).contains("░█████████████"), "the logo is drawn");
    assert_eq!(text(&first), text(&later), "the same letters");
    assert_ne!(first, later, "in other colours: it moves");
}

#[test]
fn a_server_skips_the_sign_in() {
    let a = Wizard::new(
        machine(vec![], vec![]),
        vec![],
        Selection::new(Setup::Server, Role::Everything),
    );
    assert_eq!(a.page, Page::Setup);
}

#[test]
fn a_fresh_machine_is_offered_the_sign_in() {
    let mut a = fresh(Some(github(None, false)));
    let s = screen(&a);
    assert!(s.contains("none yet") && s.contains("creates one and adds it"));
    assert!(s.contains("s sign in with GitHub"));
    keys(&mut a, "\n");
    assert!(a.status.contains("press s"));
    keys(&mut a, "s");
    assert_eq!(a.request, Some(Request::SignIn));
    keys(&mut a, "r");
    assert_eq!(a.request, Some(Request::Recheck));
}

#[test]
fn the_wrong_account_is_named() {
    let mut a = fresh(Some(github(Some("personal"), false)));
    let s = screen(&a);
    assert!(s.contains("'personal' cannot see mc2-development"));
    assert!(s.contains("MC2_SSH_KEY"));
    keys(&mut a, "\n");
    assert_eq!(
        a.page,
        Page::SignIn,
        "a key outside the org is not signed in"
    );
}

#[test]
fn the_device_code_is_shown_while_signing_in() {
    let mut a = fresh(Some(github(None, false)));
    a.sign_in = Some(SignIn::Code {
        user_code: "WDJB-MJHT".into(),
        uri: "https://github.com/login/device".into(),
    });
    let s = screen(&a);
    assert!(s.contains("WDJB-MJHT") && s.contains("github.com/login/device"));
    keys(&mut a, "s");
    assert_eq!(a.request, None, "one sign-in at a time");
    a.sign_in = Some(SignIn::Failed(
        "the code expired before it was entered".into(),
    ));
    assert!(screen(&a).contains("code expired"));
    keys(&mut a, "s");
    assert_eq!(
        a.request,
        Some(Request::SignIn),
        "a failed one can be retried"
    );
}

#[test]
fn starting_signed_out_goes_back_to_the_sign_in() {
    let mut a = app(Role::Frontend);
    a.github = Some(github(None, false));
    keys(&mut a, ":wq\n");
    assert_eq!(a.outcome, Outcome::Running);
    assert_eq!(a.page, Page::SignIn);
}

#[test]
fn github_shows_on_the_machine_tab_and_status_bar() {
    let mut a = app(Role::Frontend);
    assert!(screen(&a).contains("github ✓ dev"));
    keys(&mut a, "LL");
    let s = screen(&a);
    assert!(s.contains("GitHub") && s.contains("signed in as") && s.contains("dev"));
}

fn app(role: Role) -> Wizard {
    wizard(
        machine(vec![], vec!["git"]),
        Selection::new(Setup::Developer, role),
    )
}

fn keys(app: &mut Wizard, text: &str) {
    for ch in text.chars() {
        let key = match ch {
            '\n' => KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            '\x08' => KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            '\x0c' => KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            c => KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
        };
        app.handle_key(key);
    }
}

fn screen(app: &Wizard) -> String {
    let mut t = Terminal::new(TestBackend::new(140, 70)).unwrap();
    t.draw(|f| draw(f, app)).unwrap();
    t.backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn row_of(a: &Wizard, row: Row) -> usize {
    a.rows.iter().position(|r| *r == row).unwrap()
}

fn repo_row(a: &Wizard, name: &str) -> usize {
    a.rows
        .iter()
        .position(|r| matches!(r, Row::Repo(repo) if repo.name == name))
        .unwrap()
}

fn tools(a: &Wizard) -> Vec<&'static str> {
    a.plan().tools.iter().map(|t| t.name).collect()
}

#[test]
fn cursor_skips_headers_notes_and_gaps() {
    let mut a = app(Role::Everything);
    assert!(matches!(a.rows[a.cursor], Row::Setup(Setup::Developer)));
    for _ in 0..60 {
        keys(&mut a, "j");
        assert!(selectable(&a.rows[a.cursor]));
    }
    assert!(matches!(a.rows[a.cursor], Row::Start));
}

#[test]
fn a_frontend_machine_needs_no_cluster() {
    let a = app(Role::Frontend);
    let t = tools(&a);
    assert!(t.contains(&"pnpm") && t.contains(&"kubectl") && t.contains(&"tailscale"));
    assert!(!t.contains(&"docker") && !t.contains(&"helm") && !t.contains(&"rust"));
    assert!(!t.contains(&"taplo"), "a frontend checkout has no TOML");
}

#[test]
fn a_backend_machine_needs_the_cluster_and_both_toolchains() {
    let a = app(Role::Backend);
    let t = tools(&a);
    assert!(t.contains(&"docker") && t.contains(&"uv") && t.contains(&"rust"));
    assert!(!t.contains(&"pnpm"));
    assert_eq!(t.iter().filter(|n| **n == "kubectl").count(), 1);
}

#[test]
fn nss_is_not_needed_on_windows() {
    let mut a = app(Role::Frontend);
    a.machine.os = Os::Windows;
    assert!(!tools(&a).contains(&"nss"));
}

#[test]
fn ticking_a_repository_switches_to_custom_from_the_preset() {
    let mut a = app(Role::Frontend);
    a.cursor = repo_row(&a, "mc2-gateway");
    keys(&mut a, " ");
    assert_eq!(a.sel.role, Role::Custom);
    assert!(
        a.sel.ticked.contains(&"mc2-ui"),
        "starts from the frontend preset"
    );
    assert!(tools(&a).contains(&"rust"), "the tools follow the ticks");
}

#[test]
fn a_preset_resets_the_ticks() {
    let mut a = app(Role::Everything);
    a.cursor = repo_row(&a, "mc2-ui");
    keys(&mut a, " ");
    assert!(!a.sel.ticked.contains(&"mc2-ui"));
    a.cursor = row_of(&a, Row::Role(Role::Everything));
    keys(&mut a, " ");
    assert!(a.sel.ticked.contains(&"mc2-ui"));
}

#[test]
fn infra_comes_with_devops_and_everything() {
    let names = |role| -> Vec<&str> { app(role).plan().repos.iter().map(|r| r.name).collect() };
    assert!(names(Role::Everything).contains(&"mc2-terraform"));
    assert!(names(Role::Devops).contains(&"mc2-terraform"));
    assert!(!names(Role::Backend).contains(&"mc2-terraform"));
    assert!(names(Role::Backend).contains(&"mc2-keycloak"));
    let a = wizard(
        machine(vec![], vec![]),
        Selection::custom(vec!["mc2-terraform", "mc2-actions"]),
    );
    let t = tools(&a);
    assert!(t.contains(&"terraform") && t.contains(&"gh") && t.contains(&"kubectl"));
    assert_eq!(
        app(Role::Everything)
            .plan()
            .tools
            .iter()
            .filter(|t| t.name == "infisical")
            .count(),
        1
    );
}

#[test]
fn devops_is_the_cluster_the_server_and_ci_without_the_apps() {
    let a = app(Role::Devops);
    let names: Vec<&str> = a.plan().repos.iter().map(|r| r.name).collect();
    assert!(
        names.contains(&"mc2-k8s")
            && names.contains(&"mc2-actions")
            && names.contains(&"mc2-bootstrap")
    );
    assert!(!names.contains(&"mc2-ui") && !names.contains(&"mc2-gateway"));
    let t = tools(&a);
    for tool in [
        "terraform",
        "kubectl",
        "helm",
        "argocd",
        "infisical",
        "rust",
    ] {
        assert!(t.contains(&tool), "{tool}");
    }
    assert!(!t.contains(&"pnpm") && !t.contains(&"uv"));
}

#[test]
fn infra_explains_itself() {
    let mut a = app(Role::Everything);
    let s = screen(&a);
    assert!(
        s.contains("Infra") && s.contains("server, DNS, GitHub, shared CI"),
        "menu header and note"
    );
    assert!(
        s.contains("terraform") && s.contains("infra"),
        "the tool is tagged with what brought it"
    );
    keys(&mut a, "L");
    assert_eq!(a.tab, Tab::Repos);
    assert!(screen(&a).contains("Infra · server, DNS"));
}

#[test]
fn placeholders_are_never_in_a_preset() {
    let a = app(Role::Everything);
    assert!(!a.plan().repos.iter().any(|r| r.name == "mc2-realtime"));
}

#[test]
fn a_preset_ignores_cloned_repositories_outside_it() {
    let a = wizard(
        machine(vec!["mc2-gateway"], vec![]),
        Selection::new(Setup::Developer, Role::Frontend),
    );
    assert!(a.plan().repos.iter().all(|r| r.name != "mc2-gateway"));
    assert!(!tools(&a).contains(&"rust") && !tools(&a).contains(&"docker"));
}

#[test]
fn cloned_repositories_stay_on_when_choosing() {
    let mut a = wizard(
        machine(vec!["mc2-gateway"], vec![]),
        Selection::custom(vec!["mc2-ui"]),
    );
    assert!(tools(&a).contains(&"rust"));
    a.cursor = repo_row(&a, "mc2-gateway");
    keys(&mut a, " ");
    assert!(a.status.contains("already cloned"));
    assert!(a.plan().repos.iter().any(|r| r.name == "mc2-gateway"));
    assert!(
        a.plan()
            .to_clone(&a.machine)
            .iter()
            .all(|r| r.name != "mc2-gateway")
    );
}

#[test]
fn start_needs_a_repository() {
    let mut a = wizard(machine(vec![], vec![]), Selection::custom(vec![]));
    keys(&mut a, ":wq\n");
    assert_eq!(a.outcome, Outcome::Running);
    assert!(a.status.contains("repository"));
    let mut b = app(Role::Frontend);
    keys(&mut b, ":wq\n");
    assert_eq!(b.outcome, Outcome::Start);
}

#[test]
fn preview_lists_missing_and_installed_tools() {
    let installed = TOOLS
        .iter()
        .map(|t| t.name)
        .filter(|n| *n != "pnpm")
        .collect();
    let a = wizard(
        machine(vec![], installed),
        Selection::new(Setup::Developer, Role::Frontend),
    );
    let s = screen(&a);
    assert!(s.contains("This machine needs (1)") && s.contains("pnpm"));
    assert!(s.contains("Already installed"));
    assert!(s.contains("1 tools missing"));
}

#[test]
fn repositories_tab_shows_what_gets_cloned() {
    let mut a = wizard(
        machine(vec!["mc2-k8s"], vec![]),
        Selection::new(Setup::Developer, Role::Frontend),
    );
    keys(&mut a, "L");
    let s = screen(&a);
    assert!(s.contains("/home/dev/Work/mc2/"));
    assert!(s.contains("will clone") && s.contains("cloned"));
}

#[test]
fn machine_tab_shows_what_was_detected() {
    let mut a = app(Role::Frontend);
    keys(&mut a, "LL");
    assert_eq!(a.tab, Tab::Machine);
    let s = screen(&a);
    assert!(s.contains("macOS 26.1") && s.contains("aarch64") && s.contains("devbox"));
    assert!(s.contains("Developer machine"), "the suggestion");
    keys(&mut a, "L");
    assert_eq!(a.tab, Tab::Tools, "tabs wrap around");
}

#[test]
fn only_a_headless_root_linux_box_is_a_server() {
    let mut m = machine(vec![], vec![]);
    assert_eq!(suggested_setup(&m), Setup::Developer);
    m.os = Os::Linux;
    m.admin = true;
    assert_eq!(
        suggested_setup(&m),
        Setup::Developer,
        "a desktop session is a person's"
    );
    m.desktop = false;
    assert_eq!(suggested_setup(&m), Setup::Server);
    m.admin = false;
    assert_eq!(suggested_setup(&m), Setup::Developer);
}

#[test]
fn the_server_setup_is_locked_behind_the_passphrase() {
    let mut a = app(Role::Frontend);
    a.cursor = row_of(&a, Row::Setup(Setup::Server));
    keys(&mut a, " ");
    assert_eq!(a.mode, Mode::Password, "asked before anything changes");
    assert_eq!(a.sel.setup, Setup::Developer);
    assert!(
        a.rows.iter().any(|r| matches!(r, Row::Repo(_))),
        "the menu has not switched yet"
    );
    keys(&mut a, "hunter2");
    let s = screen(&a);
    assert!(
        s.contains("•••••••") && !s.contains("hunter2"),
        "the characters never show"
    );
    assert!(s.contains("LOCKED"));
    keys(&mut a, "\n");
    assert_eq!(a.mode, Mode::Normal);
    assert_eq!(a.sel.setup, Setup::Developer);
    assert!(a.status.contains("wrong passphrase"));
    assert!(a.input.is_empty(), "a wrong attempt is not kept");
}

#[test]
fn esc_leaves_the_server_setup_locked() {
    let mut a = app(Role::Frontend);
    a.cursor = row_of(&a, Row::Setup(Setup::Server));
    keys(&mut a, " abc");
    a.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(a.mode, Mode::Normal);
    assert_eq!(a.sel.setup, Setup::Developer);
    assert!(a.input.is_empty());
}

#[test]
fn a_detected_server_opens_locked_and_falls_back_to_developer() {
    let mut a = Wizard::new(
        machine(vec![], vec![]),
        vec![],
        Selection::new(Setup::Server, Role::Everything),
    );
    assert_eq!(a.mode, Mode::Password);
    keys(&mut a, "nope\n");
    assert_eq!(a.sel.setup, Setup::Developer);
    assert_eq!(a.page, Page::SignIn, "a developer machine signs in first");
}

#[test]
fn once_unlocked_the_menu_swaps_for_the_checks() {
    // Only the hash is in the code, so this goes past the lock the way a right passphrase would.
    let mut a = app(Role::Frontend);
    a.choose_setup(Setup::Server);
    assert_eq!(a.sel.setup, Setup::Server);
    assert!(
        a.rows
            .iter()
            .all(|r| !matches!(r, Row::Repo(_) | Row::Role(_)))
    );
    assert!(
        matches!(a.rows[a.cursor], Row::Setup(Setup::Server)),
        "the cursor stays on it"
    );
    assert_eq!(a.tab, Tab::Checks);
    let s = screen(&a);
    assert!(
        s.contains("Install k3s") && s.contains("set passphrase") && s.contains("1 checks failing")
    );
    assert!(s.contains("Linux only"), "this machine is a Mac");
}

#[test]
fn a_failing_check_blocks_the_install() {
    let mut a = wizard(
        machine(vec![], vec![]),
        Selection::new(Setup::Server, Role::Everything),
    );
    a.mode = Mode::Normal; // past the lock
    keys(&mut a, ":wq\n");
    assert_eq!(a.outcome, Outcome::Running);
    assert_eq!(a.status, "fix first: passphrase");
    a.checks[1].ok = true;
    keys(&mut a, ":wq\n");
    assert_eq!(a.outcome, Outcome::Start);
}

#[test]
fn missing_package_manager_is_called_out() {
    let mut a = app(Role::Frontend);
    a.machine.package_manager = None;
    assert!(screen(&a).contains("Homebrew is not installed"));
    a.machine.os = Os::Linux;
    assert!(screen(&a).contains("No apt, dnf or pacman"));
}

#[test]
fn q_and_ctrl_c_cancel() {
    let mut a = app(Role::Frontend);
    keys(&mut a, "q");
    assert_eq!(a.outcome, Outcome::Cancel);
    let mut b = app(Role::Frontend);
    b.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert_eq!(b.outcome, Outcome::Cancel);
}

#[test]
fn panes_braces_and_search() {
    let mut a = app(Role::Frontend);
    keys(&mut a, "\x0c");
    assert_eq!(a.focus, Focus::Preview);
    keys(&mut a, "\x08}");
    assert!(matches!(a.rows[a.cursor], Row::Role(Role::Frontend)));
    keys(&mut a, "}");
    assert!(
        matches!(a.rows[a.cursor], Row::Repo(r) if r.name == REPOS[0].name),
        "past the note"
    );
    keys(&mut a, "/gateway\n");
    assert!(matches!(a.rows[a.cursor], Row::Repo(r) if r.name == "mc2-gateway"));
    keys(&mut a, "G");
    assert!(matches!(a.rows[a.cursor], Row::Start));
    keys(&mut a, "gg");
    assert!(matches!(a.rows[a.cursor], Row::Setup(Setup::Developer)));
}

#[test]
fn every_tool_the_plan_can_ask_for_has_a_source_on_every_os() {
    use crate::backend::{models::package::source, services::plan::plan};

    for os in [Os::Mac, Os::Linux, Os::Windows] {
        let everything = plan(Role::Everything, REPOS.iter().collect(), os);
        for tool in everything.tools {
            assert!(
                source(tool.name, os).is_some(),
                "{} has no source on {os}",
                tool.name
            );
        }
    }
}

#[test]
fn a_name_that_installs_another_program_is_never_used() {
    use crate::backend::models::{Source, package::source};

    // Homebrew calls it kubernetes-cli; crates.io's `hl` and Scoop's lookalikes are not these tools.
    assert_eq!(
        source("kubectl", Os::Mac),
        Some(Source::Brew {
            formula: "kubernetes-cli",
            tap: None
        })
    );
    assert_eq!(
        source("hl", Os::Windows),
        Some(Source::Scoop {
            app: "hl",
            bucket: None
        })
    );
    for os in [Os::Mac, Os::Linux, Os::Windows] {
        assert_ne!(source("hl", os), Some(Source::Cargo("hl")));
    }
    assert!(matches!(
        source("terraform", Os::Linux),
        Some(Source::Brew {
            tap: Some("hashicorp/tap"),
            ..
        })
    ));
    assert_eq!(
        source("docker", Os::Windows),
        Some(Source::Winget("Docker.DockerDesktop"))
    );
}
