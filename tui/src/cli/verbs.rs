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
