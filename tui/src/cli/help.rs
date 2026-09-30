//! The help texts.

use crate::core::out::{BOLD, DIM, NC};

use crate::core::kind::ProjectKind;

use super::verbs::VERBS;

/// Where the documentation lives.
pub const DOCS: &str = "https://bmack.github.io/tryout/";

/// The commands this kind of project has; TYPO3 Core's own notes only for it.
pub fn main(kind: &dyn ProjectKind) -> String {
    let what = if kind.opens_pull_requests() {
        "every branch of your project, served side by side"
    } else {
        "TYPO3 Core development toolkit"
    };
    let mut s = format!("\n{BOLD}ddev tryout{NC} — {what}\n\nCommands:\n");
    for v in VERBS.iter().filter(|v| kind.supports(v.name)) {
        let pad = " ".repeat(26usize.saturating_sub(v.usage.chars().count()));
        s.push_str(&format!("  {BOLD}{}{NC}{pad}{}\n", v.usage, v.help));
    }
    s.push_str(&format!(
        "\n  {DIM}worktree has its own help: ddev tryout worktree help{NC}\n\n"
    ));
    if kind.opens_pull_requests() {
        s.push_str("A pull request as its own site:\n    ddev tryout worktree add --pr 42\n\n");
    } else {
        s.push_str(
            "Custom extensions:\n  Place extensions in packages/ and run:\n    ddev composer require vendor/my-extension:@dev\n\n\
Gerrit patches (auto-apply on start):\n  Edit .ddev/config.tryout-patches.yaml:\n    TRYOUT_PATCHES=56947,12345\n\n",
        );
    }
    s.push_str(&format!("Documentation: {DOCS}\n\n"));
    s
}

pub fn worktree(kind: &dyn ProjectKind) -> String {
    let row = |cmd: &str, text: &str| {
        format!(
            "  {BOLD}{cmd}{NC}{}{text}\n",
            " ".repeat(24 - cmd.chars().count())
        )
    };
    let what = if kind.opens_pull_requests() {
        "side-by-side checkouts of your project"
    } else {
        "side-by-side Core checkouts"
    };
    let mut s = format!("\n{BOLD}ddev tryout worktree{NC} — {what}\n\n");
    for (c, t) in [
        (
            "add <name> [<branch>]",
            "Create worktrees/<name>, detached at origin/<branch>",
        ),
        ("list", "List worktrees and served sites"),
        (
            "use <name>",
            "Serve that worktree's Core at the project URL",
        ),
        (
            "serve <name>",
            "Give <name> its own URL, PHP version and database",
        ),
        ("unserve <name>", "Drop the site, keep the worktree"),
        (
            "remove <name>",
            "Delete a worktree and its directory (asks first)",
        ),
        (
            "rename <old> <new>",
            "Rename a checkout (the branch is untouched)",
        ),
        (
            "branches [--json]",
            "The branches a new worktree can be based on",
        ),
    ] {
        // `use` rewrites Core's overlay: a project has none.
        if c.starts_with("use ") && !kind.supports("worktree use") {
            continue;
        }
        s.push_str(&row(c, t));
    }
    s.push_str(
        "\n  Flags: --serve     (add: serve it immediately)\n\
\x20        --php 8.2   (add/serve: run this site on another PHP version)\n\
\x20        --db postgres:16 (add/serve: type[:version] — mariadb, mysql, postgres, sqlite)\n\
\x20        --switch    (serve --db: move a served site, its old database kept)\n\
\x20        --db-from x (add/serve, a project: copy the database of site x — default @primary)\n\
\x20        --db-empty  (add/serve, a project: start with an empty database)\n\
\x20        --pr 123    (add, a project: pull/merge request #123 of origin, served as pr-123)\n\
\x20        --force     (remove: also drop an unmerged branch)\n\
\x20        --yes       (remove: skip the confirmation)\n\
\x20        --drop-db   (unserve: also drop the site's database)\n\
\x20        --no-restart (add --serve/serve/unserve/rename/remove: skip the restart)\n\
\n  Adding or removing a served site restarts DDEV automatically, so\n\
\x20 it can register the hostname and issue its TLS certificate.\n\
\x20 Re-serving one that already exists (a --php change, say) applies\n\
\x20 right away with no restart. --no-restart skips it — useful when\n\
\x20 serving several worktrees, then restarting once at the end.\n\
\n  --db puts a site on another database server than the project's: a type,\n\
\x20 or a type at a version (postgres:16; a bare type is its newest). MariaDB,\n\
\x20 MySQL and Postgres get a server of their own per version, started for it\n\
\x20 (the first time with a restart) and stopped when no site or kept\n\
\x20 database needs it. SQLite is a file in the site, with no server at all.\n\
\n  All worktrees share one git object store, so ddev tryout cs hooks and\n\
\x20 Gerrit config are set up once and apply to every one of them.\n\n",
    );
    if kind.supports("worktree use") {
        s.push_str(
            "  Two ways to run several Cores:\n\
\x20   use    — one site at the project URL, switch which Core it serves\n\
\x20   serve  — every worktree live at once on <name>.<project>.ddev.site\n\n",
        );
    }
    s.push_str(&format!("  Documentation: {DOCS}reference/commands\n\n"));
    s
}

pub fn launch() -> String {
    format!(
        "\n{BOLD}ddev tryout launch{NC} — open a site in the browser\n\n\
\x20 {BOLD}launch{NC}                  The worktree you are in, or pick from a list\n\
\x20 {BOLD}launch <worktree>{NC}       That worktree's site\n\n\
\x20 Flags: --backend   open /typo3/ instead of the frontend\n\n"
    )
}

pub fn cs() -> String {
    format!(
        "\n{BOLD}ddev tryout cs{NC} — TYPO3 Core contribution setup\n\nCommands:\n\
\x20 {BOLD}setup [user]{NC}   Install hooks, template, and Gerrit push URL (default)\n\
\x20 {BOLD}doctor{NC}         Diagnose the current contribution setup\n\
\x20 {BOLD}uninstall{NC}      Remove hooks, template, and reset push URL\n\
\x20 {BOLD}help{NC}           Show this help\n\n\
Username resolution order:\n\
\x20 1. argument:   ddev tryout cs setup jdoe\n\
\x20 2. env:        TRYOUT_GERRIT_USER=jdoe\n\
\x20 3. git config: tryout.gerritUser (cached from previous setup)\n\
\x20 4. prompt      (interactive)\n\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::kind::{GENERIC_PROJECT, TYPO3_CORE};

    #[test]
    fn help_offers_what_this_kind_of_project_has() {
        let core = main(&TYPO3_CORE);
        assert!(core.contains("TYPO3 Core development toolkit"));
        assert!(core.contains("Gerrit patches") && core.contains(DOCS));

        let project = main(&GENERIC_PROJECT);
        assert!(project.contains("every branch of your project"));
        assert!(project.contains("--pr 42") && project.contains(DOCS));
        for core_only in ["Gerrit", "packages/", "ddev tryout cs", "ddev tryout patch"] {
            assert!(!project.contains(core_only), "{core_only}");
        }

        let wt = worktree(&GENERIC_PROJECT);
        assert!(!wt.contains("use <name>") && !wt.contains("Two ways"));
        assert!(worktree(&TYPO3_CORE).contains("use <name>"));
    }
}
