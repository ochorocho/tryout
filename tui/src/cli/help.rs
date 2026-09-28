//! The help texts, word for word what the bash command printed.

use crate::core::out::{BOLD, DIM, NC};

use super::verbs::VERBS;

pub fn main() -> String {
    let mut s = format!("\n{BOLD}ddev tryout{NC} — TYPO3 development toolkit\n\nCommands:\n");
    for v in VERBS {
        let pad = " ".repeat(26usize.saturating_sub(v.usage.chars().count()));
        s.push_str(&format!("  {BOLD}{}{NC}{pad}{}\n", v.usage, v.help));
    }
    s.push_str(&format!(
        "\n  {DIM}worktree has its own help: ddev tryout worktree help{NC}\n\n\
Custom extensions:\n  Place extensions in packages/ and run:\n    ddev composer require vendor/my-extension:@dev\n\n\
Gerrit patches (auto-apply on start):\n  Edit .ddev/config.tryout-patches.yaml:\n    TRYOUT_PATCHES=56947,12345\n\n"
    ));
    s
}

pub fn worktree() -> String {
    let row = |cmd: &str, text: &str| {
        format!(
            "  {BOLD}{cmd}{NC}{}{text}\n",
            " ".repeat(24 - cmd.chars().count())
        )
    };
    let mut s = format!("\n{BOLD}ddev tryout worktree{NC} — side-by-side Core checkouts\n\n");
    for (c, t) in [
        (
            "add <name> [<branch>]",
            "Create worktrees/<name> on a branch <name>, off origin/<branch>",
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
        s.push_str(&row(c, t));
    }
    s.push_str(
        "\n  Flags: --serve     (add: serve it immediately)\n\
\x20        --php 8.2   (add/serve: run this site on another PHP version)\n\
\x20        --force     (remove: also drop an unmerged branch)\n\
\x20        --yes       (remove: skip the confirmation)\n\
\x20        --drop-db   (unserve: also drop the site's database)\n\
\x20        --no-restart (add --serve/serve/unserve/rename: skip the restart)\n\
\n  Two ways to run several Cores:\n\
\x20   use    — one site at the project URL, switch which Core it serves\n\
\x20   serve  — every worktree live at once on <name>.<project>.ddev.site\n\
\n  Adding or removing a served site restarts DDEV automatically, so\n\
\x20 it can register the hostname and issue its TLS certificate.\n\
\x20 Re-serving one that already exists (a --php change, say) applies\n\
\x20 right away with no restart. --no-restart skips it — useful when\n\
\x20 serving several worktrees, then restarting once at the end.\n\
\n  All worktrees share one git object store, so ddev tryout cs hooks and\n\
\x20 Gerrit config are set up once and apply to every one of them.\n\n",
    );
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
