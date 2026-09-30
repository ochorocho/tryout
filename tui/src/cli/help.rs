//! The help texts.

use crate::core::out::{BOLD, DIM, NC};

use crate::core::ctx::Ctx;

use super::verbs::{self, VERBS};

/// Where the documentation lives.
pub const DOCS: &str = "https://bmack.github.io/tryout/";

/// The commands this project has, in its own words, its own tool among them,
/// and examples for its type.
pub fn main(ctx: &Ctx) -> String {
    let kind = ctx.kind();
    let what = if kind.opens_pull_requests() {
        "every branch of your project, served side by side"
    } else {
        "TYPO3 Core development toolkit"
    };
    let mut s = format!("\n{BOLD}ddev tryout{NC} — {what}\n\nCommands:\n");
    let row = |usage: &str, help: &str| {
        let pad = " ".repeat(26usize.saturating_sub(usage.chars().count()));
        format!("  {BOLD}{usage}{NC}{pad}{help}\n")
    };
    for v in VERBS.iter().filter(|v| kind.supports(v.name)) {
        s.push_str(&row(v.usage, &verbs::help_for(v, ctx)));
    }
    if let Some(c) = ctx.cli() {
        s.push_str(&row(&format!("{} <site> <args>", c.verb), c.about));
    }
    s.push_str(&format!(
        "\n  {DIM}worktree has its own help: ddev tryout worktree help{NC}\n\nExamples:\n"
    ));
    for e in verbs::examples(ctx) {
        s.push_str(&format!("  {e}\n"));
    }
    s.push('\n');
    if !kind.opens_pull_requests() {
        s.push_str(
            "Custom extensions:\n  Place extensions in packages/ and run:\n    ddev composer require vendor/my-extension:@dev\n\n\
Gerrit patches (auto-apply on start):\n  Edit .ddev/config.tryout-patches.yaml:\n    TRYOUT_PATCHES=56947,12345\n\n",
        );
    }
    s.push_str(&format!("Documentation: {DOCS}\n\n"));
    s
}

pub fn worktree(ctx: &Ctx) -> String {
    let kind = ctx.kind();
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
    let project_flags = if kind.seeds_databases() {
        "\x20        --db-from x (add/serve: copy the database of site x — default @primary)\n\
\x20        --db-empty  (add/serve: start with an empty database)\n\
\x20        --pr 123    (add: pull/merge request #123 of origin, served as pr-123)\n"
    } else {
        ""
    };
    s.push_str(&format!(
        "\n  Flags: --serve     (add: serve it immediately)\n\
\x20        --php 8.2   (add/serve: run this site on another PHP version)\n\
\x20        --db postgres:16 (add/serve: type[:version] — mariadb, mysql, postgres, sqlite)\n\
\x20        --switch    (serve --db: move a served site, its old database kept)\n\
{project_flags}\x20        --force     (remove: also drop an unmerged branch)\n\
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
\n"
    ));
    if kind.supports("worktree use") {
        s.push_str(
            "  All worktrees share one git object store, so ddev tryout cs hooks and\n\
\x20 Gerrit config are set up once and apply to every one of them.\n\n\
\x20 Two ways to run several Cores:\n\
\x20   use    — one site at the project URL, switch which Core it serves\n\
\x20   serve  — every worktree live at once on <name>.<project>.ddev.site\n\n",
        );
    }
    s.push_str(&format!("  Documentation: {DOCS}reference/commands\n\n"));
    s
}

/// `.ddev/commands/host/tryout` for this project: DDEV shows its Description
/// and Example lines in `ddev help` and `ddev tryout -h`, so they are the
/// project type's. Everything else is the shipped shim, unchanged.
pub fn host_command(ctx: &Ctx) -> String {
    let what = if ctx.mode() == crate::core::kind::Mode::Core {
        "TYPO3 Core: worktrees, served sites, Gerrit patches".to_string()
    } else if ctx.env.project_type.is_empty() {
        "Every branch of this project, served side by side".to_string()
    } else {
        format!(
            "Every branch of this {} project, served side by side",
            ctx.env.project_type
        )
    };
    let examples = verbs::examples(ctx).join("\\n");
    format!(
        r#"#!/usr/bin/env bash
#ddev-silent-no-warn
#ddev-generated

## Description: {what}
## Usage: tryout [command] [args]
## Example: "{examples}"
# Description and Example are this project type's: tryout writes them at
# install and on every start (`tryout __host-command`).
# NO ## AutocompleteTerms: header on purpose. It sets cobra's ValidArgs, which
# then rejects any second argument during completion — so ValidArgsFunction,
# i.e. commands/host/autocomplete/tryout, is never called and
# `ddev tryout cs <TAB>` completes nothing. No ## Flags: either: it makes DDEV
# parse flags and reject the ones it does not know, like `--php 8.2`.

# Everything is the tryout binary (.ddev/tryout/tryout picks the build for this
# machine); this file only exists because DDEV discovers commands as files.
# Through bash, so a launcher that lost its executable bit still runs.
exec bash "${{DDEV_APPROOT}}/.ddev/tryout/tryout" "$@"
"#
    )
}

