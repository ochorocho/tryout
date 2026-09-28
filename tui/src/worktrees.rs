//! The worktrees tryout knows about, read from `ddev tryout worktree list --plain`.
//!
//! `--plain` is the add-on's machine-readable contract (NAME HEAD BRANCH STATE PHP
//! DB URL, space-padded) — the same output tests/e2e parses — so the TUI never
//! re-implements how tryout finds its checkouts.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub name: String,
    pub head: String,
    pub branch: String,
    pub dirty: bool,
    pub php: Option<String>,
    pub db: Option<String>,
    pub url: Option<String>,
    pub primary: bool,
}

impl Worktree {
    /// Served means it has a URL of its own — the primary always does.
    pub fn served(&self) -> bool {
        self.url.is_some()
    }
}

/// Parse `worktree list --plain`. Rows are the indented lines after the NAME
/// header; the title, blank lines and the dim footer are not rows.
pub fn parse_plain(output: &str) -> Vec<Worktree> {
    let clean = strip_ansi(output);
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in clean.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.first() == Some(&"NAME") {
            in_table = true;
            continue;
        }
        if !in_table || !line.starts_with("  ") || fields.len() < 6 {
            in_table = in_table && !line.trim().is_empty();
            continue;
        }
        let dash = |s: &str| (s != "-").then(|| s.to_string());
        let url = fields
            .get(6)
            .filter(|u| u.starts_with("http"))
            .map(|u| u.to_string());
        rows.push(Worktree {
            name: fields[0].to_string(),
            head: fields[1].to_string(),
            branch: fields[2].to_string(),
            dirty: fields[3] == "dirty",
            php: dash(fields[4]),
            db: dash(fields[5]),
            url,
            primary: line.contains("← primary"),
        });
    }
    rows
}

/// Where a worktree's checkout lives: worktrees/<name>, or the project root for
/// the root checkout, which has no directory under worktrees/.
pub fn checkout_dir(root: &Path, name: &str) -> PathBuf {
    let nested = root.join("worktrees").join(name);
    if nested.is_dir() {
        nested
    } else {
        root.to_path_buf()
    }
}

/// The project a path belongs to: the nearest ancestor with the add-on installed.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|p| p.join(".ddev/tryout/functions.sh").is_file())
        .map(Path::to_path_buf)
}

/// Run the listing. Blocking and slow (a `ddev exec`), so callers run it off the
/// UI thread.
pub fn load(root: &Path) -> Result<Vec<Worktree>> {
    let out = Command::new("ddev")
        .args(["tryout", "worktree", "list", "--plain"])
        .current_dir(root)
        .output()
        .context("could not run ddev — is it installed and on PATH?")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!(
            "ddev tryout worktree list failed: {}",
            strip_ansi(err.lines().last().unwrap_or("no output"))
        );
    }
    Ok(parse_plain(&String::from_utf8_lossy(&out.stdout)))
}

/// Drop CSI escape sequences: the command colours its output even when piped.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from a real project, escapes and all.
    const PLAIN: &str = "\n\u{1b}[1mCore worktrees\u{1b}[0m
  NAME         HEAD         BRANCH       STATE  PHP   DB         URL
  main         32d1f513a57  main         clean  8.5   db         https://p.ddev.site ← primary
  jochen       76a06cdfddc  (detached)   dirty  8.2   db_jochen  https://jochen.p.ddev.site
  testa        80b2622defc  (detached)   clean  -     -

  \u{1b}[2mserved sites have their own URL, PHP and database;\u{1b}[0m
  \u{1b}[2mthe primary is whichever worktree 'use' points at\u{1b}[0m
";

    #[test]
    fn parses_every_row_and_nothing_else() {
        let rows = parse_plain(PLAIN);
        let names: Vec<_> = rows.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["main", "jochen", "testa"]);
    }

    #[test]
    fn marks_the_primary_and_what_is_served() {
        let rows = parse_plain(PLAIN);
        assert!(rows[0].primary && rows[0].served());
        assert_eq!(rows[1].url.as_deref(), Some("https://jochen.p.ddev.site"));
        assert!(!rows[1].primary && rows[1].dirty);
        assert!(!rows[2].served());
        assert_eq!(rows[2].php, None);
        assert_eq!(rows[1].php.as_deref(), Some("8.2"));
    }

    #[test]
    fn the_root_checkout_lives_at_the_project_root() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("worktrees/v13")).unwrap();
        assert_eq!(
            checkout_dir(root.path(), "v13"),
            root.path().join("worktrees/v13")
        );
        assert_eq!(checkout_dir(root.path(), "main"), root.path());
    }

    #[test]
    fn finds_the_project_from_inside_a_worktree() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".ddev/tryout")).unwrap();
        std::fs::write(root.path().join(".ddev/tryout/functions.sh"), "").unwrap();
        let deep = root.path().join("worktrees/v13/typo3/sysext");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(find_project_root(&deep).as_deref(), Some(root.path()));
    }
}
