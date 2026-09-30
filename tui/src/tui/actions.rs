//! The tryout commands offered for a worktree. An entry never offers something
//! that cannot work there, and every command names its own worktree — a bare verb acts
//! on whichever Core is primary at the moment it runs, not the one you selected.

use crate::core::db::{Db, Engine};
use crate::core::kind::ProjectKind;
use crate::tui::forms::FormKind;
use crate::tui::worktrees::Worktree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// No terminal, no popup: fast, silent on success. Only `launch` — it raises
    /// the browser, and a popup would just sit in front of it.
    Background,
    /// A queued job in the Activity panel: fully argued, nothing to answer.
    /// `reveal`: the output is the point, so its log opens when it is done.
    Job { reveal: bool },
    /// Questions first — a native form — and then a job with the answers.
    Form(FormKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub label: String,
    pub hint: String,
    /// Arguments after `ddev tryout`.
    pub args: Vec<String>,
    pub run: Run,
}

impl Action {
    fn new(label: impl Into<String>, hint: impl Into<String>, args: &str, run: Run) -> Self {
        Self {
            label: label.into(),
            hint: hint.into(),
            args: args.split_whitespace().map(String::from).collect(),
            run,
        }
    }

    #[cfg(test)]
    pub fn command_line(&self) -> String {
        format!("ddev tryout {}", self.args.join(" "))
    }

    /// Worktree verbs change what the list shows, so it reloads after them.
    pub fn changes_worktrees(&self) -> bool {
        matches!(
            self.args.first().map(String::as_str),
            Some("worktree" | "checkout" | "patch" | "download" | "reset")
        )
    }
}

/// What a job needs to itself while it runs, so jobs that need different
/// things run side by side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    /// The whole project. Anything that can `ddev restart` (which ends every
    /// running `ddev exec`), rewrites the webserver config, or writes the
    /// primary's overlay; and anything not known here.
    Alone,
    /// One worktree: its checkout, its site's vendor/ and its database.
    Site(String),
    /// Reads only.
    Free,
}

impl Claim {
    /// The claim of `ddev tryout <args>`.
    pub fn of(args: &[String]) -> Self {
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let site = |n: Option<&&str>| match n {
            Some(n) if !n.starts_with('-') => Claim::Site(n.to_string()),
            _ => Claim::Alone,
        };
        // `--site <name>`, where the verb takes it that way.
        let flagged = || {
            site(
                a.iter()
                    .position(|x| *x == "--site")
                    .and_then(|i| a.get(i + 1)),
            )
        };
        match a.as_slice() {
            ["status", ..] => Claim::Free,
            ["patch" | "checkout", ..] => flagged(),
            ["reset" | "download" | "delete" | "exec", rest @ ..] => site(rest.first()),
            // Adding touches only the new checkout — unless it serves it too.
            ["worktree", "add", rest @ ..]
                if !rest.iter().any(|x| {
                    ["--serve", "--php", "--db"]
                        .iter()
                        .any(|f| x.starts_with(f))
                }) =>
            {
                site(rest.first())
            }
            _ => Claim::Alone,
        }
    }

    /// Whether the two may not run at the same time.
    pub fn conflicts(&self, other: &Claim) -> bool {
        match (self, other) {
            (Claim::Alone, _) | (_, Claim::Alone) => true,
            (Claim::Site(a), Claim::Site(b)) => a == b,
            _ => false,
        }
    }
}

/// One line of a menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Action(Action),
    /// A `▸` row that opens a second level — the PHP versions.
    Sub {
        label: String,
        hint: String,
        items: Vec<Action>,
    },
    /// A thin line between groups; never selected.
    Separator,
}

impl Entry {
    pub fn label(&self) -> &str {
        match self {
            Entry::Action(a) => &a.label,
            Entry::Sub { label, .. } => label,
            Entry::Separator => "",
        }
    }

    pub fn hint(&self) -> &str {
        match self {
            Entry::Action(a) => &a.hint,
            Entry::Sub { hint, .. } => hint,
            Entry::Separator => "",
        }
    }

