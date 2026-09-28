//! Application state and what each key does to it. No terminal I/O here, so all
//! of it runs under test.

use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::actions::{self, Action, Run};
use crate::keys;
use crate::pane::Pane;
use crate::worktrees::Worktree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listing {
    Loading,
    Loaded,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Pane,
}

/// Work the event loop has to do outside the state: it owns the terminal size
/// and the background loader.
#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    Reload,
    /// Start (or restart) the shell for this worktree, then focus it.
    OpenPane(String),
    /// Run a tryout command: in a popup terminal, or in the background.
    Run(Action),
}

/// The command menu for one worktree.
pub struct Menu {
    pub worktree: String,
    pub items: Vec<Action>,
    pub selected: usize,
}

/// A tryout command running in a modal terminal of its own, so its prompts work.
pub struct Popup {
    pub action: Action,
    pub pane: Pane,
}

pub struct App {
    pub root: PathBuf,
    pub project: String,
    pub worktrees: Vec<Worktree>,
    pub listing: Listing,
    pub selected: usize,
    pub focus: Focus,
    /// One shell per worktree, kept alive while you look at another.
    pub panes: HashMap<String, Pane>,
    pub menu: Option<Menu>,
    pub popup: Option<Popup>,
    pub notice: Option<String>,
}

/// The key that moves focus between the list and the pane. It must be one a
/// shell rarely needs, since in the pane every other key belongs to the shell.
pub fn is_focus_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL)
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        let project = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "tryout".into());
        Self {
            root,
            project,
            worktrees: Vec::new(),
            listing: Listing::Loading,
            selected: 0,
            focus: Focus::List,
            panes: HashMap::new(),
            menu: None,
            popup: None,
            notice: None,
        }
    }

    pub fn selected(&self) -> Option<&Worktree> {
        self.worktrees.get(self.selected)
    }

    /// A fresh listing replaces the old one, but the selection stays on the same
    /// worktree by name — a reload must not move you.
    pub fn set_worktrees(&mut self, result: anyhow::Result<Vec<Worktree>>) {
        match result {
            Ok(list) => {
                let keep = self.selected().map(|w| w.name.clone());
                self.worktrees = list;
                self.selected = keep
                    .and_then(|n| self.worktrees.iter().position(|w| w.name == n))
                    .or_else(|| self.worktrees.iter().position(|w| w.primary))
                    .unwrap_or(0);
                self.listing = Listing::Loaded;
            }
            Err(e) => self.listing = Listing::Failed(format!("{e:#}")),
        }
    }

    pub fn checkout_dir(&self, name: &str) -> PathBuf {
        self.worktrees
            .iter()
            .find(|w| w.name == name)
            .map_or_else(|| self.root.clone(), |w| w.checkout_dir(&self.root))
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Effect {
        self.notice = None;
        // Modal first: a popup owns every key, then the menu.
        if self.popup.is_some() {
            return self.popup_key(key);
        }
        if self.menu.is_some() {
            return self.menu_key(key);
        }
        match self.focus {
            Focus::Pane => self.pane_key(key),
            Focus::List => self.list_key(key),
        }
    }

    fn pane_key(&mut self, key: KeyEvent) -> Effect {
        if is_focus_key(&key) {
            self.focus = Focus::List;
            return Effect::None;
        }
        let Some(name) = self.selected().map(|w| w.name.clone()) else {
            self.focus = Focus::List;
            return Effect::None;
        };
        match self.panes.get_mut(&name) {
            Some(pane) if pane.is_alive() => {
                if let Some(bytes) = keys::to_bytes(key) {
                    pane.write(&bytes);
                }
                Effect::None
            }
            // The shell ended: Enter starts a new one, anything else goes back.
            _ if key.code == KeyCode::Enter => Effect::OpenPane(name),
            _ => {
                self.focus = Focus::List;
                Effect::None
            }
        }
    }

    fn list_key(&mut self, key: KeyEvent) -> Effect {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => Effect::Quit,
            KeyCode::Char('c') if ctrl => Effect::Quit,
            KeyCode::Char('r') => {
                self.listing = Listing::Loading;
                Effect::Reload
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_by(1);
                Effect::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_by(-1);
                Effect::None
            }
            KeyCode::Home => {
                self.selected = 0;
                Effect::None
            }
            KeyCode::End => {
                self.selected = self.worktrees.len().saturating_sub(1);
                Effect::None
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.enter_pane(),
            KeyCode::Char('a') | KeyCode::Char(' ') => {
                self.open_menu();
                Effect::None
            }
            _ if is_focus_key(&key) => self.enter_pane(),
            _ => Effect::None,
        }
    }

    /// Focus the selected worktree's shell, starting it if there is none.
    fn enter_pane(&mut self) -> Effect {
        let Some(w) = self.selected() else {
            return Effect::None;
        };
        let name = w.name.clone();
        match self.panes.get(&name) {
            Some(p) if p.is_alive() => {
                self.focus = Focus::Pane;
                Effect::None
            }
            _ => Effect::OpenPane(name),
        }
    }

    fn open_menu(&mut self) {
        if let Some(w) = self.selected() {
            self.menu = Some(Menu {
                worktree: w.name.clone(),
                items: actions::for_worktree(w),
                selected: 0,
            });
        }
    }

    fn menu_key(&mut self, key: KeyEvent) -> Effect {
        let Some(menu) = self.menu.as_mut() else {
            return Effect::None;
        };
        let last = menu.items.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.menu = None,
            KeyCode::Down | KeyCode::Char('j') => menu.selected = (menu.selected + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => menu.selected = menu.selected.saturating_sub(1),
            KeyCode::Enter => {
                let action = menu.items[menu.selected].clone();
                self.menu = None;
                return Effect::Run(action);
            }
            _ => {}
        }
        Effect::None
    }

    fn popup_key(&mut self, key: KeyEvent) -> Effect {
        let Some(popup) = self.popup.as_mut() else {
            return Effect::None;
        };
        // While it runs, every key is the command's — Esc included, which is how
        // a gum prompt is cancelled.
        if popup.pane.is_alive() {
            if let Some(bytes) = keys::to_bytes(key) {
                popup.pane.write(&bytes);
            }
            return Effect::None;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char(' ') => {
                self.close_popup()
            }
            _ => Effect::None,
        }
    }

    fn close_popup(&mut self) -> Effect {
        match self.popup.take() {
            Some(p) if p.action.changes_worktrees() => {
                self.listing = Listing::Loading;
                Effect::Reload
            }
            _ => Effect::None,
        }
    }

    /// Called every loop. A popup that succeeded closes itself — clicking a
    /// command and then dismissing a report of its success is friction — unless
    /// its output is the point. A failure always stays: its output is the only
    /// place the error is written.
    pub fn tick(&mut self) -> Effect {
        let done = self.popup.as_ref().and_then(|p| {
            (p.pane.exit_code() == Some(0) && p.action.run == (Run::Popup { keep_open: false }))
                .then_some(p.action.label)
        });
        match done {
            Some(label) => {
                self.notice = Some(format!("✓ {label}"));
                self.close_popup()
            }
            None => Effect::None,
        }
    }

    fn move_by(&mut self, delta: isize) {
        if self.worktrees.is_empty() {
            return;
        }
        let last = self.worktrees.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crossterm::event::KeyModifiers as M;

    pub fn fixture() -> Vec<Worktree> {
        let wt = |name: &str, branch: Option<&str>, base: &str, url: Option<&str>| Worktree {
            name: name.into(),
            dir: if branch.is_some() {
                ".".into()
            } else {
                format!("worktrees/{name}")
            },
            head: "32d1f513a57".into(),
            branch: branch.map(Into::into),
            base: Some(base.into()),
            patches: 0,
            modified: 0,
            untracked: 0,
            primary: branch.is_some(),
            url: url.map(Into::into),
            php: url.map(|_| "8.4".into()),
            db: url.map(|_| format!("db_{name}")),
            subject: Some("[TASK] Raise phpstan to 2.1.17".into()),
        };
        let mut v13 = wt("v13", None, "13.4", Some("https://v13.demo.ddev.site"));
        (v13.patches, v13.modified, v13.untracked) = (2, 3, 1);
        vec![
            wt("main", Some("main"), "main", Some("https://demo.ddev.site")),
            v13,
            wt("bugfix", None, "main", None),
        ]
    }

    fn app() -> App {
        let mut a = App::new(PathBuf::from("/p/demo"));
        a.set_worktrees(Ok(fixture()));
        a
    }

    fn press(a: &mut App, code: KeyCode) -> Effect {
        a.handle_key(KeyEvent::new(code, M::NONE))
    }

    #[test]
    fn starts_on_the_primary_and_moves_within_bounds() {
        let mut a = app();
        assert_eq!(a.selected().unwrap().name, "main");
        press(&mut a, KeyCode::Up);
        assert_eq!(a.selected, 0);
        press(&mut a, KeyCode::Char('j'));
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Down);
        assert_eq!(a.selected().unwrap().name, "bugfix");
    }

    #[test]
    fn enter_asks_for_a_shell_in_the_selected_worktree() {
        let mut a = app();
        press(&mut a, KeyCode::Down);
        assert_eq!(
            press(&mut a, KeyCode::Enter),
            Effect::OpenPane("v13".into())
        );
    }

    #[test]
    fn a_reload_keeps_the_selection_on_the_same_worktree() {
        let mut a = app();
        press(&mut a, KeyCode::End);
        assert_eq!(press(&mut a, KeyCode::Char('r')), Effect::Reload);
        assert_eq!(a.listing, Listing::Loading);
        let mut reordered = fixture();
        reordered.reverse();
        a.set_worktrees(Ok(reordered));
        assert_eq!(a.selected().unwrap().name, "bugfix");
    }

    #[test]
    fn a_failed_listing_says_why() {
        let mut a = App::new(PathBuf::from("/p/demo"));
        a.set_worktrees(Err(anyhow::anyhow!("ddev is not running")));
        assert_eq!(a.listing, Listing::Failed("ddev is not running".into()));
    }

    fn popup(action_args: &str, keep_open: bool, script: &str) -> Popup {
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.args(["-c", script]);
        let action = crate::actions::Action {
            label: "test",
            hint: "",
            args: action_args.split_whitespace().map(String::from).collect(),
            run: Run::Popup { keep_open },
        };
        Popup {
            action,
            pane: Pane::spawn(cmd, 5, 30).unwrap(),
        }
    }

    fn wait_exit(a: &App) {
        let t = std::time::Instant::now();
        while a.popup.as_ref().unwrap().pane.exit_code().is_none() {
            assert!(t.elapsed().as_secs() < 5, "command never ended");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn the_menu_offers_the_selected_worktrees_commands_and_runs_one() {
        let mut a = app();
        press(&mut a, KeyCode::End);
        press(&mut a, KeyCode::Char('a'));
        let menu = a.menu.as_ref().unwrap();
        assert_eq!(menu.worktree, "bugfix");
        assert_eq!(menu.items[1].args, ["worktree", "serve", "bugfix"]);
        press(&mut a, KeyCode::Down);
        let Effect::Run(action) = press(&mut a, KeyCode::Enter) else {
            panic!("no run")
        };
        assert_eq!(action.command_line(), "ddev tryout worktree serve bugfix");
        assert!(a.menu.is_none());
    }

    #[test]
    fn escape_closes_the_menu_and_runs_nothing() {
        let mut a = app();
        press(&mut a, KeyCode::Char(' '));
        assert_eq!(press(&mut a, KeyCode::Esc), Effect::None);
        assert!(a.menu.is_none());
    }

    #[test]
    fn a_successful_popup_closes_itself_and_reloads() {
        let mut a = app();
        a.popup = Some(popup("worktree serve x", false, "exit 0"));
        wait_exit(&a);
        assert_eq!(a.tick(), Effect::Reload);
        assert!(a.popup.is_none());
        assert_eq!(a.notice.as_deref(), Some("✓ test"));
    }

    #[test]
    fn a_failed_popup_stays_until_dismissed() {
        let mut a = app();
        a.popup = Some(popup("worktree serve x", false, "echo boom; exit 2"));
        wait_exit(&a);
        assert_eq!(a.tick(), Effect::None);
        assert!(a.popup.is_some(), "the error would vanish with it");
        assert_eq!(press(&mut a, KeyCode::Esc), Effect::Reload);
        assert!(a.popup.is_none());
    }

    #[test]
    fn output_worth_reading_waits_even_on_success() {
        let mut a = app();
        a.popup = Some(popup("status", true, "echo report; exit 0"));
        wait_exit(&a);
        assert_eq!(a.tick(), Effect::None);
        assert!(a.popup.is_some());
        // status changes nothing, so dismissing it does not reload.
        assert_eq!(press(&mut a, KeyCode::Enter), Effect::None);
    }

    #[test]
    fn the_focus_key_leaves_the_pane_and_q_there_is_just_a_letter() {
        let mut a = app();
        a.focus = Focus::Pane;
        // No pane is running, so a plain key falls back to the list — never quits.
        assert_eq!(press(&mut a, KeyCode::Char('q')), Effect::None);
        assert_eq!(a.focus, Focus::List);
        a.focus = Focus::Pane;
        a.handle_key(KeyEvent::new(KeyCode::Char('g'), M::CONTROL));
        assert_eq!(a.focus, Focus::List);
    }
}
