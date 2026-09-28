//! Application state and what each key does to it. No terminal I/O here, so all
//! of it runs under test.

use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::keys;
use crate::pane::Pane;
use crate::worktrees::{self, Worktree};

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
        worktrees::checkout_dir(&self.root, name)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Effect {
        self.notice = None;
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
        let wt = |name: &str, branch: &str, primary, url: Option<&str>, dirty| Worktree {
            name: name.into(),
            head: "32d1f513a57".into(),
            branch: branch.into(),
            dirty,
            php: url.map(|_| "8.4".into()),
            db: url.map(|_| format!("db_{name}")),
            url: url.map(Into::into),
            primary,
        };
        vec![
            wt("main", "main", true, Some("https://demo.ddev.site"), false),
            wt(
                "v13",
                "(detached)",
                false,
                Some("https://v13.demo.ddev.site"),
                true,
            ),
            wt("bugfix", "(detached)", false, None, false),
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
