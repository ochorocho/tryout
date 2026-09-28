//! Native forms: the questions a command needs, asked by the TUI itself, so the
//! command runs as a job with every argument supplied. A form is either fields
//! (text, a filterable pick list, a checkbox) or a confirmation that names what
//! goes; either way it ends in exactly one fully-argued `ddev tryout` command.
//! Pure state and keys — all of it runs under test.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use serde::Deserialize;

use crate::tui::actions::{Action, Run};

/// An open Gerrit change, as `ddev tryout patch --list --json` gives it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Change {
    pub number: u64,
    pub subject: String,
    pub owner: String,
    pub scores: String,
}

/// Which question is being asked, and about which worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormKind {
    NewWorktree,
    Rename(String),
    Checkout(String),
    Remove(String),
    DropDb(String),
    Reset(String),
    FreshInstall(String),
    Exec(String),
    Patch(String),
}

impl FormKind {
    /// Its pick list is the branch list, loaded from the add-on on demand.
    pub fn wants_branches(&self) -> bool {
        matches!(self, FormKind::NewWorktree | FormKind::Checkout(_))
    }

    /// Its list is the open Gerrit changes for this worktree's branch.
    pub fn wants_patches(&self) -> Option<&str> {
        match self {
            FormKind::Patch(n) => Some(n),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Text {
        label: &'static str,
        value: String,
        hint: &'static str,
    },
    /// A list with a filter typed into it. `options` None: still loading.
    Pick {
        label: &'static str,
        options: Option<Vec<String>>,
        filter: String,
        selected: usize,
    },
    Check {
        label: &'static str,
        value: bool,
    },
    /// Several of a list, ticked with space. `options` None: still loading.
    Choose {
        label: &'static str,
        options: Option<Vec<Change>>,
        filter: String,
        selected: usize,
        chosen: Vec<u64>,
    },
}

impl Field {
    /// The changes that match the filter — by number, subject or owner.
    pub fn visible_changes(&self) -> Vec<&Change> {
        match self {
            Field::Choose {
                options: Some(o),
                filter,
                ..
            } => {
                let f = filter.to_lowercase();
                o.iter()
                    .filter(|c| {
                        c.number.to_string().contains(&f)
                            || c.subject.to_lowercase().contains(&f)
                            || c.owner.to_lowercase().contains(&f)
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// The pick list's options that match its filter, in order.
    pub fn visible(&self) -> Vec<&str> {
        match self {
            Field::Pick {
                options: Some(o),
                filter,
                ..
            } => o
                .iter()
                .filter(|v| v.to_lowercase().contains(&filter.to_lowercase()))
                .map(String::as_str)
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    pub kind: FormKind,
    pub title: String,
    pub fields: Vec<Field>,
    pub focus: usize,
    /// For a confirmation: what it will do, line by line. y confirms.
    pub confirm: Vec<String>,
    pub error: Option<String>,
    /// Names already taken, so a clash is caught before the job runs.
    taken: Vec<String>,
    /// The branch the list starts on once it arrives.
    default_branch: String,
}

/// What a key did to the form.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Stay,
    Cancel,
    Submit(Action),
}

const JOB: Run = Run::Job { reveal: false };

impl Form {
    /// `taken`: the worktree names that exist. `default_branch`: what the
    /// branch list starts on.
    pub fn new(kind: FormKind, taken: Vec<String>, default_branch: &str) -> Self {
        let branch_pick = |label| Field::Pick {
            label,
            options: None,
            filter: String::new(),
            selected: 0,
        };
        let (title, fields, confirm) = match &kind {
            FormKind::NewWorktree => (
                "New worktree".to_string(),
                vec![
                    Field::Text {
                        label: "Name",
                        value: String::new(),
                        hint: "becomes worktrees/<name> — letters, digits, . _ -",
                    },
                    branch_pick("Based on"),
                    Field::Check {
                        label: "Serve it now (its own URL, the best PHP it accepts)",
                        value: false,
                    },
                ],
                vec![],
            ),
            FormKind::Rename(n) => (
                format!("Rename {n}"),
                vec![Field::Text {
                    label: "New name",
                    value: n.clone(),
                    hint: "the branch is untouched; a served site moves with it",
                }],
                vec![],
            ),
            FormKind::Checkout(n) => (
                format!("Switch {n} to another TYPO3 version"),
                vec![branch_pick("Branch")],
                vec![],
            ),
            FormKind::Remove(n) => (
                format!("Remove {n}?"),
                vec![],
                vec![
                    format!("Deletes the checkout worktrees/{n} and everything in it,"),
                    "including vendor/, var/ and uncommitted work.".into(),
                    "A served site goes too; its database is kept.".into(),
                ],
            ),
            FormKind::DropDb(n) => (
                format!("Unserve {n} and drop its database?"),
                vec![],
                vec![
                    format!("Its site stops, and the database db_{n} is dropped."),
                    "The checkout stays.".into(),
                ],
            ),
            FormKind::Reset(n) => (
                format!("Reset {n}?"),
                vec![],
                vec![
                    "Resets its Core to the latest of its base branch and rebuilds.".into(),
                    "Local changes and applied patches are lost.".into(),
                ],
            ),
            FormKind::FreshInstall(n) => (
                format!("Fresh install of {n}?"),
                vec![],
                vec![
                    "Wipes its database and fileadmin, then sets TYPO3 up anew.".into(),
                    "Content, users and uploads are lost.".into(),
                ],
            ),
            FormKind::Exec(n) => (
                format!("Run a command in {n}"),
                vec![Field::Text {
                    label: "Command, run with the site's own PHP",
                    value: String::new(),
                    hint: "e.g. vendor/bin/typo3 cache:flush — quote as in a shell",
                }],
                vec![],
            ),
            FormKind::Patch(n) => (
                format!("Apply Gerrit changes to {n}"),
                vec![Field::Choose {
                    label: "Open changes on its branch",
                    options: None,
                    filter: String::new(),
                    selected: 0,
                    chosen: Vec::new(),
                }],
                vec![],
            ),
        };
        Self {
            kind,
            title,
            fields,
            focus: 0,
            confirm,
            error: None,
            taken,
            default_branch: default_branch.to_string(),
        }
    }

    /// The open changes arrived.
    pub fn set_changes(&mut self, changes: Vec<Change>) {
        for f in &mut self.fields {
            if let Field::Choose { options, .. } = f {
                *options = Some(changes.clone());
            }
        }
    }

    pub fn is_confirmation(&self) -> bool {
        self.fields.is_empty()
    }

    /// The branch list arrived: newest first, the default preselected.
    pub fn set_branches(&mut self, branches: &[String]) {
        let sorted = order_branches(branches);
        let default = self.default_branch.as_str();
        for f in &mut self.fields {
            if let Field::Pick {
                options, selected, ..
            } = f
            {
                *selected = sorted.iter().position(|b| b == default).unwrap_or(0);
                *options = Some(sorted.clone());
            }
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        if key.code == KeyCode::Esc {
            return Outcome::Cancel;
        }
        if self.is_confirmation() {
            // Destructive: only an explicit y goes ahead.
            return match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.submit(),
                _ => Outcome::Cancel,
            };
        }
        match key.code {
            KeyCode::Tab => self.focus = (self.focus + 1) % self.fields.len(),
            KeyCode::BackTab => {
                self.focus = (self.focus + self.fields.len() - 1) % self.fields.len()
            }
            KeyCode::Enter => return self.submit(),
            _ => self.edit(key),
        }
        Outcome::Stay
    }

    fn edit(&mut self, key: KeyEvent) {
        self.error = None;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match &mut self.fields[self.focus] {
            Field::Text { value, .. } => match key.code {
                KeyCode::Backspace => {
                    value.pop();
                }
                KeyCode::Char('u') if ctrl => value.clear(),
                KeyCode::Char(c) if !ctrl => value.push(c),
                _ => {}
            },
            Field::Check { value, .. } => {
                if matches!(key.code, KeyCode::Char(' ')) {
                    *value = !*value;
                }
            }
            choose @ Field::Choose { .. } => {
                let visible: Vec<u64> = choose.visible_changes().iter().map(|c| c.number).collect();
                let Field::Choose {
                    filter,
                    selected,
                    chosen,
                    ..
                } = choose
                else {
                    unreachable!()
                };
                match key.code {
                    KeyCode::Down => {
                        *selected = (*selected + 1).min(visible.len().saturating_sub(1))
                    }
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Char(' ') => {
                        if let Some(n) = visible.get(*selected) {
                            match chosen.iter().position(|c| c == n) {
                                Some(i) => {
                                    chosen.remove(i);
                                }
                                None => chosen.push(*n),
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        filter.pop();
                        *selected = 0;
                    }
                    KeyCode::Char(c) if !ctrl => {
                        filter.push(c);
                        *selected = 0;
                    }
                    _ => {}
                }
            }
            pick @ Field::Pick { .. } => {
                let count = pick.visible().len();
                let Field::Pick {
                    filter, selected, ..
                } = pick
                else {
                    unreachable!()
                };
                match key.code {
                    KeyCode::Down => *selected = (*selected + 1).min(count.saturating_sub(1)),
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Backspace => {
                        filter.pop();
                        *selected = 0;
                    }
                    KeyCode::Char(c) if !ctrl => {
                        filter.push(c);
                        *selected = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    fn text(&self, i: usize) -> String {
        match self.fields.get(i) {
            Some(Field::Text { value, .. }) => value.trim().to_string(),
            _ => String::new(),
        }
    }

    fn picked(&self, i: usize) -> Option<String> {
        let f = self.fields.get(i)?;
        let Field::Pick { selected, .. } = f else {
            return None;
        };
        f.visible().get(*selected).map(|s| s.to_string())
    }

    fn checked(&self, i: usize) -> bool {
        matches!(self.fields.get(i), Some(Field::Check { value: true, .. }))
    }

    /// The command this form asks for, or the reason it cannot be run yet.
    fn submit(&mut self) -> Outcome {
        match self.command() {
            Ok(action) => Outcome::Submit(action),
            Err(e) => {
                self.error = Some(e);
                Outcome::Stay
            }
        }
    }

    fn command(&self) -> Result<Action, String> {
        let job = |label: String, args: String| Action {
            label,
            hint: String::new(),
            args: args.split_whitespace().map(String::from).collect(),
            run: JOB,
        };
        Ok(match &self.kind {
            FormKind::NewWorktree => {
                let name = self.valid_new_name(self.text(0))?;
                let branch = self.picked(1).ok_or("pick the branch it is based on")?;
                let serve = if self.checked(2) { " --serve" } else { "" };
                job(
                    format!("Add {name}"),
                    format!("worktree add {name} {branch}{serve}"),
                )
            }
            FormKind::Rename(n) => {
                let new = self.valid_new_name(self.text(0))?;
                job(
                    format!("Rename {n} → {new}"),
                    format!("worktree rename {n} {new}"),
                )
            }
            FormKind::Checkout(n) => {
                let branch = self.picked(0).ok_or("pick a branch")?;
                job(
                    format!("{n} → {branch}"),
                    format!("checkout --site {n} {branch}"),
                )
            }
            FormKind::Remove(n) => job(format!("Remove {n}"), format!("worktree remove {n} --yes")),
            FormKind::DropDb(n) => job(
                format!("Unserve {n}, drop DB"),
                format!("worktree unserve {n} --drop-db"),
            ),
            FormKind::Reset(n) => job(format!("Reset {n}"), format!("reset {n}")),
            FormKind::FreshInstall(n) => {
                job(format!("Fresh install {n}"), format!("delete {n} --yes"))
            }
            FormKind::Exec(n) => {
                let words = split_args(&self.text(0))?;
                if words.is_empty() {
                    return Err("type the command to run".into());
                }
                let mut args = vec!["exec".to_string(), n.clone()];
                args.extend(words);
                // Its output is its answer: the log opens when it is done.
                Action {
                    label: format!("{n}: {}", self.text(0)),
                    hint: String::new(),
                    args,
                    run: Run::Job { reveal: true },
                }
            }
            FormKind::Patch(n) => {
                let field = &self.fields[0];
                let Field::Choose {
                    chosen, selected, ..
                } = field
                else {
                    unreachable!()
                };
                // Ticked ones in list order; none ticked means the one selected.
                let visible = field.visible_changes();
                let mut ids: Vec<u64> = match field {
                    Field::Choose {
                        options: Some(o), ..
                    } if !chosen.is_empty() => o
                        .iter()
                        .map(|c| c.number)
                        .filter(|num| chosen.contains(num))
                        .collect(),
                    _ => visible
                        .get(*selected)
                        .map(|c| c.number)
                        .into_iter()
                        .collect(),
                };
                ids.dedup();
                if ids.is_empty() {
                    return Err("pick a change (space ticks several)".into());
                }
                let list = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(" ");
                let what = if ids.len() == 1 {
                    format!("#{list}")
                } else {
                    format!("{} changes", ids.len())
                };
                job(
                    format!("Patch {n}: {what}"),
                    format!("patch --site {n} {list}"),
                )
            }
        })
    }

    /// The add-on's own rule (validate_worktree_name), checked before a job
    /// exists — the add-on still has the last word.
    fn valid_new_name(&self, name: String) -> Result<String, String> {
        if name.is_empty() {
            return Err("a name is needed".into());
        }
        if name.starts_with('-') {
            return Err("a name cannot start with '-'".into());
        }
        if name == "."
            || name == ".."
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        {
            return Err("letters, digits, . _ - only".into());
        }
        if self.taken.contains(&name) {
            return Err(format!("{name} already exists"));
        }
        Ok(name)
    }
}

/// A command line split as a shell would: whitespace separates, '…' is
/// literal, "…" allows \" and \\, and a backslash escapes the next character.
pub fn split_args(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => cur.push(ch),
                        None => return Err("a ' is not closed".into()),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(e @ ('"' | '\\')) => cur.push(e),
                            Some(other) => {
                                cur.push('\\');
                                cur.push(other);
                            }
                            None => return Err("a \" is not closed".into()),
                        },
                        Some(ch) => cur.push(ch),
                        None => return Err("a \" is not closed".into()),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(ch) = chars.next() {
                    cur.push(ch);
                }
            }
            c if c.is_whitespace() => {
                if in_word {
                    out.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                cur.push(c);
            }
        }
    }
    if in_word {
        out.push(cur);
    }
    Ok(out)
}

/// main first, then release branches newest first, then anything else
/// (the old TYPO3_x-y branches) — the order you look for them in.
pub fn order_branches(branches: &[String]) -> Vec<String> {
    let version = |b: &str| -> Option<Vec<u32>> {
        b.split('.')
            .map(|p| p.parse().ok())
            .collect::<Option<Vec<_>>>()
    };
    let mut releases: Vec<&String> = branches.iter().filter(|b| version(b).is_some()).collect();
    releases.sort_by_key(|b| std::cmp::Reverse(version(b)));
    let mut out: Vec<String> = branches.iter().filter(|b| *b == "main").cloned().collect();
    out.extend(releases.into_iter().cloned());
    out.extend(
        branches
            .iter()
            .filter(|b| *b != "main" && version(b).is_none())
            .cloned(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(f: &mut Form, code: KeyCode) -> Outcome {
        f.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn typing(f: &mut Form, text: &str) {
        for c in text.chars() {
            key(f, KeyCode::Char(c));
        }
    }

    fn branches() -> Vec<String> {
        ["12.4", "13.4", "14.0", "main", "TYPO3_8-7", "13.10"]
            .map(String::from)
            .to_vec()
    }

    fn args(o: Outcome) -> String {
        match o {
            Outcome::Submit(a) => {
                assert_eq!(a.run, Run::Job { reveal: false }, "a form ends in a job");
                a.args.join(" ")
            }
            other => panic!("not submitted: {other:?}"),
        }
    }

    #[test]
    fn branches_come_main_first_then_newest_release_then_the_rest() {
        assert_eq!(
            order_branches(&branches()),
            ["main", "14.0", "13.10", "13.4", "12.4", "TYPO3_8-7"]
        );
    }

    #[test]
    fn a_new_worktree_asks_name_and_branch_and_may_serve_it() {
        let mut f = Form::new(FormKind::NewWorktree, vec!["main".into()], "13.4");
        f.set_branches(&branches());
        typing(&mut f, "bugfix-9");
        key(&mut f, KeyCode::Enter);
        // Straight Enter takes the preselected branch.
        let mut g = f.clone();
        assert_eq!(
            args(g.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))),
            "worktree add bugfix-9 13.4"
        );
        // Tab to the list, type to filter, Tab to the box, tick it.
        key(&mut f, KeyCode::Tab);
        typing(&mut f, "12");
        key(&mut f, KeyCode::Tab);
        key(&mut f, KeyCode::Char(' '));
        assert_eq!(
            args(key(&mut f, KeyCode::Enter)),
            "worktree add bugfix-9 12.4 --serve"
        );
    }

    #[test]
    fn a_bad_or_taken_name_is_refused_before_anything_runs() {
        for (name, why) in [
            ("", "needed"),
            ("-x", "'-'"),
            ("a b", "only"),
            ("..", "only"),
            ("main", "exists"),
        ] {
            let mut f = Form::new(FormKind::NewWorktree, vec!["main".into()], "main");
            f.set_branches(&branches());
            typing(&mut f, name);
            assert_eq!(key(&mut f, KeyCode::Enter), Outcome::Stay, "{name:?}");
            assert!(
                f.error.as_deref().unwrap().contains(why),
                "{name:?}: {:?}",
                f.error
            );
        }
    }

    #[test]
    fn no_branch_yet_means_no_submit() {
        let mut f = Form::new(FormKind::NewWorktree, vec![], "main");
        typing(&mut f, "x");
        assert_eq!(
            key(&mut f, KeyCode::Enter),
            Outcome::Stay,
            "the list is still loading"
        );
        assert!(f.error.is_some());
    }

    #[test]
    fn rename_starts_from_the_old_name() {
        let mut f = Form::new(FormKind::Rename("v13".into()), vec!["v13".into()], "main");
        assert_eq!(
            key(&mut f, KeyCode::Enter),
            Outcome::Stay,
            "the same name is taken"
        );
        f.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        typing(&mut f, "v13-old");
        assert_eq!(
            args(key(&mut f, KeyCode::Enter)),
            "worktree rename v13 v13-old"
        );
    }

    #[test]
    fn switching_the_version_picks_from_the_branches() {
        let mut f = Form::new(FormKind::Checkout("v13".into()), vec![], "13.4");
        f.set_branches(&branches());
        key(&mut f, KeyCode::Down);
        assert_eq!(
            args(key(&mut f, KeyCode::Enter)),
            "checkout --site v13 12.4"
        );
    }

    #[test]
    fn a_command_is_split_the_way_a_shell_would() {
        assert_eq!(
            split_args(r#"vendor/bin/typo3 config:set X "My Site""#).unwrap(),
            ["vendor/bin/typo3", "config:set", "X", "My Site"]
        );
        assert_eq!(
            split_args(r#"-r 'sleep(1); echo "hi";'"#).unwrap(),
            ["-r", r#"sleep(1); echo "hi";"#]
        );
        assert_eq!(split_args(r#"a\ b "c\"d""#).unwrap(), ["a b", "c\"d"]);
        assert_eq!(split_args("  ").unwrap(), Vec::<String>::new());
        assert!(split_args("'open").is_err());
    }

    #[test]
    fn run_command_passes_its_words_as_arguments_and_shows_the_answer() {
        let mut f = Form::new(FormKind::Exec("v13".into()), vec![], "main");
        assert_eq!(
            key(&mut f, KeyCode::Enter),
            Outcome::Stay,
            "nothing typed yet"
        );
        typing(&mut f, "vendor/bin/typo3 cache:flush --group 'pages a'");
        let Outcome::Submit(a) = key(&mut f, KeyCode::Enter) else {
            panic!()
        };
        assert_eq!(
            a.args,
            [
                "exec",
                "v13",
                "vendor/bin/typo3",
                "cache:flush",
                "--group",
                "pages a"
            ]
        );
        assert_eq!(a.run, Run::Job { reveal: true });
    }

    fn changes() -> Vec<Change> {
        [
            (91001, "Fix page tree", "Anna"),
            (91002, "Speed up cache", "Ben"),
            (91003, "Fix login", "Cleo"),
        ]
        .map(|(number, subject, owner)| Change {
            number,
            subject: subject.into(),
            owner: owner.into(),
            scores: "CR+1".into(),
        })
        .to_vec()
    }

    #[test]
    fn patches_are_ticked_with_space_and_applied_in_list_order() {
        let mut f = Form::new(FormKind::Patch("v13".into()), vec![], "main");
        assert_eq!(key(&mut f, KeyCode::Enter), Outcome::Stay, "still loading");
        f.set_changes(changes());
        key(&mut f, KeyCode::Down);
        key(&mut f, KeyCode::Down);
        key(&mut f, KeyCode::Char(' ')); // 91003
        key(&mut f, KeyCode::Up);
        key(&mut f, KeyCode::Up);
        key(&mut f, KeyCode::Char(' ')); // 91001
        assert_eq!(
            args(key(&mut f, KeyCode::Enter)),
            "patch --site v13 91001 91003"
        );
    }

    #[test]
    fn with_nothing_ticked_the_selected_change_is_applied_and_the_filter_finds_it() {
        let mut f = Form::new(FormKind::Patch("main".into()), vec![], "main");
        f.set_changes(changes());
        typing(&mut f, "login");
        assert_eq!(args(key(&mut f, KeyCode::Enter)), "patch --site main 91003");
    }

    #[test]
    fn destructive_commands_need_an_explicit_y() {
        for (kind, cmd) in [
            (FormKind::Remove("v13".into()), "worktree remove v13 --yes"),
            (
                FormKind::DropDb("v13".into()),
                "worktree unserve v13 --drop-db",
            ),
            (FormKind::Reset("v13".into()), "reset v13"),
            (FormKind::FreshInstall("v13".into()), "delete v13 --yes"),
        ] {
            let mut f = Form::new(kind.clone(), vec![], "main");
            assert!(
                f.is_confirmation() && !f.confirm.is_empty(),
                "{kind:?} names what goes"
            );
            assert_eq!(
                key(&mut f, KeyCode::Enter),
                Outcome::Cancel,
                "Enter is not a yes"
            );
            assert_eq!(args(key(&mut f, KeyCode::Char('y'))), cmd);
        }
    }
}
