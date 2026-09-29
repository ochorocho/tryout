//! The worktrees tryout knows about — read with the add-on's own code, the same
//! the `worktree list --json` contract is written from.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

use crate::core::ctx::Ctx;
use crate::core::{gerrit, site, worktree};

/// One checkout, as the add-on's `worktree list --json` reports it — the same
/// type, so the TUI and the contract cannot drift apart.
pub type Worktree = crate::core::worktree::Info;

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
        .find(|p| p.join(".ddev/tryout").is_dir())
        .map(Path::to_path_buf)
}

/// The project's context as the TUI sees it: the environment `ddev tryout ui`
/// was started with carries DDEV's values for the project.
fn context(root: &Path) -> Ctx {
    Ctx::from_env(root)
}

/// The PHP versions the web image provides. Only the container can look, so
/// post-start leaves them in .ddev/tryout/.state/php-versions; before it has,
/// DDEV's 8.x line-up stands in.
fn php_versions(root: &Path) -> Vec<String> {
    let snapshot =
        std::fs::read_to_string(root.join(".ddev/tryout/.state/php-versions")).unwrap_or_default();
    let v: Vec<String> = snapshot.split_whitespace().map(String::from).collect();
    if v.is_empty() {
        ["8.1", "8.2", "8.3", "8.4", "8.5"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        v
    }
}

/// Every checkout with its state, read in-process with the host's git. A few
/// git calls per worktree, so callers run it off the UI thread.
pub fn load(root: &Path) -> Result<Vec<Worktree>> {
    let ctx = context(root);
    if !ctx.has_core() {
        bail!("TYPO3 Core is not cloned yet — ddev tryout download");
    }
    Ok(worktree::infos(&ctx, &php_versions(root)))
}

/// The branches a worktree can be based on. Fetches the branch list the first
/// time (the clone carries one branch), so off-thread.
pub fn load_branches(root: &Path) -> Result<Vec<String>> {
    let ctx = context(root);
    worktree::ensure_branch_refs_quiet(&ctx);
    Ok(ctx.local_core_branches())
}

/// One page of the open Gerrit changes on a site's branch that match `search`.
/// Asks Gerrit, so off-thread.
pub fn load_patches(root: &Path, name: &str, search: &str, page: u32) -> Result<PatchPage> {
    let ctx = context(root);
    let target = site::for_name(&ctx, name);
    let branch = if !target.is_empty() && !site::is_primary(&target) {
        worktree::detect_detached_base_branch(&site::core_dir(&ctx, &target))
    } else {
        ctx.branch().to_string()
    };
    let found = gerrit::search_open(&branch, search, page).map_err(|e| match e {
        gerrit::Error::Fetch => anyhow::anyhow!("Gerrit could not be reached"),
        gerrit::Error::Parse => anyhow::anyhow!("Gerrit's answer was not understood"),
    })?;
    let changes = found
        .changes
        .into_iter()
        .map(|c| crate::tui::forms::Change {
            number: c.number,
            subject: c.subject,
            owner: c.owner,
            scores: c.scores,
        })
        .collect();
    Ok((changes, found.more))
}

/// A page of open changes, and whether Gerrit has another.
pub type PatchPage = (Vec<crate::tui::forms::Change>, bool);

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
        std::fs::write(root.path().join(".ddev/tryout/VERSION"), "").unwrap();
        let deep = root.path().join("worktrees/v13/typo3/sysext");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(find_project_root(&deep).as_deref(), Some(root.path()));
    }
}
