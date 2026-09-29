//! Contribution setup for Gerrit: hooks, commit template, push URL, account.

use std::os::unix::fs::PermissionsExt;

use super::ctx::{Ctx, GERRIT_SSH_HOST};
use super::git;

/// What `inspect_contribution_setup` found.
#[derive(Debug, Default, Clone)]
pub struct Setup {
    pub hook_commit_msg: bool,
    pub hook_pre_commit: bool,
    pub template: bool,
    pub push_url: String,
    pub user: String,
}

impl Setup {
    /// Is the push URL Gerrit's SSH one?
    pub fn gerrit_push(&self) -> bool {
        self.push_url
            .strip_prefix("ssh://")
            .is_some_and(|rest| rest.contains(&format!("@{}", ssh_host())))
    }
}

pub fn inspect(ctx: &Ctx) -> Setup {
    let hooks = ctx.core_git_dir().join("hooks");
    let executable = |name: &str| {
        std::fs::metadata(hooks.join(name))
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    let tmpl = git::out(&ctx.root, &["config", "--get", "commit.template"]).unwrap_or_default();
    // Joined as text, as the bash did: an absolute template path does not count.
    let template = !tmpl.is_empty()
        && std::path::Path::new(&format!("{}/{tmpl}", ctx.root.display())).is_file();
    Setup {
        hook_commit_msg: executable("commit-msg"),
        hook_pre_commit: executable("pre-commit"),
        template,
        push_url: git::out(&ctx.root, &["remote", "get-url", "--push", "origin"])
            .unwrap_or_default(),
        user: git::out(&ctx.root, &["config", "--get", "tryout.gerritUser"]).unwrap_or_default(),
    }
}

// ─── cs setup / doctor / uninstall ──────────────────────────────────────────

use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::ctx::{GERRIT_PROJECT, GERRIT_SSH_PORT};
use super::out::{self, BOLD, DIM, GREEN, NC, RED, YELLOW};
use super::{Failed, Step, gerrit};

/// The Gerrit SSH host: review.typo3.org, or TRYOUT_GERRIT_SSH_HOST (tests).
pub fn ssh_host() -> String {
    std::env::var("TRYOUT_GERRIT_SSH_HOST")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| GERRIT_SSH_HOST.into())
}

/// One Gerrit account, looked up anonymously (accounts are world-readable).
pub struct Account {
    pub id: String,
    pub email: String,
    pub name: String,
}

/// Why an account lookup found nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum Lookup {
    /// Gerrit could not be asked.
    Offline,
    /// Not exactly one account matched.
    NoMatch,
}

