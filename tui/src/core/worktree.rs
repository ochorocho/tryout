//! Core checkouts: the root clone (the primary) and the worktrees nested under
//! worktrees/<name>.

use std::path::Path;

use super::ctx::Ctx;
use super::{git, vsort};

const ADD_USAGE: &str = "  → ddev tryout worktree add <name> [<branch>]";

/// A name becomes a directory, so keep it strictly harmless. On failure, the
/// error lines to print.
pub fn validate_name(name: &str) -> Result<(), Vec<String>> {
    if name.is_empty() {
        return Err(vec!["Missing worktree name".into(), ADD_USAGE.into()]);
    }
    // A leading hyphen is what a mis-parsed flag looks like, and is unusable as
    // a directory or a branch anyway.
    if name.starts_with('-') {
        return Err(vec![
            format!("Invalid worktree name '{name}' (cannot start with '-')"),
            ADD_USAGE.into(),
        ]);
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(vec![format!(
            "Invalid worktree name '{name}' (allowed: letters, digits, . _ -)"
        )]);
    }
    if name == "." || name == ".." {
        return Err(vec![format!("Invalid worktree name '{name}'")]);
    }
    Ok(())
}

/// For a detached HEAD, the branch it was based on: the newest release branch on
/// origin containing it, else main.
pub fn detect_detached_base_branch(dir: &Path) -> String {
    let refs: Vec<String> = git::lines(
        dir,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "--contains",
            "HEAD",
            "refs/remotes/origin",
        ],
    )
    .into_iter()
    .map(|r| r.strip_prefix("origin/").map(String::from).unwrap_or(r))
    .collect();
    let mut releases: Vec<String> = refs.iter().filter(|r| is_release(r)).cloned().collect();
    vsort::sort(&mut releases);
    releases.pop().unwrap_or_else(|| "main".into())
}