/// Write `host_command` into the project, when it says something new.
pub fn write_host_command(ctx: &Ctx) -> std::io::Result<bool> {
    let file = ctx.root.join(".ddev/commands/host/tryout");
    let want = host_command(ctx);
    if std::fs::read_to_string(&file).is_ok_and(|have| have == want) {
        return Ok(false);
    }
    std::fs::create_dir_all(file.parent().expect("has a directory"))?;
    std::fs::write(&file, want)?;
    Ok(true)
}

pub fn launch(ctx: &Ctx) -> String {
    let flags = match ctx.backend_path() {
        Some(p) => format!("  Flags: --backend   open {p} instead of the site's home page\n\n"),
        None => String::new(),
    };
    format!(
        "\n{BOLD}ddev tryout launch{NC} — open a site in the browser\n\n\
\x20 {BOLD}launch{NC}                  The worktree you are in, or pick from a list\n\
\x20 {BOLD}launch <worktree>{NC}       That worktree's site\n\n{flags}"
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
    use crate::core::ctx::{DdevEnv, tests::core_repo, tests::project_repo};

    fn project(t: &str) -> (tempfile::TempDir, Ctx) {
        let d = project_repo();
        let c = Ctx::new(
            d.path(),
            DdevEnv {
                project_type: t.into(),
                ..DdevEnv::default()
            },
        );
        (d, c)
    }

    #[test]
    fn a_drupal_project_is_offered_drupals_commands_and_none_of_typo3s() {
        let (_d, c) = project("drupal11");
        let h = main(&c);
        assert!(h.contains("every branch of your project"));
        assert!(h.contains("drush <site> <args>"), "{h}");
        assert!(h.contains("ddev tryout drush feature-x status"), "{h}");
        assert!(h.contains("--backend for /user/login"), "{h}");
        assert!(h.contains("--pr 42") && h.contains(DOCS));
        for core_only in [
            "Gerrit",
            "packages/",
            "ddev tryout cs",
            "ddev tryout patch",
            "/typo3/",
            "fileadmin",
            "ddev tryout checkout",
            "ddev tryout download",
            "TYPO3 Core",
        ] {
            assert!(!h.contains(core_only), "{core_only}: {h}");
        }
        let wt = worktree(&c);
        assert!(!wt.contains("use <name>") && !wt.contains("Gerrit") && !wt.contains("Two ways"));
        assert!(wt.contains("--db-from"));
        assert!(launch(&c).contains("/user/login"));
    }

    #[test]
    fn a_laravel_project_gets_artisan_and_no_admin_to_open() {
        let (_d, c) = project("laravel");
        let h = main(&c);
        assert!(h.contains("artisan <site> <args>") && h.contains("artisan feature-x migrate"));
        assert!(
            !h.contains("--backend") && !launch(&c).contains("--backend"),
            "{h}"
        );
    }

    #[test]
    fn ddev_help_describes_tryout_for_this_project_type() {
        let (d, c) = project("drupal11");
        let f = host_command(&c);
        assert!(f.contains("## Description: Every branch of this drupal11 project"));
        assert!(
            f.contains("ddev tryout drush feature-x status\\nddev tryout worktree add --pr 42")
        );
        assert!(!f.contains("patch 56947"));
        // DDEV's rules for the file stay as they were.
        assert!(f.starts_with("#!/usr/bin/env bash\n#ddev-silent-no-warn\n#ddev-generated\n"));
        assert!(!f.contains("\n## AutocompleteTerms") && !f.contains("\n## Flags"));
        assert!(f.ends_with("exec bash \"${DDEV_APPROOT}/.ddev/tryout/tryout\" \"$@\"\n"));
        // Written once; a second write finds nothing new.
        assert!(write_host_command(&c).unwrap());
        assert!(!write_host_command(&c).unwrap());
        assert_eq!(
            std::fs::read_to_string(d.path().join(".ddev/commands/host/tryout")).unwrap(),
            f
        );

        let core = core_repo();
        let f = host_command(&Ctx::new(core.path(), DdevEnv::default()));
        assert!(f.contains("## Description: TYPO3 Core") && f.contains("patch 56947"));
    }

    #[test]
    fn typo3_core_keeps_its_commands_and_gains_its_console() {
        let d = core_repo();
        let c = Ctx::new(d.path(), DdevEnv::default());
        let h = main(&c);
        assert!(h.contains("TYPO3 Core development toolkit"));
        assert!(h.contains("Gerrit patches") && h.contains("ddev tryout patch 56947"));
        assert!(
            h.contains("typo3 <site> <args>") && h.contains("ddev tryout typo3 v13 cache:flush")
        );
        assert!(h.contains("--backend for /typo3/"));
        let wt = worktree(&c);
        assert!(wt.contains("use <name>") && wt.contains("Two ways"));
        assert!(!wt.contains("--db-from"));
    }
}
