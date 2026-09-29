//! Tab completion for `ddev tryout`. DDEV runs the autocomplete shim on every
//! TAB with the command line as argv (`tryout cs doc`, an empty word as the
//! literal `''`) and reads candidates from stdout, one per line:
//! `value<TAB>description`, or `_activeHelp_ <hint>` for a free-text word.
//!
//! It must be instant and must never fail: local files and local git refs only
//! — no network, no `ddev exec`, no per-worktree `git status` — and nothing on
//! stderr, which DDEV would mix into the candidates.

use std::path::{Path, PathBuf};

use crate::core::ctx::{Ctx, DdevEnv, PRIMARY_SITE};
use crate::core::db::Engine;
use crate::core::{php, site, vsort};

use super::verbs::VERBS;

pub fn run(argv: &[String]) {
    let Some(root) = project_root() else { return };
    let ctx = Ctx::new(root, DdevEnv::default());
    // A panic must not reach stderr: DDEV would list its message as candidates.
    std::panic::set_hook(Box::new(|_| {}));
    if let Ok(out) = std::panic::catch_unwind(|| candidates(&ctx, argv)) {
        crate::core::out::print(&out);
    }
}

/// DDEV gives a completion no DDEV_* variables and an arbitrary cwd. The shim
/// exports the root it derives from its own path; otherwise walk up from the
/// cwd to a directory with .ddev/config*.yaml, as DDEV itself does.
fn project_root() -> Option<PathBuf> {
    if let Some(r) = std::env::var_os("DDEV_APPROOT").map(PathBuf::from)
        && r.join(".ddev").is_dir()
    {
        return Some(r);
    }
    let cwd = std::env::current_dir().ok()?;
    cwd.ancestors()
        .filter(|d| *d != Path::new("/"))
        .find(|d| {
            std::fs::read_dir(d.join(".ddev")).is_ok_and(|rd| {
                rd.flatten().any(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.starts_with("config") && n.ends_with(".yaml")
                })
            })
        })
        .map(Path::to_path_buf)
}

struct Line<'a> {
    /// Which word is being completed, counted after "tryout".
    pos: usize,
    verb: &'a str,
    sub: &'a str,
    prev: &'a str,
    partial: &'a str,
    /// The complete words.
    words: &'a [String],
}

impl Line<'_> {
    fn wants_flag(&self) -> bool {
        self.partial.starts_with('-')
    }
    fn on_line(&self, w: &str) -> bool {
        self.words.iter().any(|x| x == w)
    }
}

