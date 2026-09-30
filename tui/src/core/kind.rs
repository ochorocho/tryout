//! What kind of project tryout works on, and what differs between kinds.
//!
//! **Core mode** is the TYPO3 Core checkout tryout was built for: the project
//! root is a Core clone, instances overlay its sysexts, changes come from
//! TYPO3's Gerrit. **Project mode** is a project of the user's own, of any DDEV
//! type — worktrees of its own repository, served side by side. Everything that
//! differs between them is asked of a `ProjectKind`; everything else (worktrees,
//! sites, database servers, the TUI) is the same for all.

use std::path::Path;

use super::ctx::{CORE_REPO, GERRIT_REMOTE};

/// Which of the two tryout works in, decided by what the project root holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A TYPO3 Core checkout — or nothing yet, since the install clones Core.
    Core,
    /// A repository of the user's own.
    Project,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Core => "TYPO3 Core",
            Mode::Project => "project",
        }
    }
}

/// The root is a Core checkout when it holds Core's sysexts, or its origin is
/// TYPO3's repository; with no repository at all, it is about to become one.
/// Anything else is a project of the user's own.
pub fn detect_mode(root: &Path) -> Mode {
    if !root.join(".git").exists() || root.join("typo3/sysext/core/composer.json").is_file() {
        return Mode::Core;
    }
    let origin = super::git::out(root, &["remote", "get-url", "origin"]).unwrap_or_default();
    let origin = origin.to_lowercase();
    if origin.contains("typo3/typo3") || origin.contains("packages/typo3.cms") {
        Mode::Core
    } else {
        Mode::Project
    }
}

/// What tryout needs to know about a kind of project.
pub trait ProjectKind: Sync {
    /// For messages: "TYPO3 Core".
    fn label(&self) -> &'static str;

    /// Where the root checkout comes from, when tryout clones it.
    fn clone_url(&self) -> Option<&'static str>;

    /// A second remote to add beside origin (the review system's), if any.
    fn review_remote(&self) -> Option<(&'static str, &'static str)>;

    /// The branch a fresh checkout starts on.
    fn default_branch(&self) -> &'static str;

    /// Branches a worktree can be based on.
    fn is_base_branch(&self, branch: &str) -> bool;

    /// What `.git/info/exclude` keeps out of `git status`: tryout's own files.
    fn git_excludes(&self) -> &'static [&'static str];

    /// The PHP versions a checkout accepts, as its composer.json says.
    fn php_constraint(&self, checkout: &Path) -> Option<String>;

    /// The command a fresh site is set up with (run in the site, its database
    /// settings passed in the environment).
    fn setup_command(&self, webserver_type: &str) -> Vec<String>;

    /// What runs in a site after every install or rebuild.
    fn rebuild_commands(&self) -> &'static [&'static [&'static str]];

    /// The site's admin, below its URL, when it has one.
    fn backend_path(&self) -> Option<&'static str>;

    /// Whether a verb applies to this kind (`patch`, `cs` … need Core).
    fn supports(&self, verb: &str) -> bool;
}

/// The one kind today: a TYPO3 Core checkout.
pub struct Typo3Core;

pub static TYPO3_CORE: Typo3Core = Typo3Core;

impl ProjectKind for Typo3Core {
    fn label(&self) -> &'static str {
        "TYPO3 Core"
    }

    fn clone_url(&self) -> Option<&'static str> {
        Some(CORE_REPO)
    }

    fn review_remote(&self) -> Option<(&'static str, &'static str)> {
        Some(("gerrit", GERRIT_REMOTE))
    }

    fn default_branch(&self) -> &'static str {
        "main"
    }

    /// `main` and the release branches, `<major>.<minor>`.
    fn is_base_branch(&self, branch: &str) -> bool {
        branch == "main" || super::worktree::is_release(branch)
    }

    fn git_excludes(&self) -> &'static [&'static str] {
        &["/.ddev/", "/worktrees/", "/TYPO3-Instances/", "/packages/"]
    }

    fn php_constraint(&self, checkout: &Path) -> Option<String> {
        super::php::core_constraint(&checkout.join("composer.json"))
    }

    fn setup_command(&self, webserver_type: &str) -> Vec<String> {
        let server_type = if webserver_type.starts_with("apache") || webserver_type.is_empty() {
            "apache"
        } else {
            "other"
        };
        ["vendor/bin/typo3", "setup", "--no-interaction", "--force"]
            .iter()
            .map(|s| s.to_string())
            .chain(std::iter::once(format!("--server-type={server_type}")))
            .collect()
    }

    fn rebuild_commands(&self) -> &'static [&'static [&'static str]] {
        &[
            &["vendor/bin/typo3", "extension:setup"],
            &["vendor/bin/typo3", "cache:flush"],
        ]
    }

    fn backend_path(&self) -> Option<&'static str> {
        Some("/typo3/")
    }

    fn supports(&self, _verb: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }

    #[test]
    fn a_core_checkout_or_nothing_yet_is_core_mode_and_any_other_repo_a_project() {
        // Nothing there: the install is about to clone Core into it.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(detect_mode(empty.path()), Mode::Core);

        // A Core checkout, recognised by its sysexts.
        let core = tempfile::tempdir().unwrap();
        git(core.path(), &["init", "-q"]);
        std::fs::create_dir_all(core.path().join("typo3/sysext/core")).unwrap();
        std::fs::write(core.path().join("typo3/sysext/core/composer.json"), "{}").unwrap();
        assert_eq!(detect_mode(core.path()), Mode::Core);

        // One not checked out yet, recognised by where it comes from.
        let fetched = tempfile::tempdir().unwrap();
        git(fetched.path(), &["init", "-q"]);
        git(fetched.path(), &["remote", "add", "origin", CORE_REPO]);
        assert_eq!(detect_mode(fetched.path()), Mode::Core);

        // Anything else is a project of the user's own.
        let own = tempfile::tempdir().unwrap();
        git(own.path(), &["init", "-q"]);
        git(
            own.path(),
            &["remote", "add", "origin", "git@github.com:acme/shop.git"],
        );
        assert_eq!(detect_mode(own.path()), Mode::Project);
    }

    #[test]
    fn typo3_core_answers_what_tryout_always_did() {
        let k: &dyn ProjectKind = &TYPO3_CORE;
        assert_eq!(k.clone_url(), Some("https://github.com/typo3/typo3.git"));
        assert_eq!(k.default_branch(), "main");
        assert!(k.is_base_branch("main") && k.is_base_branch("13.4"));
        assert!(!k.is_base_branch("feature/x"));
        assert_eq!(
            k.git_excludes(),
            ["/.ddev/", "/worktrees/", "/TYPO3-Instances/", "/packages/"]
        );
        assert_eq!(
            k.setup_command("nginx-fpm"),
            [
                "vendor/bin/typo3",
                "setup",
                "--no-interaction",
                "--force",
                "--server-type=other"
            ]
        );
        assert_eq!(
            k.setup_command("apache-fpm").last().unwrap(),
            "--server-type=apache"
        );
        assert_eq!(k.backend_path(), Some("/typo3/"));
        for verb in [
            "download", "checkout", "patch", "cs", "reset", "composer", "worktree",
        ] {
            assert!(k.supports(verb), "{verb}");
        }
    }
}
