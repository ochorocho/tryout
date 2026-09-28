//! The worktrees tryout knows about, read from `ddev tryout worktree list --json`.
//!
//! `--json` is the add-on's machine-readable contract for tools (pinned by its
//! unit suite), so the TUI never re-implements how tryout finds its checkouts.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Worktree {
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
}

impl Worktree {
    /// Served means it has a site of its own — the primary always does.
    pub fn served(&self) -> bool {
        self.url.is_some()
    }

    pub fn dirty(&self) -> bool {
        self.modified > 0 || self.untracked > 0
    }

    /// "main", or "detached from 14.0" — what a person calls where it stands.
    pub fn position(&self) -> String {
        match (&self.branch, &self.base) {
            (Some(b), _) => b.clone(),
            (None, Some(base)) => format!("detached from {base}"),
            (None, None) => "detached".into(),
        }
    }

    pub fn checkout_dir(&self, root: &Path) -> PathBuf {
        if self.dir == "." {
            root.to_path_buf()
        } else {
            root.join(&self.dir)
        }
    }
}

/// Parse `worktree list --json`. Anything DDEV prints around it is skipped: the
/// array starts at the first line that opens one.
pub fn parse_json(output: &str) -> Result<Vec<Worktree>> {
    let start = output
        .find("\n[")
        .map(|i| i + 1)
        .or_else(|| output.starts_with('[').then_some(0))
        .context("no JSON array in the output — is the add-on older than the TUI?")?;
    serde_json::from_str(&output[start..]).context("the worktree list is not the JSON expected")
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
        .args(["tryout", "worktree", "list", "--json"])
        .current_dir(root)
        .output()
        .context("could not run ddev — is it installed and on PATH?")?;
    if !out.status.success() {
        let err = strip_ansi(&String::from_utf8_lossy(&out.stderr));
        let reason = err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("no output");
        bail!("ddev tryout worktree list failed: {}", reason.trim());
    }
    parse_json(&String::from_utf8_lossy(&out.stdout))
}

/// Drop CSI escape sequences: ddev colours its messages even when piped.
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

    // As `ddev tryout worktree list --json` prints it on a real project.
    pub const JSON: &str = r#"[
  {"name":"main","dir":".","head":"32d1f513a57","branch":"main","base":"main","patches":0,"modified":0,"untracked":0,"primary":true,"url":"https://p.ddev.site","php":"8.5","db":"db","subject":"[BUGFIX] Add check"},
  {"name":"jochen","dir":"worktrees/jochen","head":"382f3012dee","branch":null,"base":"14.0","patches":2,"modified":3,"untracked":1,"primary":false,"url":null,"php":null,"db":null,"subject":"[TASK] Set \"version\""}
]
"#;

    #[test]
    fn parses_the_contract() {
        let rows = parse_json(JSON).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].primary && rows[0].served() && !rows[0].dirty());
        let j = &rows[1];
        assert_eq!(j.position(), "detached from 14.0");
        assert_eq!((j.patches, j.modified, j.untracked), (2, 3, 1));
        assert!(j.dirty() && !j.served());
        assert_eq!(j.subject.as_deref(), Some("[TASK] Set \"version\""));
    }

    #[test]
    fn skips_whatever_ddev_prints_before_the_array() {
        let noisy = format!("Custom configuration detected\n  • something\n{JSON}");
        assert_eq!(parse_json(&noisy).unwrap().len(), 2);
    }

    #[test]
    fn an_old_add_on_gets_a_reason_not_a_parse_error() {
        let err = parse_json("\nCore worktrees\n  NAME HEAD\n").unwrap_err();
        assert!(format!("{err:#}").contains("older than the TUI"));
    }

    #[test]
    fn the_root_checkout_lives_at_the_project_root() {
        let rows = parse_json(JSON).unwrap();
        let root = Path::new("/p");
        assert_eq!(rows[0].checkout_dir(root), PathBuf::from("/p"));
        assert_eq!(
            rows[1].checkout_dir(root),
            PathBuf::from("/p/worktrees/jochen")
        );
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