pub fn candidates(ctx: &Ctx, argv: &[String]) -> String {
    let n = argv.len();
    let arg = |i: usize| argv.get(i).map(String::as_str).unwrap_or("");
    let partial = argv.last().map(String::as_str).unwrap_or("");
    let l = Line {
        pos: n.saturating_sub(2),
        verb: if n >= 3 { arg(1) } else { "" },
        sub: if n >= 4 { arg(2) } else { "" },
        prev: if n >= 2 { arg(n - 2) } else { "" },
        partial: if partial == "''" { "" } else { partial },
        words: if n > 2 { &argv[1..n - 1] } else { &[] },
    };
    let mut o = Out(String::new());
    match l.verb {
        "" | "''" => {
            for v in VERBS {
                o.c(v.name, v.complete);
            }
        }
        "ui" => {
            if l.pos <= 1 {
                o.c(
                    "stop",
                    "close the session: every shell, agent and command in it",
                );
            }
        }
        "status" | "composer" | "help" => o.hint(&format!("{} takes no arguments", l.verb)),
        "download" => {
            o.flag(&l, "--reset", "hard-reset Core to the current branch");
            if l.pos <= 1 && !l.wants_flag() {
                sites(ctx, &mut o);
            }
        }
        "checkout" => {
            o.flag(
                &l,
                "--site",
                "pin the site, so only the branch is asked for",
            );
            if l.pos <= 1 {
                branches(ctx, &mut o)
            } else {
                sites(ctx, &mut o)
            }
        }
        "patch" => {
            if l.pos <= 1 {
                o.hint("Gerrit change number, e.g. 56947 — or nothing to browse the open changes");
                patch_numbers(ctx, &mut o);
            } else {
                sites(ctx, &mut o);
            }
            o.flag(
                &l,
                "--all-branches",
                "Browse changes on every branch, not just this one",
            );
        }
        "reset" => {
            if l.pos <= 1 {
                sites(ctx, &mut o);
            }
        }
        "launch" => {
            if l.pos <= 1 && !l.wants_flag() {
                sites(ctx, &mut o);
            }
            o.flag(&l, "--backend", "open /typo3/ instead of the frontend");
        }
        "exec" => {
            if l.pos <= 1 {
                sites(ctx, &mut o);
            } else if l.pos == 2 {
                o.hint(&format!("command to run in {}", l.sub));
                o.c("typo3", "the TYPO3 console");
                o.c("composer", "Composer in that site");
                o.c("php", "that site's PHP");
                o.c("bash", "a shell in that site");
            }
        }
        "delete" => {
            if !l.on_line("--all") && !l.wants_flag() {
                sites(ctx, &mut o);
            }
            o.flag(&l, "--all", "every site, not just one");
            o.flag(&l, "--yes", "skip the confirmation");
        }
        "cs" => {
            if l.pos <= 1 {
                o.c(
                    "setup",
                    "install hooks, template, push URL, author identity",
                );
                o.c("doctor", "diagnose the contribution setup (probes Gerrit)");
                o.c("uninstall", "remove hooks, reset push URL");
                o.c("help", "show help");
            } else if l.sub == "setup" && l.pos == 2 {
                o.hint("your Gerrit username (default: TRYOUT_GERRIT_USER, then git config)");
            }
        }
        "worktree" => worktree(ctx, &l, &mut o),
        _ => {}
    }
    o.0
}

fn worktree(ctx: &Ctx, l: &Line, o: &mut Out) {
    let third = l.words.get(2).map(String::as_str).unwrap_or("");
    match l.sub {
        "" | "''" => {
            if l.pos <= 1 {
                for (v, d) in [
                    ("add", "create worktrees/<name> from a branch"),
                    ("list", "list worktrees and served sites"),
                    ("use", "point the primary site at a worktree, then rebuild"),
                    (
                        "serve",
                        "give a worktree its own URL, PHP version and database",
                    ),
                    ("unserve", "drop a site, keep the worktree"),
                    ("remove", "remove a worktree (not the primary)"),
                    ("rename", "rename a checkout (the branch is untouched)"),
                    ("branches", "the branches a worktree can be based on"),
                    ("help", "show help"),
                ] {
                    o.c(v, d);
                }
            }
        }
        "add" => {
            if l.pos <= 2 {
                o.hint("name for the new worktree (becomes worktrees/<name>)");
            } else if l.prev == "--php" {
                php_versions(ctx, third, o);
            } else if l.prev == "--db" {
                engines(ctx, o);
            } else {
                if l.pos == 3 && !l.wants_flag() {
                    branches(ctx, o);
                }
                o.flag(l, "--serve", "serve it immediately");
                o.flag(l, "--php", "run it on another PHP version");
                o.flag(l, "--db", "run it on another database type");
                o.flag(l, "--no-restart", "skip the DDEV restart --serve needs");
            }
        }
        "branches" => o.flag(l, "--json", "one JSON array, for tools"),
        "list" => {
            o.flag(l, "--plain", "machine-readable columns, for scripts");
            o.flag(l, "--json", "one JSON array, for tools");
        }
        "use" => {
            if l.pos <= 2 && !l.wants_flag() {
                worktrees(ctx, Mode::NonPrimary, o);
            }
        }
        "remove" | "rm" => {
            if l.pos <= 2 && !l.wants_flag() {
                worktrees(ctx, Mode::NonPrimary, o);
            }
            o.flag(l, "--force", "also drop an unmerged branch");
            o.flag(l, "--yes", "skip the confirmation");
        }
        "serve" => {
            if l.prev == "--php" {
                php_versions(ctx, third, o);
            } else if l.prev == "--db" {
                engines(ctx, o);
            } else {
                if l.pos <= 2 && !l.wants_flag() {
                    worktrees(ctx, Mode::Unserved, o);
                }
                o.flag(l, "--php", "run this site on another PHP version");
                o.flag(l, "--db", "run this site on another database type");
                o.flag(
                    l,
                    "--switch",
                    "with --db: move a served site, keeping its old database",
                );
                o.flag(
                    l,
                    "--no-restart",
                    "skip the DDEV restart a new hostname needs",
                );
            }
        }
        "unserve" => {
            if l.pos <= 2 && !l.wants_flag() {
                worktrees(ctx, Mode::Served, o);
            }
            o.flag(l, "--drop-db", "also drop the site's database");
            o.flag(
                l,
                "--no-restart",
                "skip the DDEV restart that releases the hostname",
            );
        }
        "rename" => {
            if l.pos <= 2 {
                worktrees(ctx, Mode::All, o);
            } else if l.pos == 3 {
                let what = if third.is_empty() {
                    "the worktree"
                } else {
                    third
                };
                o.hint(&format!("new name for {what}"));
            }
        }
        _ => {}
    }
}