pub fn query_account(query: &str) -> Result<Account, Lookup> {
    let v =
        gerrit::get(&format!("/accounts/?q={}&o=DETAILS", encode(query))).map_err(|e| match e {
            gerrit::Error::Fetch => Lookup::Offline,
            gerrit::Error::Parse => Lookup::NoMatch,
        })?;
    let list = v
        .as_array()
        .filter(|a| a.len() == 1)
        .ok_or(Lookup::NoMatch)?;
    let a = &list[0];
    let s = |k: &str| {
        a.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    Ok(Account {
        id: a
            .get("_account_id")
            .map(|v| v.to_string())
            .ok_or(Lookup::NoMatch)?,
        email: s("email"),
        name: s("name"),
    })
}

/// Percent-encode a query value (what curl --data-urlencode did).
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn config(core: &Path, key: &str) -> String {
    git::out(core, &["config", "--get", key]).unwrap_or_default()
}

/// The Gerrit username: the argument, TRYOUT_GERRIT_USER, the cached git
/// config, a prompt — and cached for next time.
pub fn resolve_user(core: &Path, given: &str) -> Result<String, Failed> {
    let mut user = given.to_string();
    if user.is_empty() {
        user = std::env::var("TRYOUT_GERRIT_USER").unwrap_or_default();
    }
    if user.is_empty() {
        user = config(core, "tryout.gerritUser");
    }
    if user.is_empty() && super::prompt::have_tty() {
        user =
            super::prompt::ask_text("Gerrit username (review.typo3.org)", "").unwrap_or_default();
    }
    if user.is_empty() {
        out::error("No Gerrit username provided.");
        out::error(
            "  → ddev tryout cs setup <username>   or   export TRYOUT_GERRIT_USER=<username>",
        );
        return Err(Failed);
    }
    git::ok(core, &["config", "tryout.gerritUser", &user]);
    Ok(user)
}

/// Commits must be authored with an address the pushing account owns; a
/// global git identity from another account gets the push rejected. Set the
/// repository-local identity from the Gerrit account.
pub fn configure_author_identity(core: &Path, user: &str) {
    let Ok(acc) = query_account(&format!("username:{user}")) else {
        out::warn(format!(
            "Could not look up Gerrit account '{user}' — skipping author identity check"
        ));
        return;
    };
    if acc.email.is_empty() {
        out::warn(format!(
            "Gerrit account '{user}' exposes no preferred email — set one manually:"
        ));
        out::warn("  git config user.email <your-gerrit-email>");
        return;
    }
    git::ok(core, &["config", "tryout.gerritEmail", &acc.email]);
    let current = git::out(core, &["config", "--local", "--get", "user.email"]).unwrap_or_default();
    if current == acc.email {
        out::success(format!("Author identity already set to {}", acc.email));
        return;
    }
    git::ok(core, &["config", "user.email", &acc.email]);
    if !acc.name.is_empty() {
        git::ok(core, &["config", "user.name", &acc.name]);
    }
    out::success(format!(
        "Author identity set to {} (repository-local)",
        acc.email
    ));
    if !current.is_empty() {
        out::warn(format!(
            "Previous value was {current} — amend commits made before this with:"
        ));
        out::warn("  git commit --amend --reset-author --no-edit");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorStatus {
    Ok { from_cache: bool },
    Mismatch,
    Unregistered,
    Unknown,
    NoEmail,
}

/// The configured author email, judged by Gerrit account id — an account may
/// author with any of its addresses, and only the preferred one is public.
/// Returns (email, repository-local?, status).
pub fn inspect_author(core: &Path, user: &str) -> (String, bool, AuthorStatus) {
    let email = config(core, "user.email");
    if email.is_empty() {
        return (email, false, AuthorStatus::NoEmail);
    }
    let local = !git::out(core, &["config", "--local", "--get", "user.email"])
        .unwrap_or_default()
        .is_empty();
    if user.is_empty() {
        return (email, local, AuthorStatus::Unknown);
    }
    // Only an exact match with the cached preferred address is conclusive.
    let cached = |e: &str| {
        let c = config(core, "tryout.gerritEmail");
        if !c.is_empty() && c == e {
            AuthorStatus::Ok { from_cache: true }
        } else {
            AuthorStatus::Unknown
        }
    };
    let Ok(expected) = query_account(&format!("username:{user}")) else {
        return (email.clone(), local, cached(&email));
    };
    let status = match query_account(&format!("email:{email}")) {
        Ok(a) if a.id == expected.id => AuthorStatus::Ok { from_cache: false },
        Ok(_) => AuthorStatus::Mismatch,
        Err(Lookup::NoMatch) => AuthorStatus::Unregistered,
        Err(Lookup::Offline) => cached(&email),
    };
    (email, local, status)
}

fn install_hook(
    core: &Path,
    git_dir: &Path,
    src: &str,
    name: &str,
    done: &str,
    missing: &str,
) -> bool {
    let src = core.join(src);
    if !src.is_file() {
        out::warn(missing.replace("{src}", &src.display().to_string()));
        return false;
    }
    let dst = git_dir.join("hooks").join(name);
    let _ = std::fs::create_dir_all(dst.parent().expect("has a directory"));
    if std::fs::copy(&src, &dst).is_err() {
        return false;
    }
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755));
    out::success(done);
    true
}

/// Why SSH to Gerrit failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshReason {
    NoUser,
    Unreachable,
    NoAgentKey,
    Denied,
}