    pub fn selectable(&self) -> bool {
        !matches!(self, Entry::Separator)
    }
}

/// Needs no answers: a job in the Activity panel.
const JOB: Run = Run::Job { reveal: false };

/// The PHP versions to switch a site to, the running one ticked. `serve` with
/// `--php` re-serves in place — no DDEV restart — so a switch is instant.
fn php_items(w: &Worktree) -> Vec<Action> {
    w.php_versions
        .iter()
        .map(|v| {
            let current = w.php.as_deref() == Some(v.as_str());
            Action::new(
                format!("{} PHP {v}", if current { "✓" } else { " " }),
                if current { "running now" } else { "" },
                &format!("worktree serve {} --php {v}", w.name),
                JOB,
            )
        })
        .collect()
}

/// The database servers to serve a site on — every type at every version,
/// grouped by type — with `current` ticked. The project's own server and the
/// current one are there even when their version is not one offered. For a
/// served site the others move it (`--switch`), keeping its old database.
fn db_items(w: &Worktree, project: &Db, current: Option<&Db>) -> Vec<Action> {
    let mut list = Db::choices();
    for extra in [Some(project), current].into_iter().flatten() {
        if !list.contains(extra) {
            let at = list
                .iter()
                .position(|d| d.engine == extra.engine)
                .unwrap_or(list.len());
            list.insert(at, extra.clone());
        }
    }
    list.iter()
        .map(|d| {
            let ticked = current == Some(d);
            let mut hint = if ticked {
                "running now".to_string()
            } else if d == project {
                "the project's own server".to_string()
            } else if d.engine == Engine::Sqlite {
                "a file in the site, no server".to_string()
            } else {
                "its own server".to_string()
            };
            if d.typo3_note().is_some() {
                hint.push_str(" · TYPO3 lists ≤ 10.x");
            }
            let switch = if current.is_some() && !ticked {
                " --switch"
            } else {
                ""
            };
            Action::new(
                format!("{} {}", if ticked { "✓" } else { " " }, d.label()),
                hint,
                &format!("worktree serve {} --db {}{switch}", w.name, d.name()),
                JOB,
            )
        })
        .collect()
}

