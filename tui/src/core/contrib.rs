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
            .is_some_and(|rest| rest.contains(&format!("@{GERRIT_SSH_HOST}")))
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