struct Out(String);

impl Out {
    /// A described candidate.
    fn c(&mut self, value: &str, description: &str) {
        self.0.push_str(&format!("{value}\t{description}\n"));
    }
    /// Guidance for a free-text word.
    fn hint(&mut self, text: &str) {
        self.0.push_str(&format!("_activeHelp_ {text}\n"));
    }
    /// A flag, offered only once.
    fn flag(&mut self, l: &Line, flag: &str, description: &str) {
        if !l.on_line(flag) {
            self.c(flag, description);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    All,
    NonPrimary,
    Served,
    Unserved,
}

fn served_by_marker(ctx: &Ctx, name: &str) -> bool {
    ctx.instances_dir()
        .join(name)
        .join(".tryout-site")
        .is_file()
}

/// Names from the worktrees/ glob plus the root checkout — no `git status`.
fn worktrees(ctx: &Ctx, mode: Mode, o: &mut Out) {
    let primary = ctx.active_worktree_name();
    let primary_label = "primary — at the project URL";
    let offer_primary = match mode {
        Mode::NonPrimary => false,
        Mode::Unserved => !served_by_marker(ctx, &primary),
        Mode::Served => served_by_marker(ctx, &primary),
        Mode::All => true,
    };
    if offer_primary {
        o.c(&primary, primary_label);
    }
    for name in crate::core::worktree::worktree_names(ctx) {
        if name == primary {
            continue;
        }
        let served = served_by_marker(ctx, &name);
        match mode {
            Mode::Unserved if served => continue,
            Mode::Served if !served => continue,
            _ => {}
        }
        if served {
            let v = site::php_version(ctx, &name);
            let php = if v.is_empty() {
                String::new()
            } else {
                format!(" · PHP {v}")
            };
            o.c(&name, &format!("served{php}"));
        } else {
            o.c(&name, "not served");
        }
    }
}

fn sites(ctx: &Ctx, o: &mut Out) {
    o.c(PRIMARY_SITE, "the primary site at the project URL");
    let mut names: Vec<String> = std::fs::read_dir(ctx.instances_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in names.into_iter().filter(|n| served_by_marker(ctx, n)) {
        let v = site::php_version(ctx, &name);
        let php = if v.is_empty() {
            String::new()
        } else {
            format!(" · PHP {v}")
        };
        o.c(&name, &format!("served worktree{php}"));
    }
}

/// Local refs only: main, release branches newest first, legacy refs last.
fn branches(ctx: &Ctx, o: &mut Out) {
    let current =
        crate::core::git::out(&ctx.root, &["branch", "--show-current"]).unwrap_or_default();
    for b in vsort::picker_order(&ctx.local_core_branches()) {
        let mut what = match b.as_str() {
            "main" => "latest development".to_string(),
            b if b.starts_with("TYPO3_") => "legacy branch".into(),
            _ => "release branch".into(),
        };
        if b == current {
            what.push_str(" — checked out");
        }
        o.c(&b, &what);
    }
}

/// The database types a site can run on, each with what choosing it means.
fn engines(ctx: &Ctx, o: &mut Out) {
    for e in Engine::ALL {
        o.c(e.name(), &e.what(ctx));
    }
}

/// The PHP versions DDEV ships for 8.x, narrowed to what the worktree's
/// composer.json accepts; the last is marked as its default.
fn php_versions(ctx: &Ctx, worktree: &str, o: &mut Out) {
    let all: Vec<String> = ["8.1", "8.2", "8.3", "8.4", "8.5"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut ok = all.clone();
    if !worktree.is_empty()
        && let Some(c) =
            php::core_constraint(&ctx.core_worktree_dir(worktree).join("composer.json"))
    {
        ok = php::matching(&c, &all);
    }
    if ok.is_empty() {
        ok = all;
    }
    let best = ok.last().cloned().unwrap_or_default();
    for v in &ok {
        if *v == best && !worktree.is_empty() {
            o.c(v, &format!("default for {worktree}"));
        } else {
            o.c(v, &format!("PHP {v}"));
        }
    }
}

/// The change numbers configured for auto-apply.
fn patch_numbers(ctx: &Ctx, o: &mut Out) {
    let Ok(f) = std::fs::read_to_string(ctx.root.join(".ddev/config.tryout-patches.yaml")) else {
        return;
    };
    for line in f.lines() {
        let Some(rest) = line.trim_start().strip_prefix('-') else {
            continue;
        };
        let Some(list) = rest.trim_start().strip_prefix("TRYOUT_PATCHES=") else {
            continue;
        };
        for n in list
            .split(',')
            .map(|n| n.replace(' ', ""))
            .filter(|n| !n.is_empty())
        {
            o.c(&n, "from config.tryout-patches.yaml");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ctx::tests::core_repo;
    use std::process::Command;

    /// A project with a served worktree (v13, PHP 8.2), an unserved one (old),
    /// and a patch list.
    fn project() -> (tempfile::TempDir, Ctx) {
        let d = core_repo();
        let root = d.path();
        for (name, base) in [("v13", "13.4"), ("old", "12.4")] {
            let ok = Command::new("git")
                .arg("-C")
                .arg(root)
                .args([
                    "worktree",
                    "add",
                    "-q",
                    "--detach",
                    &format!("worktrees/{name}"),
                    &format!("origin/{base}"),
                ])
                .status()
                .unwrap()
                .success();
            assert!(ok);
        }
        std::fs::create_dir_all(root.join("TYPO3-Instances/v13")).unwrap();
        std::fs::write(root.join("TYPO3-Instances/v13/.tryout-site"), "php=8.2\n").unwrap();
        std::fs::create_dir_all(root.join(".ddev")).unwrap();
        std::fs::write(
            root.join(".ddev/config.tryout-patches.yaml"),
            "web_environment:\n  - TRYOUT_PATCHES=56947, 12345\n",
        )
        .unwrap();
        std::fs::write(
            root.join("worktrees/old/composer.json"),
            r#"{"require":{"php":">=8.1 <8.4"}}"#,
        )
        .unwrap();
        let ctx = Ctx::new(root, DdevEnv::default());
        (d, ctx)
    }

    fn complete(ctx: &Ctx, line: &str) -> String {
        let mut argv = vec!["tryout".to_string()];
        argv.extend(line.split(' ').map(String::from));
        candidates(ctx, &argv)
    }

    fn names(ctx: &Ctx, line: &str) -> Vec<String> {
        complete(ctx, line)
            .lines()
            .filter(|l| !l.starts_with("_activeHelp_ "))
            .map(|l| l.split('\t').next().unwrap().to_string())
            .collect()
    }

    #[test]
    fn every_verb_is_offered_described() {
        let (_d, ctx) = project();
        let got = names(&ctx, "''");
        assert_eq!(got, VERBS.iter().map(|v| v.name).collect::<Vec<_>>());
        // A partly typed verb is not a verb: the list is the same, cobra filters.
        assert_eq!(names(&ctx, "st"), got);
    }

    #[test]
    fn every_candidate_is_described_or_a_hint() {
        let (_d, ctx) = project();
        for line in [
            "''",
            "worktree ''",
            "worktree serve ''",
            "exec v13 ''",
            "patch ''",
            "checkout ''",
        ] {
            for l in complete(&ctx, line).lines() {
                assert!(
                    l.starts_with("_activeHelp_ ")
                        || l.split_once('\t')
                            .is_some_and(|(v, d)| !v.is_empty() && !d.is_empty()),
                    "{line}: {l:?}"
                );
            }
        }
    }

    #[test]
    fn free_text_words_get_a_hint() {
        let (_d, ctx) = project();
        assert!(
            complete(&ctx, "worktree add ''").starts_with("_activeHelp_ name for the new worktree")
        );
        assert!(complete(&ctx, "status ''").contains("_activeHelp_ status takes no arguments"));
        assert!(complete(&ctx, "worktree rename v13 ''").contains("_activeHelp_ new name for v13"));
    }

    #[test]
    fn serve_offers_the_unserved_and_unserve_the_served() {
        let (_d, ctx) = project();
        assert_eq!(
            names(&ctx, "worktree serve ''"),
            ["main", "old", "--php", "--db", "--switch", "--no-restart"]
        );
        assert_eq!(
            names(&ctx, "worktree unserve ''"),
            ["v13", "--drop-db", "--no-restart"]
        );
        // use and remove never offer the primary.
        assert_eq!(names(&ctx, "worktree use ''"), ["old", "v13"]);
    }

    #[test]
    fn a_flag_is_offered_once_and_alone_after_a_dash() {
        let (_d, ctx) = project();
        assert_eq!(names(&ctx, "delete --all ''"), ["--yes"]);
        assert_eq!(names(&ctx, "launch -"), ["--backend"]);
        assert_eq!(
            names(&ctx, "worktree add x --serve ''"),
            ["--php", "--db", "--no-restart"]
        );
    }

    #[test]
    fn database_engines_complete_after_db() {
        let (_d, ctx) = project();
        assert_eq!(
            names(&ctx, "worktree serve old --db ''"),
            ["mariadb", "mysql", "postgres", "sqlite"]
        );
        let out = complete(&ctx, "worktree add x --db ''");
        assert!(
            out.contains("postgres\tits own postgres:17 server"),
            "{out}"
        );
        assert!(
            out.contains("sqlite\ta file in the site, no server"),
            "{out}"
        );
    }

    #[test]
    fn php_versions_follow_the_worktrees_constraint() {
        let (_d, ctx) = project();
        assert_eq!(
            names(&ctx, "worktree serve old --php ''"),
            ["8.1", "8.2", "8.3"]
        );
        assert!(complete(&ctx, "worktree serve old --php ''").contains("8.3\tdefault for old"));
    }

    #[test]
    fn sites_branches_and_patches_come_from_disk() {
        let (_d, ctx) = project();
        assert_eq!(names(&ctx, "reset ''"), ["@primary", "v13"]);
        assert_eq!(
            names(&ctx, "checkout ''"),
            ["--site", "main", "13.4", "12.4"]
        );
        assert_eq!(
            names(&ctx, "patch ''"),
            ["56947", "12345", "--all-branches"]
        );
    }

    #[test]
    fn completion_answers_instantly() {
        let (_d, ctx) = project();
        // The fastest of several rounds: a busy machine slows every round, while
        // something expensive in completion (a git status is seconds on a cold
        // Core tree, the network worse) slows even the fastest.
        let fastest = (0..5)
            .map(|_| {
                let start = std::time::Instant::now();
                for line in [
                    "''",
                    "worktree serve ''",
                    "checkout ''",
                    "exec ''",
                    "worktree rename ''",
                ] {
                    complete(&ctx, line);
                }
                start.elapsed()
            })
            .min()
            .expect("five rounds");
        // Five completions; a TAB must never feel it.
        assert!(fastest < std::time::Duration::from_secs(1), "{fastest:?}");
    }
}