impl SshReason {
    pub fn name(self) -> &'static str {
        match self {
            SshReason::NoUser => "no-user",
            SshReason::Unreachable => "unreachable",
            SshReason::NoAgentKey => "no-agent-key",
            SshReason::Denied => "denied",
        }
    }

    /// The next step, which differs inside the container (ddev-ssh-agent) and
    /// on the host (its own agent).
    pub fn hint(self, in_container: bool) -> String {
        match self {
            SshReason::NoUser => "→ set a username: ddev tryout cs setup <gerrit-user>".into(),
            SshReason::Unreachable => {
                format!("→ check firewall/VPN for {}:{GERRIT_SSH_PORT}", ssh_host())
            }
            SshReason::NoAgentKey if in_container => {
                "→ ddev auth ssh   (hands your host keys to ddev-ssh-agent)".into()
            }
            SshReason::NoAgentKey => {
                "→ load your key into your host SSH agent, e.g.: ssh-add ~/.ssh/id_ed25519".into()
            }
            SshReason::Denied => {
                "→ upload your public key at https://review.typo3.org/settings/#SSHKeys".into()
            }
        }
    }
}

/// Can `user` authenticate to Gerrit over SSH? The verdict comes from SSH
/// itself; whether an agent holds a key only classifies a failure — a key on
/// disk authenticates just as well. The port probe is a plain TCP connect (not
/// bash's /dev/tcp, which macOS kills for external hosts).
pub fn diagnose_ssh(user: &str) -> Result<(), SshReason> {
    if user.is_empty() {
        return Err(SshReason::NoUser);
    }
    let host = ssh_host();
    let reachable = (host.as_str(), GERRIT_SSH_PORT)
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .is_some_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(5)).is_ok());
    if !reachable {
        return Err(SshReason::Unreachable);
    }
    let authenticated = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=5",
            "-o",
            "StrictHostKeyChecking=accept-new",
        ])
        .args([
            "-p",
            &GERRIT_SSH_PORT.to_string(),
            &format!("{user}@{host}"),
            "gerrit",
            "version",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if authenticated {
        return Ok(());
    }
    let agent = Command::new("ssh-add")
        .arg("-l")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    let on_disk = std::env::var_os("HOME").is_some_and(|h| {
        std::fs::read_dir(Path::new(&h).join(".ssh"))
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("id_")
                    && !n.ends_with(".pub")
                    && std::fs::File::open(e.path()).is_ok()
            })
    });
    Err(if agent || on_disk {
        SshReason::Denied
    } else {
        SshReason::NoAgentKey
    })
}

