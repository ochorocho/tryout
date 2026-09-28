//! Where everything is: the project root (which IS the TYPO3 Core clone), its
//! instances and worktrees, the DDEV environment, and the branch in play.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::{git, vsort, worktree};

pub const CORE_REPO: &str = "https://github.com/typo3/typo3.git";
pub const GERRIT_REMOTE: &str = "https://review.typo3.org/Packages/TYPO3.CMS";
pub const GERRIT_API: &str = "https://review.typo3.org";
pub const GERRIT_URL: &str = "https://review.typo3.org/c/Packages/TYPO3.CMS/+/";
pub const GERRIT_SSH_HOST: &str = "review.typo3.org";
pub const GERRIT_SSH_PORT: u16 = 29418;
pub const GERRIT_PROJECT: &str = "Packages/TYPO3.CMS";
/// The instance served at the project URL, and its sentinel in site arguments.
pub const PRIMARY_INSTANCE: &str = "primary";
pub const PRIMARY_SITE: &str = "@primary";
pub const DEFAULT_CORE_WORKTREE: &str = "main";
/// Where the container sees the project.
pub const CONTAINER_ROOT: &str = "/var/www/html";

/// What DDEV tells a command about the project. Host commands get these from
/// DDEV; the completion path gets none of them.
#[derive(Clone, Debug, Default)]
pub struct DdevEnv {
    pub sitename: String,
    pub primary_url: String,
    pub php_version: String,
    pub webserver_type: String,
    pub database: String,
    pub mutagen_enabled: bool,
}

impl DdevEnv {
    pub fn from_env() -> Self {
        let v = |k: &str| std::env::var(k).unwrap_or_default();
        Self {
            sitename: v("DDEV_SITENAME"),
            primary_url: v("DDEV_PRIMARY_URL"),
            php_version: v("DDEV_PHP_VERSION"),
            webserver_type: v("DDEV_WEBSERVER_TYPE"),
            database: v("DDEV_DATABASE"),
            mutagen_enabled: v("DDEV_MUTAGEN_ENABLED") == "true",
        }
    }

    pub fn is_nginx(&self) -> bool {
        self.webserver_type.starts_with("nginx")
    }

    pub fn is_postgres(&self) -> bool {
        self.database.starts_with("postgres")
    }
}

pub struct Ctx {
    pub root: PathBuf,
    pub env: DdevEnv,
    pub in_container: bool,
    /// TRYOUT_BRANCH, when the caller set one.
    pub branch_override: Option<String>,
    branch: OnceLock<String>,
}

impl Ctx {
    pub fn new(root: impl Into<PathBuf>, env: DdevEnv) -> Self {
        Self {
            root: root.into(),
            env,
            in_container: false,
            branch_override: None,
            branch: OnceLock::new(),
        }
    }

    /// From the process environment: DDEV_APPROOT, the DDEV_* values, and
    /// TRYOUT_BRANCH / TRYOUT_IN_CONTAINER.
    pub fn from_env(root: impl Into<PathBuf>) -> Self {
        let mut c = Self::new(root, DdevEnv::from_env());
        c.in_container = std::env::var("TRYOUT_IN_CONTAINER").as_deref() == Ok("1");
        c.branch_override = std::env::var("TRYOUT_BRANCH")
            .ok()
            .filter(|b| !b.is_empty());
        c
    }

    pub fn core_dir(&self) -> &Path {
        &self.root
    }

    /// The git dir hooks and config live in. In a linked worktree `.git` is a
    /// file, so this resolves to the common dir.
    pub fn core_git_dir(&self) -> PathBuf {
        let dot = self.root.join(".git");
        if dot.is_file()
            && let Some(d) = git::out(
                &self.root,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            )
        {
            return PathBuf::from(d);
        }
        dot
    }

    pub fn has_core(&self) -> bool {
        self.root.join(".git").exists()
    }

    /// Instances live under TYPO3-Instances/, never the root (Core's source) or
    /// Build/ (Core's own build tooling).
    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("TYPO3-Instances")
    }

    pub fn instance_dir(&self) -> PathBuf {
        self.instances_dir().join(PRIMARY_INSTANCE)
    }

    pub fn worktrees_dir(&self) -> PathBuf {
        self.root.join("worktrees")
    }

    pub fn core_worktree_dir(&self, name: &str) -> PathBuf {
        self.worktrees_dir().join(name)
    }

    pub fn worktree_config(&self) -> PathBuf {
        self.root.join(".ddev/config.worktrees.yaml")
    }

    pub fn tryout_dir(&self) -> PathBuf {
        self.root.join(".ddev/tryout")
    }

    /// The branch in play: TRYOUT_BRANCH, else the root's branch, else — on a
    /// detached HEAD — the branch it was based on, else main.
    pub fn branch(&self) -> &str {
        self.branch.get_or_init(|| {
            if let Some(b) = &self.branch_override {
                return b.clone();
            }
            if !self.has_core() {
                return DEFAULT_CORE_WORKTREE.into();
            }
            match git::out(&self.root, &["branch", "--show-current"]) {
                Some(b) if !b.is_empty() => b,
                _ => worktree::detect_detached_base_branch(&self.root),
            }
        })
    }

    /// The name the ROOT checkout goes by: its branch, or the default when it is
    /// detached or the branch is not a usable name.
    pub fn plain_core_name(&self) -> String {
        match git::out(&self.root, &["branch", "--show-current"]) {
            Some(b) if !b.is_empty() && worktree::validate_name(&b).is_ok() => b,
            _ => DEFAULT_CORE_WORKTREE.into(),
        }
    }

    /// Where a checkout name lives: worktrees/<name>, or the root for the primary.
    pub fn core_checkout_dir(&self, name: &str) -> PathBuf {
        let dir = self.core_worktree_dir(name);
        if !dir.is_dir() && name == self.plain_core_name() {
            self.root.clone()
        } else {
            dir
        }
    }

    /// Which checkout the PRIMARY instance serves — read from its overlay, which
    /// `worktree use` moves; the root's branch would keep answering for the root.
    pub fn active_worktree_name(&self) -> String {
        let overlay = std::fs::read_to_string(self.instance_dir().join("composer.tryout.json"))
            .unwrap_or_default();
        overlay_worktree(&overlay).unwrap_or_else(|| self.plain_core_name())
    }

    /// The checkout the primary instance currently serves.
    pub fn active_core_dir(&self) -> PathBuf {
        self.core_checkout_dir(&self.active_worktree_name())
    }

    /// Local branches of origin, version-sorted — what completion and the
    /// pickers offer; never touches the network.
    pub fn local_core_branches(&self) -> Vec<String> {
        let mut v: Vec<String> = git::lines(
            &self.root,
            &[
                "for-each-ref",
                "--format=%(refname:strip=3)",
                "refs/remotes/origin",
            ],
        )
        .into_iter()
        .filter(|b| b != "HEAD")
        .collect();
        vsort::sort(&mut v);
        v
    }
}

