//! Every verb `ddev tryout` knows, in one table: dispatch on both sides, the
//! help and the completion all come from here, so a verb cannot be registered
//! in one place and forgotten in another.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Status,
    Download,
    Checkout,
    Composer,
    Patch,
    Worktree,
    Cs,
    Exec,
    Launch,
    Ui,
    Reset,
    Delete,
    Help,
}

pub struct Spec {
    pub verb: Verb,
    pub name: &'static str,
    /// The left column of `help`.
    pub usage: &'static str,
    pub help: &'static str,
    /// The description a TAB shows.
    pub complete: &'static str,
    /// Runs on the host alone: no browser, terminal or help text in the container.
    pub host_only: bool,
}

pub const VERBS: &[Spec] = &[
    spec(
        Verb::Status,
        "status",
        "status",
        "Show project overview",
        "Show project overview",
        false,
    ),
    spec(
        Verb::Download,
        "download",
        "download [--reset]",
        "Clone or update Core (--reset: hard reset)",
        "Clone or update Core",
        false,
    ),
    spec(
        Verb::Checkout,
        "checkout",
        "checkout <branch>",
        "Switch TYPO3 version (main, 13.4, 12.4, ...)",
        "Switch TYPO3 version (main, 13.4, 12.4, ...)",
        false,
    ),
    spec(
        Verb::Composer,
        "composer",
        "composer",
        "Regenerate the overlay (not: run Composer)",
        "Regenerate the Composer overlay",
        false,
    ),
    spec(
        Verb::Patch,
        "patch",
        "patch [<id>] [<site>]",
        "Apply a Gerrit patch — bare, browse and pick",
        "Apply one Gerrit patch, or all from config",
        false,
    ),
    spec(
        Verb::Worktree,
        "worktree",
        "worktree",
        "Manage side-by-side Core checkouts",
        "Manage side-by-side Core checkouts",
        false,
    ),
    spec(
        Verb::Cs,
        "cs",
        "cs [setup|doctor]",
        "Prepare this instance for Core contribution",
        "Prepare this instance for Core contribution",
        false,
    ),
    spec(
        Verb::Exec,
        "exec",
        "exec <site> <cmd>",
        "Run a command in a site's PHP/root/database",
        "Run a command in a site's PHP/root/database",
        false,
    ),
    spec(
        Verb::Launch,
        "launch",
        "launch [<worktree>]",
        "Open a site in the browser (--backend for /typo3/)",
        "Open a site in the browser",
        true,
    ),
    spec(
        Verb::Ui,
        "ui",
        "ui [stop]",
        "Terminal UI session: attach (q detaches), or stop it",
        "Terminal UI session: attach, or stop it",
        true,
    ),
    spec(
        Verb::Reset,
        "reset",
        "reset [<site>]",
        "Reset Core to current branch + rebuild",
        "Reset Core to current branch + rebuild",
        false,
    ),
    spec(
        Verb::Delete,
        "delete",
        "delete [<site>]",
        "Wipe DB + fileadmin, fresh setup (--all for every site)",
        "Wipe DB + fileadmin, fresh setup",
        false,
    ),
    spec(
        Verb::Help,
        "help",
        "help",
        "Show this help",
        "Show help",
        true,
    ),
];

const fn spec(
    verb: Verb,
    name: &'static str,
    usage: &'static str,
    help: &'static str,
    complete: &'static str,
    host_only: bool,
) -> Spec {
    Spec {
        verb,
        name,
        usage,
        help,
        complete,
        host_only,
    }
}

pub fn find(name: &str) -> Option<&'static Spec> {
    VERBS.iter().find(|s| s.name == name)
}

/// A verb's help line for this project: the table's, or its project-mode
/// wording where the table's speaks of TYPO3 Core.
pub fn help_for(spec: &Spec, ctx: &crate::core::ctx::Ctx) -> String {
    let project = ctx.mode() == crate::core::kind::Mode::Project;
    match spec.verb {
        Verb::Launch => match ctx.backend_path() {
            Some(p) => format!("Open a site in the browser (--backend for {p})"),
            None => "Open a site in the browser".into(),
        },
        Verb::Worktree if project => "Manage side-by-side checkouts, each its own site".into(),
        Verb::Delete if project => {
            "Reset a site's database to a fresh copy of the primary's (--all for every site)".into()
        }
        _ => spec.help.into(),
    }
}

/// A verb's TAB description for this project, like `help_for`.
pub fn complete_for(spec: &Spec, ctx: &crate::core::ctx::Ctx) -> String {
    let project = ctx.mode() == crate::core::kind::Mode::Project;
    match spec.verb {
        Verb::Worktree if project => "Manage side-by-side checkouts".into(),
        Verb::Delete if project => "Reset a site's database to a fresh copy".into(),
        _ => spec.complete.into(),
    }
}

/// What to show as examples: this project type's typical commands.
pub fn examples(ctx: &crate::core::ctx::Ctx) -> Vec<String> {
    let backend = if ctx.backend_path().is_some() {
        " --backend"
    } else {
        ""
    };
    let mut e = Vec::new();
    if ctx.mode() == crate::core::kind::Mode::Core {
        e.push("ddev tryout status".to_string());
        e.push("ddev tryout worktree add v13 13.4 --serve".to_string());
        e.push("ddev tryout patch 56947".to_string());
        e.push("ddev tryout checkout 13.4".to_string());
        if let Some(c) = ctx.cli() {
            e.push(format!("ddev tryout {} v13 {}", c.verb, c.example));
        }
        e.push(format!("ddev tryout launch v13{backend}"));
        return e;
    }
    e.push("ddev tryout worktree add feature-x feature/x --serve".to_string());
    if let Some(c) = ctx.cli() {
        e.push(format!("ddev tryout {} feature-x {}", c.verb, c.example));
    }
    e.push("ddev tryout worktree add --pr 42".to_string());
    e.push(format!("ddev tryout launch feature-x{backend}"));
    e.push("ddev tryout delete feature-x".to_string());
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_verb_is_in_the_table_once() {
        let all = [
            Verb::Status,
            Verb::Download,
            Verb::Checkout,
            Verb::Composer,
            Verb::Patch,
            Verb::Worktree,
            Verb::Cs,
            Verb::Exec,
            Verb::Launch,
            Verb::Ui,
            Verb::Reset,
            Verb::Delete,
            Verb::Help,
        ];
        for v in all {
            assert_eq!(VERBS.iter().filter(|s| s.verb == v).count(), 1, "{v:?}");
        }
        assert_eq!(VERBS.len(), all.len());
    }
}