/// `cs setup`: hooks, commit template, push URL and author identity.
pub fn setup(ctx: &Ctx, given_user: &str) -> Step {
    let core = &ctx.root;
    let git_dir = ctx.core_git_dir();
    out::print(&format!(
        "\n{BOLD}TYPO3 Core — Contribution Setup{NC}\n─────────────────────────────────────\n\n"
    ));
    out::info("[1/6] Resolving Gerrit username...");
    let user = resolve_user(core, given_user)?;
    out::print(&format!("       {DIM}user: {user}{NC}\n\n"));

    out::info("[2/6] Installing commit-msg hook (Change-Id)...");
    if !install_hook(
        core,
        &git_dir,
        "Build/git-hooks/commit-msg",
        "commit-msg",
        "Installed commit-msg hook (Change-Id generator)",
        "commit-msg hook not found at {src} — Core may be too old.",
    ) {
        out::warn("commit-msg hook install skipped");
    }
    out::print("\n");
    out::info("[3/6] Installing pre-commit hook (CGL checks)...");
    if !install_hook(
        core,
        &git_dir,
        "Build/git-hooks/unix+mac/pre-commit",
        "pre-commit",
        "Installed pre-commit hook (CGL checks)",
        "pre-commit hook not found at {src}",
    ) {
        out::warn("pre-commit hook install skipped");
    }
    out::print("\n");
    out::info("[4/6] Installing commit-message template...");
    let template = ctx.tryout_dir().join("gitmessage.txt");
    if template.is_file() {
        let rel = ".ddev/tryout/gitmessage.txt";
        git::ok(core, &["config", "commit.template", rel]);
        let _ = std::fs::remove_file(format!("{}message.txt", git_dir.display()));
        out::success(format!("Commit template wired to {DIM}{rel}{NC}"));
    } else {
        out::warn(format!(
            "Commit template not found at {}",
            template.display()
        ));
        out::warn("template install skipped");
    }
    out::print("\n");
    out::info("[5/6] Configuring Gerrit push URL...");
    let push = format!(
        "ssh://{user}@{}:{GERRIT_SSH_PORT}/{GERRIT_PROJECT}",
        ssh_host()
    );
    git::ok(core, &["remote", "set-url", "--push", "origin", &push]);
    out::success(format!("Push URL set: {DIM}{push}{NC}"));
    out::print("\n");
    out::info("[6/6] Configuring commit author identity...");
    configure_author_identity(core, &user);
    out::print("\n");

    out::info(format!(
        "Probing Gerrit SSH ({user}@{}:{GERRIT_SSH_PORT})...",
        ssh_host()
    ));
    match diagnose_ssh(&user) {
        Ok(()) => out::success("SSH reachable — you can push to Gerrit"),
        Err(r) => {
            out::warn(format!("SSH probe failed ({})", r.name()));
            out::print(&format!("       {DIM}{}{NC}\n", r.hint(ctx.in_container)));
        }
    }
    out::print("\n─────────────────────────────────────\n");
    out::success("Contribution setup complete!");
    out::print(&format!(
        "\n  {BOLD}Push a change for review:{NC}\n    {DIM}git push origin HEAD:refs/for/{}{NC}\n\n  {BOLD}Diagnose state:{NC} ddev tryout cs doctor\n\n",
        ctx.branch()
    ));
    Ok(())
}

/// `cs doctor`: every piece of the setup, judged live where it can be.
pub fn doctor(ctx: &Ctx) {
    let core = &ctx.root;
    let cs = inspect(ctx);
    let (ok, fail, warn) = (
        format!("{GREEN}✓{NC}"),
        format!("{RED}✗{NC}"),
        format!("{YELLOW}!{NC}"),
    );
    let mut s =
        format!("\n{BOLD}Contribution Setup — Doctor{NC}\n─────────────────────────────────────\n");
    let pad = "                   ";
    if cs.user.is_empty() {
        s.push_str(&format!(
            "  Gerrit user:     {fail} not set\n{pad}{DIM}→ ddev tryout cs setup <username>{NC}\n"
        ));
    } else {
        s.push_str(&format!("  Gerrit user:     {ok} {}\n", cs.user));
    }
    let state = |on: bool, yes: &str, no: &str| {
        if on {
            format!("{ok} {yes}")
        } else {
            format!("{fail} {no}")
        }
    };
    s.push_str(&format!(
        "  commit-msg hook: {}\n",
        state(cs.hook_commit_msg, "installed", "missing")
    ));
    s.push_str(&format!(
        "  pre-commit hook: {}\n",
        state(cs.hook_pre_commit, "installed", "missing")
    ));
    s.push_str(&format!(
        "  Commit template: {}\n",
        state(cs.template, "configured", "not set")
    ));
    if cs.gerrit_push() {
        s.push_str(&format!("  Push URL:        {ok} {}\n", cs.push_url));
    } else if !cs.push_url.is_empty() {
        s.push_str(&format!(
            "  Push URL:        {warn} {}\n{pad}{DIM}(not pointing at Gerrit SSH){NC}\n",
            cs.push_url
        ));
    } else {
        s.push_str(&format!("  Push URL:        {fail} origin missing\n"));
    }
    let (email, local, status) = inspect_author(core, &cs.user);
    let setup_hint = format!("{pad}{DIM}→ ddev tryout cs setup {}{NC}\n", cs.user);
    match status {
        AuthorStatus::Ok { from_cache } => {
            s.push_str(&format!("  Author identity: {ok} {email}\n"));
            if from_cache {
                s.push_str(&format!("{pad}{DIM}(from cache — Gerrit unreachable){NC}\n"));
            }
            if !local {
                s.push_str(&format!("{pad}{warn} inherited from your global git config\n{setup_hint}"));
            }
        }
        AuthorStatus::Mismatch => {
            s.push_str(&format!("  Author identity: {fail} {email} belongs to another Gerrit account\n{setup_hint}"))
        }
        AuthorStatus::Unregistered => s.push_str(&format!(
            "  Author identity: {fail} {email} is not registered on Gerrit\n{pad}{DIM}pushes are rejected as \"invalid author\"{NC}\n{setup_hint}"
        )),
        AuthorStatus::NoEmail => {
            s.push_str(&format!("  Author identity: {fail} no user.email configured\n{setup_hint}"))
        }
        AuthorStatus::Unknown => s.push_str(&format!("  Author identity: {warn} {email} (could not verify)\n")),
    }
    if !cs.user.is_empty() {
        match diagnose_ssh(&cs.user) {
            Ok(()) => s.push_str(&format!(
                "  Gerrit SSH:      {ok} reachable (authenticated)\n"
            )),
            Err(r) => {
                let what = match r {
                    SshReason::Unreachable => format!("{fail} network unreachable"),
                    SshReason::NoAgentKey => format!("{warn} no key in ddev-ssh-agent"),
                    SshReason::Denied => format!("{fail} auth denied by Gerrit"),
                    SshReason::NoUser => format!("{fail} probe failed (no-user)"),
                };
                s.push_str(&format!(
                    "  Gerrit SSH:      {what}\n{pad}{DIM}{}{NC}\n",
                    r.hint(ctx.in_container)
                ));
            }
        }
    }
    s.push_str("─────────────────────────────────────\n\n");
    out::print(&s);
}

