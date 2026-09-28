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