/// The worktree named by the first `../../[worktrees/<n>/]typo3/sysext` path in
/// an overlay; None when it names the root (or there is none).
fn overlay_worktree(overlay: &str) -> Option<String> {
    let mut rest = overlay;
    while let Some(i) = rest.find("../../") {
        let after = &rest[i + 6..];
        if after.starts_with("typo3/sysext") {
            return None;
        }
        if let Some(wt) = after.strip_prefix("worktrees/") {
            let end = wt
                .find(|c: char| !(c.is_ascii_alphanumeric() || "._-".contains(c)))
                .unwrap_or(wt.len());
            if end > 0 && wt[end..].starts_with("/typo3/sysext") {
                return Some(wt[..end].to_string());
            }
        }
        // Overlapping starts count, as they do for grep.
        rest = &rest[i + 1..];
    }
    None
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::process::Command;

    /// A Core-shaped git repo: one commit on main, origin branches main, 13.4 and
    /// 12.4, and origin/HEAD.
    pub fn core_repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let g = |args: &[&str]| {
            let s = Command::new("git")
                .args([
                    "-c",
                    "init.defaultBranch=main",
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                ])
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                s.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&s.stderr)
            );
        };
        g(&["init", "-q"]);
        std::fs::write(root.join("composer.json"), r#"{"require":{"php":"^8.2"}}"#).unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-qm", "init"]);
        for b in ["main", "13.4", "12.4"] {
            g(&["update-ref", &format!("refs/remotes/origin/{b}"), "HEAD"]);
        }
        g(&[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ]);
        d
    }

    #[test]
    fn the_branch_falls_back_to_main_with_no_core() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(Ctx::new(d.path(), DdevEnv::default()).branch(), "main");
    }

    #[test]
    fn tryout_branch_overrides_the_detected_branch() {
        let d = core_repo();
        let mut c = Ctx::new(d.path(), DdevEnv::default());
        c.branch_override = Some("13.4".into());
        assert_eq!(c.branch(), "13.4");
        assert_eq!(Ctx::new(d.path(), DdevEnv::default()).branch(), "main");
    }

    #[test]
    fn the_primary_core_is_the_root_a_named_one_its_worktree() {
        let d = core_repo();
        let c = Ctx::new(d.path(), DdevEnv::default());
        assert_eq!(c.core_checkout_dir("main"), d.path());
        assert_eq!(
            c.core_checkout_dir("other"),
            d.path().join("worktrees/other")
        );
    }

    #[test]
    fn the_active_core_comes_from_the_primary_overlay() {
        assert_eq!(
            overlay_worktree(r#""url": "../../worktrees/v13/typo3/sysext/*""#).as_deref(),
            Some("v13")
        );
        assert_eq!(overlay_worktree(r#""url": "../../typo3/sysext/*""#), None);
        assert_eq!(
            overlay_worktree(
                r#""url": "../../packages/*", "url": "../../worktrees/x/typo3/sysext/*""#
            )
            .as_deref(),
            Some("x")
        );
        assert_eq!(
            overlay_worktree("../../../worktrees/x/typo3/sysext").as_deref(),
            Some("x")
        );
        let d = core_repo();
        let c = Ctx::new(d.path(), DdevEnv::default());
        assert_eq!(c.active_worktree_name(), "main");
        std::fs::create_dir_all(c.instance_dir()).unwrap();
        std::fs::write(
            c.instance_dir().join("composer.tryout.json"),
            r#"{"url": "../../worktrees/v13/typo3/sysext/*"}"#,
        )
        .unwrap();
        assert_eq!(c.active_worktree_name(), "v13");
    }

    #[test]
    fn local_branches_are_version_sorted_without_head() {
        let d = core_repo();
        let c = Ctx::new(d.path(), DdevEnv::default());
        assert_eq!(c.local_core_branches(), ["12.4", "13.4", "main"]);
    }
}