/// `cs uninstall`: hooks, template, push URL and the cached account gone.
pub fn uninstall(ctx: &Ctx) {
    let core = &ctx.root;
    let hooks = ctx.core_git_dir().join("hooks");
    out::info("Removing git hooks...");
    let _ = std::fs::remove_file(hooks.join("commit-msg"));
    let _ = std::fs::remove_file(hooks.join("pre-commit"));
    out::success("Removed commit-msg and pre-commit hooks");
    out::info("Unsetting commit template...");
    git::ok(core, &["config", "--unset", "commit.template"]);
    let _ = std::fs::remove_file(core.join(".gitmessage.txt"));
    out::info("Resetting origin push URL to fetch URL...");
    if let Some(url) = git::out(core, &["remote", "get-url", "origin"]).filter(|u| !u.is_empty()) {
        git::ok(core, &["remote", "set-url", "--push", "origin", &url]);
    }
    git::ok(core, &["config", "--unset", "tryout.gerritUser"]);
    git::ok(core, &["config", "--unset", "tryout.gerritEmail"]);
    out::success("Contribution setup removed");
}

/// One line about the HOST's SSH after setup/doctor ran in the container:
/// a push from a host shell uses the host's keys, not ddev-ssh-agent.
pub fn host_ssh_report(user: &str) {
    if user.is_empty() {
        return;
    }
    match diagnose_ssh(user) {
        Ok(()) => out::print(&format!(
            "  Host SSH:        {GREEN}✓{NC} reachable (authenticated) — pushing from a host shell works\n\n"
        )),
        Err(r) => out::print(&format!(
            "  Host SSH:        {YELLOW}!{NC} {} {DIM}{}{NC}\n\n",
            r.name(),
            r.hint(false)
        )),
    }
}

#[cfg(test)]
mod cs_tests {
    use super::*;

    #[test]
    fn a_query_is_percent_encoded() {
        assert_eq!(encode("email:jo@example.com"), "email%3Ajo%40example.com");
    }

    #[test]
    fn no_user_is_its_own_verdict() {
        assert_eq!(diagnose_ssh(""), Err(SshReason::NoUser));
    }

    #[test]
    fn the_no_key_hint_differs_by_side() {
        assert!(SshReason::NoAgentKey.hint(true).contains("ddev auth ssh"));
        assert!(SshReason::NoAgentKey.hint(false).contains("ssh-add"));
    }
}