/// Everything that can be done to one worktree, grouped: its site, its Core,
/// its data, the checkout itself. What a group cannot do in this worktree's
/// state is not offered at all.
/// `project` is the project's own database type.
pub fn for_worktree(w: &Worktree, project: &Db) -> Vec<Entry> {
    let n = &w.name;
    let served = w.served();
    // The origin clone owns the object store every worktree branches from;
    // tryout refuses to rename or remove it.
    let origin = w.dir == ".";
    let act = |label: &str, hint: &str, args: String, run: Run| {
        Entry::Action(Action::new(label, hint, &args, run))
    };

    let mut site = Vec::new();
    if !served {
        site.push(act(
            "Serve",
            "best PHP it accepts",
            format!("worktree serve {n}"),
            JOB,
        ));
        if !w.php_versions.is_empty() {
            site.push(Entry::Sub {
                label: "Serve on PHP".into(),
                hint: String::new(),
                items: php_items(w),
            });
        }
        site.push(Entry::Sub {
            label: "Serve on database".into(),
            hint: String::new(),
            items: db_items(w, project, None),
        });
    }
    // The primary runs on the project's own PHP (ddev config), not a serve.
    if served && !w.primary && !w.php_versions.is_empty() {
        site.push(Entry::Sub {
            label: format!("PHP {}", w.php.as_deref().unwrap_or("?")),
            hint: "applies at once".into(),
            items: php_items(w),
        });
    }
    // Its database type, the same way; the primary's is the project's.
    if served && !w.primary {
        let current = w
            .db_engine
            .as_deref()
            .and_then(Db::parse_recorded)
            .unwrap_or_else(|| project.clone());
        site.push(Entry::Sub {
            label: format!("Database: {}", current.label()),
            hint: "switch type or version".into(),
            items: db_items(w, project, Some(&current)),
        });
    }
    if served {
        site.push(act(
            "Open site",
            "in the browser",
            format!("launch {n}"),
            Run::Background,
        ));
        site.push(act(
            "Open backend",
            "/typo3/ in the browser",
            format!("launch {n} --backend"),
            Run::Background,
        ));
    }
    if !w.primary {
        site.push(act(
            "Make primary",
            "at the project URL",
            format!("worktree use {n}"),
            JOB,
        ));
    }
    // The primary has no site of its own to take away.
    if served && !w.primary {
        site.push(act(
            "Unserve",
            "keeps its database",
            format!("worktree unserve {n}"),
            JOB,
        ));
        site.push(act(
            "Unserve and drop its database",
            "",
            format!("worktree unserve {n} --drop-db"),
            Run::Form(FormKind::DropDb(n.clone())),
        ));
    }

    let mut core = Vec::new();
    let mut data = Vec::new();
    if served {
        core.push(act(
            "Update from its base branch",
            "",
            format!("download {n}"),
            JOB,
        ));
        core.push(act(
            "Switch TYPO3 version…",
            "",
            format!("checkout --site {n}"),
            Run::Form(FormKind::Checkout(n.clone())),
        ));
        core.push(act(
            "Apply Gerrit patch…",
            "browse open changes",
            format!("patch --site {n}"),
            Run::Form(FormKind::Patch(n.clone())),
        ));
        core.push(act(
            "Reset Core + rebuild",
            "",
            format!("reset {n}"),
            Run::Form(FormKind::Reset(n.clone())),
        ));
        data.push(act(
            "Run command…",
            "in its PHP and DB",
            format!("exec {n}"),
            Run::Form(FormKind::Exec(n.clone())),
        ));
        data.push(act(
            "Fresh install…",
            "wipes DB + fileadmin",
            format!("delete {n}"),
            Run::Form(FormKind::FreshInstall(n.clone())),
        ));
    }

    let mut checkout = Vec::new();
    if !origin {
        checkout.push(act(
            "Rename…",
            "branch untouched",
            format!("worktree rename {n}"),
            Run::Form(FormKind::Rename(n.clone())),
        ));
        checkout.push(act(
            "Remove…",
            "deletes the checkout",
            format!("worktree remove {n}"),
            Run::Form(FormKind::Remove(n.clone())),
        ));
    }

    join([site, core, data, checkout])
}

/// The project-wide commands: not about any one worktree, so `a` adds them
/// below the worktree's own, in both the keyboard and the right-click menu.
pub fn project() -> Vec<Entry> {
    vec![
        Entry::Action(Action::new(
            "Status",
            "project overview",
            "status",
            Run::Job { reveal: true },
        )),
        Entry::Action(Action::new(
            "Regenerate the overlay",
            "composer.tryout.json",
            "composer",
            JOB,
        )),
    ]
}

/// What this kind of project can do: a project has no Core to update, patch
/// or reset, no overlay and no backend of tryout's knowing, and its "fresh
/// install" is an empty database. Separators left doubled are dropped.
pub fn for_kind(entries: Vec<Entry>, kind: &dyn ProjectKind) -> Vec<Entry> {
    let supported = |a: &Action| {
        let verb = match a.args.as_slice() {
            [w, sub, ..] if w == "worktree" => format!("{w} {sub}"),
            [v, ..] => v.clone(),
            [] => String::new(),
        };
        kind.supports(&verb)
            && (kind.backend_path().is_some() || !a.args.iter().any(|x| x == "--backend"))
    };
    let mut out: Vec<Entry> = Vec::new();
    for e in entries {
        match e {
            Entry::Action(a) if !supported(&a) => {}
            Entry::Action(mut a) if kind.setup_command("").is_empty() && a.args[0] == "delete" => {
                a.label = "Empty its database…".into();
                a.hint = "the app sets itself up again".into();
                out.push(Entry::Action(a));
            }
            Entry::Separator if matches!(out.last(), None | Some(Entry::Separator)) => {}
            e => out.push(e),
        }
    }
    if matches!(out.last(), Some(Entry::Separator)) {
        out.pop();
    }
    out
}