/// `[0-9]+\.[0-9]+`
pub fn is_release(b: &str) -> bool {
    b.split_once('.').is_some_and(|(a, c)| {
        [a, c]
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// The Core checkout a path sits in (anything inside it counts), or None.
/// Both sides are resolved first — macOS reaches a project through /var while
/// other tools report /private/var — and worktrees/ is tested before the root,
/// because it lives inside the root checkout.
pub fn name_for_path(ctx: &Ctx, path: &Path) -> Option<String> {
    let root = ctx.root.canonicalize().ok()?;
    let real = path.canonicalize().ok()?;
    let rel = real.strip_prefix(&root).ok()?;
    let mut parts = rel.components();
    match parts.next() {
        None => Some(ctx.plain_core_name()),
        Some(first) if first.as_os_str() == "worktrees" => parts
            .next()
            .map(|n| n.as_os_str().to_string_lossy().into_owned()),
        Some(_) => Some(ctx.plain_core_name()),
    }
}

/// Uncommitted changes in a checkout. A path that is not a checkout is clean:
/// "cannot look" must never read as "has changes".
pub fn is_dirty(dir: &Path) -> bool {
    if !git::ok(dir, &["rev-parse", "--git-dir"]) {
        return false;
    }
    !git::ok(dir, &["diff", "--quiet"]) || !git::ok(dir, &["diff", "--cached", "--quiet"])
}

/// (modified, untracked) from `git status --porcelain`; (0, 0) off a checkout.
pub fn change_counts(dir: &Path) -> (usize, usize) {
    if !git::ok(dir, &["rev-parse", "--git-dir"]) {
        return (0, 0);
    }
    let status = git::out(dir, &["status", "--porcelain"]).unwrap_or_default();
    let untracked = status.lines().filter(|l| l.starts_with("??")).count();
    let modified = status
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('?'))
        .count();
    (modified, untracked)
}

/// The same in words: "clean", "3 modified", "1 untracked", or both.
pub fn change_summary(dir: &Path) -> String {
    match change_counts(dir) {
        (0, 0) => "clean".into(),
        (m, 0) => format!("{m} modified"),
        (0, u) => format!("{u} untracked"),
        (m, u) => format!("{m} modified, {u} untracked"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ctx::{DdevEnv, tests::core_repo};
    use std::process::Command;

    #[test]
    fn worktree_names_are_validated() {
        for ok in ["main", "v13", "feature_x", "a.b-c"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        assert_eq!(validate_name("").unwrap_err()[0], "Missing worktree name");
        assert!(validate_name("--php").unwrap_err()[0].contains("cannot start with '-'"));
        for bad in ["a/b", "../x", "a b", "é"] {
            assert!(
                validate_name(bad).unwrap_err()[0].contains("allowed: letters"),
                "{bad}"
            );
        }
        assert_eq!(
            validate_name("..").unwrap_err(),
            ["Invalid worktree name '..'"]
        );
    }

    #[test]
    fn a_detached_head_answers_with_the_newest_release_containing_it() {
        let d = core_repo();
        assert_eq!(detect_detached_base_branch(d.path()), "13.4");
        let g = |a: &[&str]| {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(d.path())
                    .args(a)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        g(&["update-ref", "-d", "refs/remotes/origin/13.4"]);
        g(&["update-ref", "-d", "refs/remotes/origin/12.4"]);
        assert_eq!(detect_detached_base_branch(d.path()), "main");
        let none = tempfile::tempdir().unwrap();
        assert_eq!(detect_detached_base_branch(none.path()), "main");
    }

    #[test]
    fn a_path_names_the_worktree_it_sits_in() {
        let d = core_repo();
        let c = Ctx::new(d.path(), DdevEnv::default());
        std::fs::create_dir_all(d.path().join("worktrees/v13/typo3")).unwrap();
        std::fs::create_dir_all(d.path().join("typo3/sysext")).unwrap();
        assert_eq!(
            name_for_path(&c, &d.path().join("worktrees/v13/typo3")).as_deref(),
            Some("v13")
        );
        assert_eq!(
            name_for_path(&c, &d.path().join("worktrees/v13")).as_deref(),
            Some("v13")
        );
        assert_eq!(name_for_path(&c, &d.path().join("worktrees")), None);
        assert_eq!(
            name_for_path(&c, &d.path().join("typo3/sysext")).as_deref(),
            Some("main")
        );
        assert_eq!(name_for_path(&c, d.path()).as_deref(), Some("main"));
        assert_eq!(name_for_path(&c, Path::new("/")), None);
    }

    #[test]
    fn a_path_that_is_not_a_checkout_is_clean() {
        let none = tempfile::tempdir().unwrap();
        assert!(!is_dirty(none.path()));
        assert_eq!(change_summary(none.path()), "clean");
        let d = core_repo();
        assert!(!is_dirty(d.path()));
        std::fs::write(d.path().join("composer.json"), "{}").unwrap();
        std::fs::write(d.path().join("new.txt"), "").unwrap();
        assert!(is_dirty(d.path()));
        assert_eq!(change_summary(d.path()), "1 modified, 1 untracked");
    }
}

// ─── Listing ────────────────────────────────────────────────────────────────

/// One row of the fast lister: what `git` says about a checkout, without the
/// dirty check (two `git diff` per worktree — seconds on a cold Mutagen tree).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub head: String,
    /// The branch, or "(detached)".
    pub branch: String,
    pub active: bool,
}

/// Every checkout: the root first (unless a worktrees/<its name> shadows it),
/// then worktrees/* in name order.
pub fn rows(ctx: &Ctx) -> Vec<Row> {
    let active = ctx.active_worktree_name();
    let row = |name: String, dir: &Path| {
        let head =
            git::out(dir, &["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
        let branch = match git::out(dir, &["branch", "--show-current"]) {
            Some(b) if !b.is_empty() => b,
            _ => "(detached)".into(),
        };
        let active = name == active;
        Row {
            name,
            head,
            branch,
            active,
        }
    };
    let mut out = Vec::new();
    let root_name = ctx.plain_core_name();
    if !ctx.core_worktree_dir(&root_name).is_dir() {
        out.push(row(root_name, &ctx.root));
    }
    for name in worktree_names(ctx) {
        let dir = ctx.core_worktree_dir(&name);
        out.push(row(name, &dir));
    }
    out
}

/// The directories under worktrees/, sorted — the glob the listers walk.
pub fn worktree_names(ctx: &Ctx) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(ctx.worktrees_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    v.sort();
    v
}

/// What a checkout is based on, and how many commits sit on top of it — the
/// applied patches. Detached checkouts answer from the remote branches that
/// contain HEAD, attached ones from their upstream.
pub fn base_info(dir: &Path, branch: &str) -> (String, u32) {
    let (base, upstream) = if branch == "(detached)" {
        let base = detect_detached_base_branch(dir);
        let up = format!("origin/{base}");
        (base, up)
    } else {
        let up = git::out(dir, &["rev-parse", "--abbrev-ref", "@{upstream}"])
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| format!("origin/{branch}"));
        (up.strip_prefix("origin/").unwrap_or(&up).to_string(), up)
    };
    let count = git::out(dir, &["rev-list", "--count", &format!("{upstream}..HEAD")])
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (base, count)
}

/// What a worktree serves: (url, php, db), all empty when nothing.
pub fn site_info(ctx: &Ctx, name: &str, active: bool) -> (String, String, String) {
    use super::site;
    if site::is_served(ctx, name) {
        (
            format!("https://{}", site::hostname(ctx, name)),
            site::php_version(ctx, name),
            site::database(name),
        )
    } else if active {
        (
            ctx.env.primary_url.clone(),
            ctx.env.php_version.clone(),
            "db".into(),
        )
    } else {
        Default::default()
    }
}

/// One checkout, as `worktree list --json` reports it and the TUI reads it. The
/// keys are a contract a released TUI parses: add, never rename.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Info {
    pub name: String,
    /// Relative to the project root; "." for the root checkout.
    pub dir: String,
    pub head: String,
    /// None when detached — which every worktree tryout creates is.
    pub branch: Option<String>,
    /// What it is compared against: the branch it came from.
    pub base: Option<String>,
    /// Commits on top of the base: the applied patches.
    pub patches: u32,
    pub modified: u32,
    pub untracked: u32,
    pub primary: bool,
    pub url: Option<String>,
    pub php: Option<String>,
    pub db: Option<String>,
    pub subject: Option<String>,
    /// PHP versions this site could run on (installed, and accepted by its
    /// Core). Absent from an older add-on.
    #[serde(default)]
    pub php_versions: Vec<String>,
}

/// Every checkout with its state. `available` is the PHP versions the web image
/// provides, which only the container can see.
pub fn infos(ctx: &Ctx, available: &[String]) -> Vec<Info> {
    // Each checkout costs a `git status` — about two seconds on a cold Core tree
    // — so they are read side by side, not one after another.
    let rows = rows(ctx);
    std::thread::scope(|scope| {
        let handles: Vec<_> = rows
            .into_iter()
            .map(|r| scope.spawn(move || info(ctx, r, available)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a worktree read panicked"))
            .collect()
    })
}

fn info(ctx: &Ctx, r: Row, available: &[String]) -> Info {
    let some = |s: String| (!s.is_empty()).then_some(s);
    let dir = ctx.core_checkout_dir(&r.name);
    let rel = match dir.strip_prefix(&ctx.root) {
        Ok(p) if p.as_os_str().is_empty() => ".".to_string(),
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => dir.to_string_lossy().into_owned(),
    };
    let (base, patches) = base_info(&dir, &r.branch);
    let (modified, untracked) = change_counts(&dir);
    let (url, php, db) = site_info(ctx, &r.name, r.active);
    let subject = git::out(&dir, &["log", "-1", "--format=%s"]).unwrap_or_default();
    let constraint = super::php::core_constraint(&dir.join("composer.json")).unwrap_or_default();
    Info {
        name: r.name,
        dir: rel,
        head: r.head,
        branch: some(r.branch).filter(|b| b != "(detached)"),
        base: some(base),
        patches,
        modified: modified as u32,
        untracked: untracked as u32,
        primary: r.active,
        url: some(url),
        php: some(php),
        db: some(db),
        subject: some(subject),
        php_versions: super::php::matching(&constraint, available),
    }
}

/// `worktree list --json`, laid out exactly as the bash wrote it: one object per
/// line inside the array.
pub fn infos_json(infos: &[Info]) -> String {
    use super::out::{json_str, json_str_or_null};
    let opt = |o: &Option<String>| json_str_or_null(o.as_deref().unwrap_or(""));
    let mut s = String::from("[");
    for (i, w) in infos.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let phps: Vec<String> = w.php_versions.iter().map(|v| json_str(v)).collect();
        s.push_str(&format!(
            "\n  {{\"name\":{},\"dir\":{},\"head\":{},\"branch\":{},\"base\":{},\"patches\":{},\"modified\":{},\"untracked\":{},\"primary\":{},\"url\":{},\"php\":{},\"db\":{},\"subject\":{},\"php_versions\":[{}]}}",
            json_str(&w.name),
            json_str(&w.dir),
            json_str(&w.head),
            opt(&w.branch),
            opt(&w.base),
            w.patches,
            w.modified,
            w.untracked,
            w.primary,
            opt(&w.url),
            opt(&w.php),
            opt(&w.db),
            opt(&w.subject),
            phps.join(","),
        ));
    }
    s.push_str("\n]\n");
    s
}

/// `worktree branches --json`: the branches a new worktree can be based on.
pub fn branches_json(branches: &[String]) -> String {
    let items: Vec<String> = branches.iter().map(|b| super::out::json_str(b)).collect();
    format!("[{}]\n", items.join(","))
}

/// Fetch the branch tips once when the clone carried only one branch, so a
/// picker has something to offer. Never from completion: it hits the network.
pub fn ensure_branch_refs(ctx: &Ctx) -> bool {
    let n = git::lines(&ctx.root, &["for-each-ref", "refs/remotes/origin"]).len();
    if n > 1 {
        return true;
    }
    super::out::notice(
        super::out::Level::Info,
        "Fetching the branch list (once; the clone only carried one branch)...",
    );
    if !git::ok(
        &ctx.root,
        &[
            "fetch",
            "--depth",
            "1",
            "origin",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
    ) {
        super::out::notice(
            super::out::Level::Warn,
            "Could not fetch the branch list — only the current branch is offered",
        );
        return false;
    }
    true
}

/// `ensure_branch_refs` with its notices silenced, for output a tool parses.
pub fn ensure_branch_refs_quiet(ctx: &Ctx) {
    let n = git::lines(&ctx.root, &["for-each-ref", "refs/remotes/origin"]).len();
    if n <= 1 {
        git::ok(
            &ctx.root,
            &[
                "fetch",
                "--depth",
                "1",
                "origin",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
        );
    }
}

/// True when the primary's vendor/ was built from another Core than the one it
/// now serves — a switch without a rebuild.
pub fn vendor_core_mismatch(ctx: &Ctx) -> bool {
    let link = ctx.instance_dir().join("vendor/typo3/cms-core");
    if !link.is_symlink() {
        return false;
    }
    let (Ok(resolved), Ok(expected)) = (link.canonicalize(), ctx.active_core_dir().canonicalize())
    else {
        return false;
    };
    !resolved.starts_with(&expected) || resolved == expected
}

// ─── Changing checkouts ─────────────────────────────────────────────────────

use super::out::{self, DIM, NC};
use super::{Failed, Step, proc};

fn fail(lines: &[String]) -> Failed {
    for l in lines {
        out::error(l);
    }
    Failed
}

/// The worktree that owns the object store — git lists it first.
pub fn main_dir(ctx: &Ctx) -> Option<std::path::PathBuf> {
    git::lines(&ctx.root, &["worktree", "list", "--porcelain"])
        .into_iter()
        .find_map(|l| l.strip_prefix("worktree ").map(std::path::PathBuf::from))
}

/// Keep everything the add-on generates out of `git status`, in
/// .git/info/exclude — local to the clone, never part of a patch, and shared by
/// every worktree. Idempotent.
pub fn ensure_excludes(ctx: &Ctx) {
    let f = ctx.core_git_dir().join("info/exclude");
    let _ = std::fs::create_dir_all(f.parent().expect("has a directory"));
    let mut text = std::fs::read_to_string(&f).unwrap_or_default();
    for e in ["/.ddev/", "/worktrees/", "/TYPO3-Instances/", "/packages/"] {
        if !text.lines().any(|l| l == e) {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(e);
            text.push('\n');
        }
    }
    let _ = std::fs::write(&f, text);
}

/// Worktree metadata with relative paths, so host and container share it;
/// `worktree repair` mends one the other side wrote with absolute paths.
pub fn ensure_relative_paths(ctx: &Ctx) {
    if !git::supports_relative_worktrees() {
        return;
    }
    let main = main_dir(ctx).unwrap_or_else(|| ctx.root.clone());
    if !main.join(".git").exists()
        || !git::ok(&main, &["config", "worktree.useRelativePaths", "true"])
    {
        return;
    }
    let main_real = main.canonicalize().unwrap_or(main.clone());
    for name in worktree_names(ctx) {
        let dir = ctx.core_worktree_dir(&name);
        if dir.canonicalize().is_ok_and(|d| d == main_real) {
            continue;
        }
        git::ok(&main, &["worktree", "repair", &dir.to_string_lossy()]);
    }
}

/// Clone Core INTO the project root, which is never empty (`ddev config` wrote
/// .ddev/): init, fetch the one branch, check it out — what `git clone` would
/// do, without its emptiness rule.
pub fn clone_into_root(ctx: &Ctx, branch: &str) -> Step {
    use super::ctx::{CORE_REPO, GERRIT_REMOTE};
    let root = &ctx.root;
    out::info(format!("Fetching TYPO3 Core ({branch})..."));
    if !git::ok(root, &["init", "-q"]) {
        return Err(fail(&[format!("git init failed in {}", root.display())]));
    }
    if !git::ok(root, &["remote", "add", "origin", CORE_REPO]) {
        git::ok(root, &["remote", "set-url", "origin", CORE_REPO]);
    }
    git::ok(root, &["remote", "add", "gerrit", GERRIT_REMOTE]);
    if !proc::git(root, &["fetch", "--depth", "1", "origin", branch]) {
        return Err(fail(&[format!(
            "Failed to fetch {branch} from {CORE_REPO}"
        )]));
    }
    if !proc::git(root, &["checkout", "-f", "-B", branch, "FETCH_HEAD"]) {
        return Err(fail(&[format!("Failed to check out {branch}")]));
    }
    git::ok(
        root,
        &[
            "branch",
            &format!("--set-upstream-to=origin/{branch}"),
            branch,
        ],
    );
    ensure_excludes(ctx);
    Ok(())
}

/// Move a checkout to the tip of its base, staying on whatever it is on — never
/// checking the base out, which another worktree may hold.
pub fn reset_to_base(ctx: &Ctx, dir: &Path, branch: &str, site_name: &str) {
    proc::git(dir, &["fetch", "origin"]);
    proc::git(dir, &["reset", "--hard", &format!("origin/{branch}")]);
    proc::git(dir, &["clean", "-fd"]);
    proc::clear_dir(&super::site::dir(ctx, site_name).join("var/cache"));
}

/// Create worktrees/<name>, always DETACHED at origin/<branch>: no local
/// branch, so nothing collides and nothing is left behind.
pub fn add(ctx: &Ctx, name: &str, branch: &str) -> Step {
    validate_name(name).map_err(|l| fail(&l))?;
    let dir = ctx.core_worktree_dir(name);
    if dir.exists() {
        return Err(fail(&[
            format!("Worktree '{name}' already exists at {name}"),
            format!("  → ddev tryout worktree use {name}"),
        ]));
    }
    if !git::supports_relative_worktrees() {
        let v = git::version()
            .map(|(a, b)| format!("{a}.{b}"))
            .unwrap_or_default();
        return Err(fail(&[
            format!("git {v} cannot write relative worktree paths (needs 2.48+)"),
            "  → ddev restart   (rebuilds the web image with the add-on's git)".into(),
        ]));
    }
    let main = main_dir(ctx).unwrap_or_else(|| ctx.root.clone());
    ensure_relative_paths(ctx);
    out::info("Fetching origin...");
    if !proc::git(&main, &["fetch", "origin"]) {
        return Err(fail(&["Fetch failed".into()]));
    }
    if !git::ok(
        &main,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("origin/{branch}"),
        ],
    ) {
        return Err(fail(&[
            format!("Branch '{branch}' does not exist on origin"),
            "  → ddev tryout checkout   (lists available branches)".into(),
        ]));
    }
    out::info(format!("Creating worktree '{name}' at origin/{branch}..."));
    if !proc::git(
        &main,
        &[
            "worktree",
            "add",
            "--detach",
            &dir.to_string_lossy(),
            &format!("origin/{branch}"),
        ],
    ) {
        return Err(Failed);
    }
    out::success(format!("Worktree '{name}' created"));
    Ok(())
}

/// Point the primary instance's overlay at a checkout (the root for its own
/// name). Never a symlink: the root is a directory, and `ln -sfn` would put a
/// link inside it.
pub fn set_active(ctx: &Ctx, name: &str) -> Step {
    let target = if name == ctx.plain_core_name() && !ctx.core_worktree_dir(name).is_dir() {
        ""
    } else {
        name
    };
    super::composer::use_core(ctx, target)
        .map(|_| ())
        .map_err(|e| {
            eprintln!("{e}");
            Failed
        })
}

/// Serve a checkout at the project URL. The rebuild is mandatory: vendor/ is
/// bound to the resolved paths, so without it the site keeps the old Core.
pub fn use_core(ctx: &Ctx, name: &str) -> Step {
    validate_name(name).map_err(|l| fail(&l))?;
    if !ctx.core_checkout_dir(name).is_dir() {
        return Err(fail(&[
            format!("No worktree '{name}'"),
            "  → ddev tryout worktree list".into(),
        ]));
    }
    if ctx.active_worktree_name() == name {
        out::info(format!("'{name}' is already active — rebuilding anyway"));
    }
    set_active(ctx, name)?;
    out::success(format!("Active Core: {name}"));
    out::info("Syncing composer.tryout.json...");
    if !super::serve::sync_composer(&ctx.instance_dir(), &ctx.active_core_dir()) {
        out::warn("composer sync had warnings");
    }
    super::serve::wipe_vendor(ctx, "")?;
    super::serve::rebuild(ctx, "")
}

/// Remove a worktree and its directory. `--force` always (a Core checkout
/// always carries untracked files), twice on the user's `--force`; the prune
/// runs before the sweep; a legacy branch goes with `-d`, so unpushed work stays.
pub fn remove(ctx: &Ctx, name: &str, force: bool) -> Step {
    validate_name(name).map_err(|l| fail(&l))?;
    let dir = ctx.core_worktree_dir(name);
    if !dir.is_dir() {
        return Err(fail(&[format!("No worktree '{name}'")]));
    }
    if ctx.active_worktree_name() == name {
        return Err(fail(&[
            format!("Cannot remove the active worktree '{name}'"),
            "  → ddev tryout worktree use <other>   first".into(),
        ]));
    }
    if main_dir(ctx).is_some_and(|m| m == dir) {
        return Err(fail(&[format!(
            "Cannot remove '{name}': it owns the shared git object store"
        )]));
    }
    let d = dir.to_string_lossy().into_owned();
    let mut args = vec!["worktree", "remove", "--force"];
    if force {
        args.push("--force");
    }
    args.push(&d);
    if !proc::git(&ctx.root, &args) {
        return Err(fail(&[
            format!("Failed to remove worktree '{name}'"),
            format!(
                "  → git -C {} worktree remove --force {d}",
                ctx.root.display()
            ),
        ]));
    }
    git::ok(&ctx.root, &["worktree", "prune"]);
    if dir.is_dir() {
        let _ = std::fs::remove_dir_all(&dir);
        if dir.is_dir() {
            out::warn(format!("Could not delete {d} — remove it by hand"));
        }
    }
    let main = main_dir(ctx).unwrap_or_else(|| ctx.root.clone());
    if git::ok(
        &main,
        &["show-ref", "-q", "--verify", &format!("refs/heads/{name}")],
    ) {
        let del = if force { "-D" } else { "-d" };
        if git::ok(&main, &["branch", del, name]) {
            out::success(format!(
                "Removed worktree '{name}', its directory and its branch"
            ));
            return Ok(());
        }
        out::warn(format!(
            "Branch '{name}' kept — it has commits that are not merged"
        ));
        out::print_line(&format!(
            "  {DIM}→ ddev tryout worktree remove {name} --force   (delete it too){NC}"
        ));
        out::print_line(&format!(
            "  {DIM}→ or: git -C {} branch -D {name}{NC}",
            main.display()
        ));
    }
    out::success(format!("Removed worktree '{name}' and its directory"));
    Ok(())
}

/// Rename a checkout (never its branch). A served site moves with it: unserved
/// keeping its database, then served again under the new name.
pub fn rename(ctx: &Ctx, old: &str, new: &str) -> Step {
    validate_name(old).map_err(|l| fail(&l))?;
    validate_name(new).map_err(|l| fail(&l))?;
    if old == new {
        out::info(format!("'{old}' is already called that"));
        return Ok(());
    }
    let (old_dir, new_dir) = (ctx.core_worktree_dir(old), ctx.core_worktree_dir(new));
    if !old_dir.is_dir() {
        return Err(fail(&[format!("No worktree '{old}'")]));
    }
    if new_dir.exists() {
        return Err(fail(&[format!("'{new}' already exists")]));
    }
    let was_active = ctx.active_worktree_name() == old;
    let served = super::site::is_served(ctx, old);
    let php = if served {
        super::site::php_version(ctx, old)
    } else {
        String::new()
    };
    if served {
        out::info(format!(
            "Unserving '{old}' so it can be re-served as '{new}'..."
        ));
        let _ = silently(|| super::serve::unserve(ctx, old, true));
    }
    if !git::ok(
        &old_dir,
        &[
            "worktree",
            "move",
            &old_dir.to_string_lossy(),
            &new_dir.to_string_lossy(),
        ],
    ) {
        return Err(fail(&[format!(
            "Could not move {} (uncommitted changes, or it is locked?)",
            old_dir.display()
        )]));
    }
    if was_active {
        set_active(ctx, new)?;
    }
    out::success(format!("Renamed worktree '{old}' to '{new}'"));
    if served {
        out::info(format!("Re-serving as '{new}' on PHP {php}..."));
        if super::serve::serve(ctx, new, &php).is_err() {
            return Err(fail(&[
                "The worktree was renamed, but re-serving failed".into(),
                format!("  → ddev tryout worktree serve {new}"),
            ]));
        }
    }
    Ok(())
}

/// Run a step with stdout and stderr silenced (the bash `>/dev/null 2>&1`).
fn silently<T>(f: impl FnOnce() -> T) -> T {
    use std::os::fd::AsRawFd;
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let null = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .ok();
    // SAFETY: dup/dup2 on this process's own standard descriptors, restored below.
    let saved = unsafe { (libc::dup(1), libc::dup(2)) };
    if let Some(n) = &null {
        unsafe {
            libc::dup2(n.as_raw_fd(), 1);
            libc::dup2(n.as_raw_fd(), 2);
        }
    }
    let r = f();
    let _ = std::io::Write::flush(&mut std::io::stdout());
    unsafe {
        libc::dup2(saved.0, 1);
        libc::dup2(saved.1, 2);
        libc::close(saved.0);
        libc::close(saved.1);
    }
    r
}
