//! The tryout commands offered for a worktree. The rules are the removed herdr
//! panel's, which were learned the hard way: a row never offers something that
//! cannot work there, and every command names its own worktree — a bare verb acts
//! on whichever Core is primary at the moment it runs, not the one you selected.

use crate::worktrees::Worktree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// In a popup terminal, so prompts (gum) work. `keep_open`: the output is the
    /// point, so a success waits to be read instead of closing at once.
    Popup { keep_open: bool },
    /// No terminal, no popup: fast, silent on success. Only `launch` — it raises
    /// the browser, and a popup would just sit in front of it.
    Background,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub label: &'static str,
    pub hint: &'static str,
    /// Arguments after `ddev tryout`.
    pub args: Vec<String>,
    pub run: Run,
}

impl Action {
    fn new(label: &'static str, hint: &'static str, args: &str, run: Run) -> Self {
        Self {
            label,
            hint,
            args: args.split_whitespace().map(String::from).collect(),
            run,
        }
    }

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

const POPUP: Run = Run::Popup { keep_open: false };
const READ: Run = Run::Popup { keep_open: true };

pub fn for_worktree(w: &Worktree) -> Vec<Action> {
    let n = &w.name;
    // The origin clone owns the object store every worktree branches from:
    // making one belongs there, and remove_core_worktree refuses to drop it.
    let origin = w.dir == ".";
    let add_or_remove = if origin {
        Action::new(
            "worktree add",
            "create a new worktree",
            "worktree add",
            POPUP,
        )
    } else {
        Action::new(
            "worktree remove",
            "delete this checkout",
            &format!("worktree remove {n}"),
            POPUP,
        )
    };

    let mut v = vec![Action::new("status", "project overview", "status", READ)];
    if !w.served() {
        v.push(Action::new(
            "worktree serve",
            "give it its own URL",
            &format!("worktree serve {n}"),
            POPUP,
        ));
        v.push(Action::new(
            "worktree use",
            "make it the primary",
            &format!("worktree use {n}"),
            POPUP,
        ));
        v.push(add_or_remove);
        return v;
    }
    v.push(Action::new(
        "checkout",
        "switch TYPO3 version",
        &format!("checkout --site {n}"),
        POPUP,
    ));
    v.push(Action::new(
        "patch",
        "apply a Gerrit change",
        &format!("patch --site {n}"),
        POPUP,
    ));
    v.push(Action::new(
        "download",
        "update from its base branch",
        &format!("download {n}"),
        POPUP,
    ));
    v.push(Action::new(
        "reset",
        "reset Core + rebuild",
        &format!("reset {n}"),
        POPUP,
    ));
    v.push(Action::new(
        "exec",
        "run a command in it",
        &format!("exec {n}"),
        READ,
    ));
    // composer has no site: it always rewrites the PRIMARY overlay.
    if w.primary {
        v.push(Action::new(
            "composer",
            "regenerate the overlay",
            "composer",
            POPUP,
        ));
    } else {
        v.push(Action::new(
            "worktree use",
            "make it the primary",
            &format!("worktree use {n}"),
            POPUP,
        ));
    }
    v.push(Action::new(
        "worktree unserve",
        "stop serving it",
        &format!("worktree unserve {n}"),
        POPUP,
    ));
    v.push(add_or_remove);
    v.push(Action::new(
        "launch frontend",
        "open the site",
        &format!("launch {n}"),
        Run::Background,
    ));
    v.push(Action::new(
        "launch backend",
        "open /typo3/",
        &format!("launch {n} --backend"),
        Run::Background,
    ));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::fixture;

    fn labels(w: &Worktree) -> Vec<&'static str> {
        for_worktree(w).iter().map(|a| a.label).collect()
    }

    #[test]
    fn an_unserved_worktree_is_offered_nothing_that_needs_a_site() {
        let bugfix = &fixture()[2];
        let l = labels(bugfix);
        assert_eq!(
            l,
            [
                "status",
                "worktree serve",
                "worktree use",
                "worktree remove"
            ]
        );
    }

    #[test]
    fn every_command_names_its_own_worktree_never_the_sentinel() {
        for w in fixture() {
            for a in for_worktree(&w) {
                assert!(
                    !a.args.iter().any(|x| x == "@primary"),
                    "{}",
                    a.command_line()
                );
                let scoped = !matches!(a.label, "status" | "composer" | "worktree add");
                if scoped {
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
    fn the_origin_clone_offers_add_and_never_remove() {
        let main = &fixture()[0];
        let l = labels(main);
        assert!(l.contains(&"worktree add") && !l.contains(&"worktree remove"));
        assert!(l.contains(&"composer"), "composer belongs to the primary");
        let v13 = &fixture()[1];
        assert!(
            !labels(v13).contains(&"composer"),
            "composer would hit the primary"
        );
    }

    #[test]
    fn only_launch_skips_the_popup_and_launch_comes_last() {
        let v = for_worktree(&fixture()[1]);
        let bg: Vec<_> = v
            .iter()
            .filter(|a| a.run == Run::Background)
            .map(|a| a.label)
            .collect();
        assert_eq!(bg, ["launch frontend", "launch backend"]);
        assert_eq!(v.last().unwrap().args, ["launch", "v13", "--backend"]);
    }
}