/// Groups in order, a separator between the non-empty ones.
pub fn join<const N: usize>(groups: [Vec<Entry>; N]) -> Vec<Entry> {
    let mut out = Vec::new();
    for g in groups.into_iter().filter(|g| !g.is_empty()) {
        if !out.is_empty() {
            out.push(Entry::Separator);
        }
        out.extend(g);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::tests::fixture;

    /// The fixture's project server.
    fn maria() -> Db {
        Db::parse("mariadb:11.8").unwrap()
    }

    #[test]
    fn a_project_is_offered_only_what_it_can_do() {
        use crate::core::kind::{GENERIC_PROJECT, TYPO3_CORE};
        let menu = |w: &Worktree, kind: &dyn ProjectKind| {
            for_kind(join([for_worktree(w, &maria()), project()]), kind)
        };
        let lines = |entries: &[Entry]| -> Vec<String> {
            entries
                .iter()
                .filter_map(|e| match e {
                    Entry::Action(a) => Some(a.args.join(" ")),
                    _ => None,
                })
                .collect()
        };
        for w in &fixture() {
            let entries = menu(w, &GENERIC_PROJECT);
            for line in lines(&entries) {
                for core_only in [
                    "download",
                    "checkout",
                    "patch",
                    "reset",
                    "composer",
                    "--backend",
                ] {
                    assert!(!line.contains(core_only), "{line}");
                }
                assert!(!line.starts_with("worktree use"), "{line}");
            }
            // No separator at either end, none doubled.
            assert!(!matches!(entries.first(), Some(Entry::Separator)));
            assert!(!matches!(entries.last(), Some(Entry::Separator)));
            assert!(
                !entries
                    .windows(2)
                    .any(|p| matches!(p, [Entry::Separator, Entry::Separator]))
            );
            // Core keeps everything it had.
            let all = join([for_worktree(w, &maria()), project()]);
            assert_eq!(menu(w, &TYPO3_CORE), all);
        }
        let served = fixture()
            .into_iter()
            .find(|w| w.served() && !w.primary)
            .unwrap();
        let entries = menu(&served, &GENERIC_PROJECT);
        assert!(entries.iter().any(|e| e.label() == "Empty its database…"));
    }

    fn claim(line: &str) -> Claim {
        Claim::of(
            &line
                .split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn what_can_restart_ddev_or_rewrite_shared_config_runs_alone() {
        for line in [
            "worktree serve v13",
            "worktree serve v13 --php 8.2",
            "worktree unserve v13",
            "worktree unserve v13 --drop-db",
            "worktree rename v13 old",
            "worktree remove v13 --yes",
            "worktree use v13",
            "worktree add v13 13.4 --serve",
            "worktree add v13 13.4 --php=8.2",
            "worktree add v13 13.4 --db=postgres",
            "worktree serve v13 --db postgres",
            "composer",
            "patch",             // no site named: whichever is primary
            "something-new v13", // unknown: the safe side
        ] {
            assert_eq!(claim(line), Claim::Alone, "{line}");
        }
    }

    #[test]
    fn work_on_one_worktree_claims_only_that_worktree() {
        for (line, site) in [
            ("patch --site v13 91003 91001", "v13"),
            ("checkout --site main 13.4", "main"),
            ("reset v13", "v13"),
            ("download main", "main"),
            ("delete v13 --yes", "v13"),
            ("exec v13 vendor/bin/typo3 cache:flush", "v13"),
            ("worktree add v14 main", "v14"),
        ] {
            assert_eq!(claim(line), Claim::Site(site.into()), "{line}");
        }
        assert_eq!(claim("status"), Claim::Free);
    }

    #[test]
    fn every_menu_command_has_a_claim_that_matches_its_kind() {
        // A menu entry's claim: the site ones name their own worktree.
        for w in fixture() {
            for e in for_worktree(&w, &maria()) {
                let Entry::Action(a) = e else { continue };
                match Claim::of(&a.args) {
                    Claim::Site(n) => assert_eq!(n, w.name, "{}", a.args.join(" ")),
                    Claim::Alone | Claim::Free => {}
                }
            }
        }
    }

    #[test]
    fn claims_conflict_on_the_same_worktree_or_with_anything_alone() {
        let (v13, main) = (Claim::Site("v13".into()), Claim::Site("main".into()));
        assert!(v13.conflicts(&v13.clone()));
        assert!(!v13.conflicts(&main));
        assert!(!Claim::Free.conflicts(&v13));
        assert!(!Claim::Free.conflicts(&Claim::Free));
        for c in [v13, main, Claim::Free, Claim::Alone] {
            assert!(Claim::Alone.conflicts(&c) && c.conflicts(&Claim::Alone));
        }
    }

    #[test]
    fn a_served_site_shows_its_database_server_and_switches_to_another() {
        let mut v13 = fixture()[1].clone();
        v13.db_engine = Some("postgres:16".into());
        let entries = for_worktree(&v13, &maria());
        let Some(Entry::Sub { label, items, .. }) =
            entries.iter().find(|e| e.label().starts_with("Database"))
        else {
            panic!("no database entry")
        };
        assert_eq!(label, "Database: PostgreSQL 16");
        let row = |label: &str| -> (String, String) {
            let a = items
                .iter()
                .find(|a| a.label == label)
                .unwrap_or_else(|| panic!("no row {label}"));
            (a.hint.clone(), a.args.join(" "))
        };
        // Every type at every version, one row each.
        assert_eq!(items.len(), Db::choices().len());
        assert_eq!(
            row("✓ PostgreSQL 16"),
            (
                "running now".into(),
                "worktree serve v13 --db postgres:16".into()
            )
        );
        assert_eq!(
            row("  PostgreSQL 18"),
            (
                "its own server".into(),
                "worktree serve v13 --db postgres:18 --switch".into()
            )
        );
        assert_eq!(
            row("  MariaDB 11.8"),
            (
                "the project's own server · TYPO3 lists ≤ 10.x".into(),
                "worktree serve v13 --db mariadb:11.8 --switch".into()
            )
        );
        assert_eq!(row("  MariaDB 10.11").0, "its own server");
        assert_eq!(
            row("  SQLite"),
            (
                "a file in the site, no server".into(),
                "worktree serve v13 --db sqlite --switch".into()
            )
        );
        // A project on a version not offered still has its row, among its type.
        let old = Db::parse_recorded("mariadb:10.4").unwrap();
        let entries = for_worktree(&v13, &old);
        let Some(Entry::Sub { items, .. }) =
            entries.iter().find(|e| e.label().starts_with("Database"))
        else {
            panic!("no database entry")
        };
        let labels: Vec<&str> = items.iter().map(|a| a.label.as_str()).collect();
        let at = labels
            .iter()
            .position(|l| *l == "  MariaDB 10.4")
            .expect("its row");
        assert!(labels[at + 1].contains("MariaDB"), "{labels:?}");
        // The primary's server is the project's: nothing to pick there.
        assert!(
            !for_worktree(&fixture()[0], &maria())
                .iter()
                .any(|e| e.label().starts_with("Database"))
        );
    }

    /// Each entry as "label → command", submenus as "label ▸ [commands]".
    fn menu(w: &Worktree) -> Vec<String> {
        for_worktree(w, &maria())
            .iter()
            .map(|e| match e {
                Entry::Action(a) => format!("{} → {}", a.label, a.args.join(" ")),
                // Its own test spells the database list out; here, its size.
                Entry::Sub { label, items, .. } if label.contains("atabase") => {
                    format!("{label} ▸ [{} servers]", items.len())
                }
                Entry::Sub { label, items, .. } => format!(
                    "{label} ▸ [{}]",
                    items
                        .iter()
                        .map(|a| a.args.join(" "))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Entry::Separator => "—".into(),
            })
            .collect()
    }

    fn with_php(mut w: Worktree) -> Worktree {
        w.php_versions = vec!["8.2".into(), "8.3".into(), "8.4".into()];
        w
    }

    #[test]
    fn the_primary_and_origin_clone_gets_its_site_core_and_data_but_no_php_or_removal() {
        assert_eq!(
            menu(&with_php(fixture()[0].clone())),
            [
                "Open site → launch main",
                "Open backend → launch main --backend",
                "—",
                "Update from its base branch → download main",
                "Switch TYPO3 version… → checkout --site main",
                "Apply Gerrit patch… → patch --site main",
                "Reset Core + rebuild → reset main",
                "—",
                "Run command… → exec main",
                "Fresh install… → delete main",
            ]
        );
    }

    #[test]
    fn a_served_worktree_can_switch_php_and_be_unserved_with_or_without_its_database() {
        let m = menu(&with_php(fixture()[1].clone()));
        assert_eq!(
            m[..7],
            [
                "PHP 8.4 ▸ [worktree serve v13 --php 8.2, worktree serve v13 --php 8.3, worktree serve v13 --php 8.4]",
                "Database: MariaDB 11.8 ▸ [12 servers]",
                "Open site → launch v13",
                "Open backend → launch v13 --backend",
                "Make primary → worktree use v13",
                "Unserve → worktree unserve v13",
                "Unserve and drop its database → worktree unserve v13 --drop-db",
            ]
        );
        assert_eq!(
            m[m.len() - 2..],
            [
                "Rename… → worktree rename v13",
                "Remove… → worktree remove v13"
            ]
        );
    }

    #[test]
    fn an_unserved_worktree_is_offered_serving_and_nothing_that_needs_a_site() {
        assert_eq!(
            menu(&with_php(fixture()[2].clone())),
            [
                "Serve → worktree serve bugfix",
                "Serve on PHP ▸ [worktree serve bugfix --php 8.2, worktree serve bugfix --php 8.3, worktree serve bugfix --php 8.4]",
                "Serve on database ▸ [12 servers]",
                "Make primary → worktree use bugfix",
                "—",
                "Rename… → worktree rename bugfix",
                "Remove… → worktree remove bugfix",
            ]
        );
    }

    #[test]
    fn the_running_php_is_ticked_and_without_versions_there_is_no_submenu() {
        let w = with_php(fixture()[1].clone()); // runs 8.4
        let Some(Entry::Sub { items, .. }) = for_worktree(&w, &maria()).into_iter().next() else {
            panic!("no PHP submenu")
        };
        let labels: Vec<_> = items.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, ["  PHP 8.2", "  PHP 8.3", "✓ PHP 8.4"]);
        // An add-on too old to say which versions fit: no guessing.
        assert!(!menu(&fixture()[1]).iter().any(|e| e.starts_with("PHP")));
    }

    #[test]
    fn every_command_names_its_own_worktree_never_the_sentinel() {
        for w in fixture().into_iter().map(with_php) {
            for e in for_worktree(&w, &maria()) {
                let actions = match e {
                    Entry::Action(a) => vec![a],
                    Entry::Sub { items, .. } => items,
                    Entry::Separator => continue,
                };
                for a in actions {
                    assert!(
                        !a.args.iter().any(|x| x == "@primary"),
                        "{}",
                        a.command_line()
                    );
                    assert!(
                        a.args.contains(&w.name),
                        "{} does not name {}",
                        a.command_line(),
                        w.name
                    );
                }
            }
        }
    }

    #[test]
    fn only_opening_the_site_runs_outside_the_queue() {
        let bg: Vec<_> = for_worktree(&fixture()[1], &maria())
            .into_iter()
            .filter_map(|e| match e {
                Entry::Action(a) if a.run == Run::Background => Some(a.label),
                _ => None,
            })
            .collect();
        assert_eq!(bg, ["Open site", "Open backend"]);
    }

    #[test]
    fn groups_are_separated_but_never_by_two_lines_or_at_the_ends() {
        let j = join([vec![], project(), vec![], project()]);
        assert_eq!(j.first().map(Entry::selectable), Some(true));
        assert_eq!(j.last().map(Entry::selectable), Some(true));
        assert_eq!(j.iter().filter(|e| !e.selectable()).count(), 1);
    }
}
