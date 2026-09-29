//! Application state and what each key does to it. No terminal I/O here, so all
//! of it runs under test.

use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::actions::{self, Action, Entry, Run};
use crate::tui::agents::{self, Status};
use crate::tui::forms::{Form, FormKind, Outcome};
use crate::tui::jobs::{Job, JobState, Jobs};
use crate::tui::keys;
use crate::tui::pane::Pane;
use crate::tui::worktrees::Worktree;

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
    /// Leave the session running and let this terminal go.
    Detach,
    /// End the session: every tab, agent and running command with it.
    CloseSession,
    Reload,
    /// Open a new shell tab in this worktree, then focus it.
    NewTab(String),
    /// Run a tryout command: in a popup terminal, or in the background.
    Run(Action),
    /// A form needs the branch list; the event loop fetches it off-thread.
    LoadBranches,
    /// A form needs the open Gerrit changes for this worktree's branch.
    LoadPatches(String),
}

/// The command menu for one worktree.
pub struct Menu {
    pub worktree: String,
    pub items: Vec<Entry>,
    pub selected: usize,
    /// Where it was right-clicked open; None for the `a` key's menu.
    pub anchor: Option<(u16, u16)>,
    /// The open submenu's selection, when the selected entry's `▸` is open.
    pub sub: Option<usize>,
}

impl Menu {
    fn new(worktree: &str, items: Vec<Entry>, anchor: Option<(u16, u16)>) -> Self {
        Self {
            worktree: worktree.to_string(),
            items,
            selected: 0,
            anchor,
            sub: None,
        }
    }

    /// The open submenu's actions, if one is open.
    pub fn sub_items(&self) -> Option<&[Action]> {
        self.sub?;
        match self.items.get(self.selected)? {
            Entry::Sub { items, .. } => Some(items),
            _ => None,
        }
    }

    /// Move the selection by one, over separators, stopping at the ends.
    fn step(&mut self, down: bool) {
        let mut i = self.selected;
        loop {
            let next = if down {
                i.checked_add(1)
            } else {
                i.checked_sub(1)
            };
            match next.filter(|n| *n < self.items.len()) {
                Some(n) if self.items[n].selectable() => {
                    self.selected = n;
                    return;
                }
                Some(n) => i = n,
                None => return,
            }
        }
    }
}

/// Where a click on an open menu landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHit {
    Top(usize),
    Sub(usize),
}

/// One terminal in a worktree's tab bar. The id is stable across closes, so a
/// click or an agent row can name a tab after the ones before it went away.
pub struct Tab {
    pub id: u64,
    pub pane: Pane,
    /// A name you gave it. Wins over the title its program sets; None goes back
    /// to that automatic title.
    pub name: Option<String>,
}

impl Tab {
    /// What the tab bar calls it: your name, else its program's title, else
    /// "shell".
    pub fn label(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.pane.title())
            .unwrap_or_else(|| "shell".into())
    }
}

/// The rename prompt for one tab.
pub struct Rename {
    pub tab_id: u64,
    pub text: String,
    /// The old name is still "selected": the first key typed replaces it,
    /// the first Backspace clears it. An arrow key keeps it for editing.
    pub selected: bool,
}

/// The sidebar's width until it is dragged or reset.
pub const SIDEBAR_DEFAULT: u16 = 34;

/// Two clicks on the same tab within this are a double-click: rename it.
const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

/// A worktree's tabs, kept running while you look at another worktree.
#[derive(Default)]
pub struct Workspace {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl Workspace {
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    /// Drop the tabs whose program has exited, keeping the active one where it
    /// was or on its neighbour. True when anything went.
    fn reap(&mut self) -> bool {
        let before = self.tabs.len();
        let active_id = self.active_tab().map(|t| t.id);
        self.tabs.retain(|t| t.pane.is_alive());
        if let Some(i) = active_id.and_then(|id| self.tabs.iter().position(|t| t.id == id)) {
            self.active = i;
        } else {
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
        }
        self.tabs.len() != before
    }
}

/// One agent in the agents pane.
pub struct AgentRow {
    pub worktree: String,
    pub tab_id: u64,
    pub tab_index: usize,
    /// The program's terminal title (its task), or the agent's name without one.
    pub title: String,
    pub status: Status,
}

/// The password popup: which job asks, what it asked, and the typed value —
/// kept only until it is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordPrompt {
    pub job: u64,
    pub prompt: String,
    pub label: String,
    pub value: String,
}

pub struct App {
    pub root: PathBuf,
    pub project: String,
    pub worktrees: Vec<Worktree>,
    pub listing: Listing,
    pub selected: usize,
    pub focus: Focus,
    /// Each worktree's tabs, by worktree name.
    pub workspaces: HashMap<String, Workspace>,
    next_tab_id: u64,
    /// Which tabs run an agent, by tab id — from the last `ps` poll.
    agent_kinds: HashMap<u64, &'static str>,
    pub menu: Option<Menu>,
    pub rename: Option<Rename>,
    /// `Q` asked whether to close the session; waiting for y or n.
    pub confirm_close: bool,
    /// A job stopped at a password prompt (sudo, for DDEV's hosts file), and
    /// what has been typed for it so far.
    pub password: Option<PasswordPrompt>,
    /// Commands running and done, one at a time, in the Activity panel.
    pub jobs: Jobs,
    /// The job whose log fills the right pane, while it does.
    pub log_view: Option<u64>,
    /// How far the log view is scrolled back from its end, in lines.
    pub log_scroll: usize,
    /// The terminal's size, as the server last drew it: what the log can show,
    /// and so how far back it can scroll.
    pub screen: ratatui::layout::Rect,
    /// A native form asking a command's questions.
    pub form: Option<Form>,
    /// The branch list, fetched once when a form first needs it.
    pub branches: Option<Vec<String>>,
    /// The width the sidebar is asked to take; the layout keeps it in bounds.
    pub sidebar_width: u16,
    /// The widest it can be on the current screen, set by the event loop, so
    /// `}` never grows a width nobody can see.
    pub sidebar_max: u16,
    /// While the divider is dragged: how far the grab was from the width.
    resize_grab: Option<i32>,
    /// The last press on the divider, to tell a double-click.
    last_divider_press: Option<std::time::Instant>,
    /// The last click on a tab, to tell a double-click.
    last_tab_click: Option<(u64, std::time::Instant)>,
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
            workspaces: HashMap::new(),
            next_tab_id: 1,
            agent_kinds: HashMap::new(),
            menu: None,
            rename: None,
            confirm_close: false,
            password: None,
            jobs: Jobs::new("ddev"),
            log_view: None,
            log_scroll: 0,
            screen: ratatui::layout::Rect::new(0, 0, 80, 24),
            form: None,
            branches: None,
            sidebar_width: SIDEBAR_DEFAULT,
            sidebar_max: u16::MAX,
            resize_grab: None,
            last_divider_press: None,
            last_tab_click: None,
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
        // A job waiting for a password comes before everything: it is blocked.
        if self.password.is_some() {
            self.password_key(key);
            return Effect::None;
        }
        // Modal first: a confirmation, a form, a rename, a log, then the menu.
        if self.confirm_close {
            self.confirm_close = false;
            return match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => Effect::CloseSession,
                _ => Effect::None,
            };
        }
        if let Some(form) = self.form.as_mut() {
            return match form.key(key) {
                Outcome::Stay => Effect::None,
                Outcome::Cancel => {
                    self.form = None;
                    Effect::None
                }
                Outcome::Submit(action) => {
                    self.form = None;
                    Effect::Run(action)
                }
            };
        }
        if self.rename.is_some() {
            return self.rename_key(key);
        }
        if self.log_view.is_some() && self.menu.is_none() {
            return self.log_key(key);
        }
        if self.menu.is_some() {
            return self.menu_key(key);
        }
        match self.focus {
            Focus::Pane => self.pane_key(key),
            Focus::List => self.list_key(key),
        }
    }

    /// The selected worktree's tabs, if it has any.
    pub fn selected_workspace(&self) -> Option<&Workspace> {
        self.selected().and_then(|w| self.workspaces.get(&w.name))
    }

    fn selected_workspace_mut(&mut self) -> Option<&mut Workspace> {
        let name = self.selected()?.name.clone();
        self.workspaces.get_mut(&name)
    }

    /// A freshly started shell becomes the worktree's newest tab, and has focus.
    pub fn add_tab(&mut self, worktree: &str, pane: Pane) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let ws = self.workspaces.entry(worktree.to_string()).or_default();
        ws.tabs.push(Tab {
            id,
            pane,
            name: None,
        });
        ws.active = ws.tabs.len() - 1;
        self.focus = Focus::Pane;
    }

    fn pane_key(&mut self, key: KeyEvent) -> Effect {
        if is_focus_key(&key) {
            self.focus = Focus::List;
            return Effect::None;
        }
        match self
            .selected_workspace_mut()
            .and_then(Workspace::active_tab_mut)
        {
            Some(tab) if tab.pane.is_alive() => {
                if let Some(bytes) = keys::to_bytes(key) {
                    tab.pane.write(&bytes);
                }
            }
            _ => self.focus = Focus::List,
        }
        Effect::None
    }

    /// Make tab `index` of the selected worktree active and focus it.
    fn switch_tab(&mut self, index: usize) {
        if let Some(ws) = self.selected_workspace_mut()
            && index < ws.tabs.len()
        {
            ws.active = index;
            self.focus = Focus::Pane;
        }
    }

    /// Step to the previous or next tab, wrapping around.
    fn cycle_tab(&mut self, delta: isize) {
        if let Some(ws) = self.selected_workspace() {
            let n = ws.tabs.len() as isize;
            if n > 0 {
                let i = (ws.active as isize + delta).rem_euclid(n) as usize;
                self.switch_tab(i);
            }
        }
    }

    /// Close the selected worktree's active tab. Dropping the pane drops the PTY
    /// master, which hangs up whatever ran in it.
    fn close_tab(&mut self) {
        let Some(name) = self.selected().map(|w| w.name.clone()) else {
            return;
        };
        let Some(ws) = self.workspaces.get_mut(&name) else {
            return;
        };
        if ws.tabs.is_empty() {
            return;
        }
        ws.tabs.remove(ws.active);
        ws.active = ws.active.min(ws.tabs.len().saturating_sub(1));
        if ws.tabs.is_empty() {
            self.workspaces.remove(&name);
            self.focus = Focus::List;
        }
    }

    fn list_key(&mut self, key: KeyEvent) -> Effect {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => Effect::Detach,
            KeyCode::Char('c') if ctrl => Effect::Detach,
            KeyCode::Char('Q') => {
                self.confirm_close = true;
                Effect::None
            }
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
            KeyCode::Char('t') => self
                .selected()
                .map_or(Effect::None, |w| Effect::NewTab(w.name.clone())),
            KeyCode::Char(c @ '1'..='9') => {
                self.switch_tab(c as usize - '1' as usize);
                Effect::None
            }
            KeyCode::Char('[') => {
                self.cycle_tab(-1);
                Effect::None
            }
            KeyCode::Char(']') => {
                self.cycle_tab(1);
                Effect::None
            }
            KeyCode::Char('w') => {
                self.close_tab();
                Effect::None
            }
            KeyCode::Char('n') => {
                self.next_agent();
                Effect::None
            }
            KeyCode::Char(',') => {
                if let Some(id) = self
                    .selected_workspace()
                    .and_then(Workspace::active_tab)
                    .map(|t| t.id)
                {
                    self.start_rename(id);
                }
                Effect::None
            }
            KeyCode::Char('+') => self.new_worktree(),
            KeyCode::Char('{') => {
                self.set_sidebar_width(i32::from(self.sidebar_width.min(self.sidebar_max)) - 4);
                Effect::None
            }
            KeyCode::Char('}') => {
                self.set_sidebar_width(i32::from(self.sidebar_width.min(self.sidebar_max)) + 4);
                Effect::None
            }
            KeyCode::Char('R') => {
                self.retry_last_failed();
                Effect::None
            }
            KeyCode::Char('L') => {
                match self.jobs.list().first().map(|j| j.id) {
                    Some(id) => self.open_log(id),
                    None => self.notice = Some("no command has run yet".into()),
                }
                Effect::None
            }
            KeyCode::Char('<') => {
                self.move_tab(-1);
                Effect::None
            }
            KeyCode::Char('>') => {
                self.move_tab(1);
                Effect::None
            }
            _ if is_focus_key(&key) => self.enter_pane(),
            _ => Effect::None,
        }
    }

    /// Focus the selected worktree's active tab, opening a first one if there
    /// is none.
    fn enter_pane(&mut self) -> Effect {
        let Some(w) = self.selected() else {
            return Effect::None;
        };
        let name = w.name.clone();
        match self.workspaces.get(&name).and_then(Workspace::active_tab) {
            Some(t) if t.pane.is_alive() => {
                self.focus = Focus::Pane;
                Effect::None
            }
            _ => Effect::NewTab(name),
        }
    }

    /// `a`: the selected worktree's menu, and the project's commands below it.
    fn open_menu(&mut self) {
        if let Some(w) = self.selected() {
            let items = actions::join([actions::for_worktree(w), actions::project()]);
            self.menu = Some(Menu::new(&w.name.clone(), items, None));
        }
    }

    fn menu_key(&mut self, key: KeyEvent) -> Effect {
        let Some(menu) = self.menu.as_mut() else {
            return Effect::None;
        };
        if let (Some(sel), Some(items)) = (menu.sub, menu.sub_items().map(<[Action]>::len)) {
            // In a submenu: pick a version, or step back out of it.
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => menu.sub = Some((sel + 1).min(items - 1)),
                KeyCode::Up | KeyCode::Char('k') => menu.sub = Some(sel.saturating_sub(1)),
                KeyCode::Left | KeyCode::Esc | KeyCode::Char('h') => menu.sub = None,
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    return self.click_menu(Some(MenuHit::Sub(sel)));
                }
                KeyCode::Char('q') => self.menu = None,
                _ => {}
            }
            return Effect::None;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.menu = None,
            KeyCode::Down | KeyCode::Char('j') => menu.step(true),
            KeyCode::Up | KeyCode::Char('k') => menu.step(false),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                let at = menu.selected;
                return self.click_menu(Some(MenuHit::Top(at)));
            }
            _ => {}
        }
        Effect::None
    }

    /// Called every loop: exited tabs close, what is on screen counts as seen,
    /// and finished jobs report — reloading the list when they changed it.
    pub fn tick(&mut self) -> Effect {
        // A tab whose program exited closes itself, as in herdr. A worktree left
        // with none goes back to its placeholder, and the list gets the focus.
        let mut emptied = Vec::new();
        for (name, ws) in &mut self.workspaces {
            if ws.reap() && ws.tabs.is_empty() {
                emptied.push(name.clone());
            }
        }
        for name in emptied {
            self.workspaces.remove(&name);
        }
        if self.focus == Focus::Pane && self.selected_workspace().is_none() {
            self.focus = Focus::List;
        }
        // What is on screen has been seen: its bell and fresh output no longer
        // need anyone's attention.
        if let Some(tab) = self.selected_workspace().and_then(Workspace::active_tab) {
            tab.pane.mark_seen();
        }

        // A job asking for a password gets the popup; one that stopped asking
        // (it ended, or moved on) loses it.
        match (&self.password, self.jobs.waiting()) {
            (None, Some((job, prompt, label))) => {
                self.password = Some(PasswordPrompt {
                    job,
                    prompt,
                    label,
                    value: String::new(),
                });
            }
            (Some(p), waiting) if waiting.as_ref().is_none_or(|w| w.0 != p.job) => {
                self.password = None
            }
            _ => {}
        }

        // Finished jobs: say how it went, show the output where it is the point,
        // and reload the list when a job changed what it shows.
        let mut reload = false;
        for job in self.jobs.tick(&self.root.clone()) {
            reload |= self.job_finished(&job);
        }
        if reload {
            self.listing = Listing::Loading;
            return Effect::Reload;
        }

        Effect::None
    }

    /// A job ended. True when the worktree list should reload.
    fn job_finished(&mut self, job: &Job) -> bool {
        let JobState::Done { ok, code, .. } = job.state else {
            return false;
        };
        self.notice = Some(if ok {
            format!("✓ {}", job.label)
        } else {
            let why = job
                .last_error
                .clone()
                .unwrap_or_else(|| format!("exit {}", code.map_or("?".into(), |c| c.to_string())));
            format!("✗ {}: {why} — R retries · L shows the log", job.label)
        });
        // Its output is its answer: show it, unless you are typing somewhere.
        if job.reveal && self.focus == Focus::List && !self.modal() {
            self.open_log(job.id);
        }
        job.changes_worktrees
    }

    /// Fill the right pane with a job's log, from its end.
    pub fn open_log(&mut self, id: u64) {
        if self.jobs.get(id).is_some() {
            self.log_view = Some(id);
            self.log_scroll = 0;
            self.focus = Focus::List;
        }
    }

    fn log_key(&mut self, key: KeyEvent) -> Effect {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => self.log_view = None,
            KeyCode::PageUp | KeyCode::Char('b') => self.scroll_log(20),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_log(-20),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_log(1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_log(-1),
            KeyCode::Home | KeyCode::Char('g') => self.scroll_log(isize::MAX),
            KeyCode::End | KeyCode::Char('G') => self.log_scroll = 0,
            KeyCode::Char('r') => self.retry_log(),
            _ => {}
        }
        Effect::None
    }

    /// Scroll the open log: positive goes back in time. Kept within what the
    /// log can actually scroll, so the way back is never a run of dead keys.
    pub fn scroll_log(&mut self, delta: isize) {
        let Some(job) = self.log_view.and_then(|id| self.jobs.get(id)) else {
            return;
        };
        let max = crate::tui::ui::log_max_scroll(job, crate::tui::ui::log_inner(self.screen, self));
        self.log_scroll = self.log_scroll.saturating_add_signed(delta).min(max);
    }

    /// Run the command that just failed again, from the list.
    pub fn retry_last_failed(&mut self) {
        match self.jobs.last_failed() {
            Some(id) => self.retry(id),
            None => self.notice = Some("nothing failed to retry".into()),
        }
    }

    /// Run a finished job's command again. The log follows it when it was open.
    pub fn retry(&mut self, id: u64) {
        let Some(new) = self.jobs.retry(id) else {
            self.notice = Some("still running — retry once it has finished".into());
            return;
        };
        let label = self
            .jobs
            .get(new)
            .map(|j| j.label.clone())
            .unwrap_or_default();
        self.notice = Some(format!("↻ {label} — queued again"));
        if self.log_view.is_some() {
            self.log_view = Some(new);
            self.log_scroll = 0;
        }
    }

    /// Run the open log's command again and follow the new run.
    pub fn retry_log(&mut self) {
        let Some(id) = self.log_view else { return };
        match self.jobs.retry(id) {
            Some(new) => {
                let label = self
                    .jobs
                    .get(new)
                    .map(|j| j.label.clone())
                    .unwrap_or_default();
                self.notice = Some(format!("↻ {label} — queued again"));
                self.log_view = Some(new);
                self.log_scroll = 0;
            }
            None => self.notice = Some("still running — retry once it has finished".into()),
        }
    }

    fn set_sidebar_width(&mut self, width: i32) {
        let max = i32::from(self.sidebar_max.max(crate::tui::ui::SIDEBAR_MIN));
        self.sidebar_width = width.clamp(i32::from(crate::tui::ui::SIDEBAR_MIN), max) as u16;
    }

    pub fn resizing(&self) -> bool {
        self.resize_grab.is_some()
    }

    /// A press on the divider: grab it where it was pressed — so the border
    /// does not jump to the pointer — or, pressed twice quickly, reset it.
    pub fn press_divider(&mut self, col: u16, at: std::time::Instant) {
        let double = self
            .last_divider_press
            .is_some_and(|t| at.duration_since(t) <= DOUBLE_CLICK);
        if double {
            self.last_divider_press = None;
            self.resize_grab = None;
            self.sidebar_width = SIDEBAR_DEFAULT;
            return;
        }
        self.last_divider_press = Some(at);
        let width = i32::from(self.sidebar_width.min(self.sidebar_max));
        self.resize_grab = Some(width - i32::from(col));
    }

    /// The pointer moved with the divider held.
    pub fn drag_divider(&mut self, col: u16) {
        if let Some(grab) = self.resize_grab {
            self.set_sidebar_width(i32::from(col) + grab);
        }
    }

    pub fn release_divider(&mut self) {
        self.resize_grab = None;
    }

    /// A click on a worktree does what the arrow keys do: select it, and bring
    /// the focus back to the list if it was in a shell. Not while a menu or a
    /// popup is open — those are modal, and a click must not reach behind them.
    pub fn click_worktree(&mut self, index: usize) {
        if self.menu.is_some() || self.form.is_some() || index >= self.worktrees.len() {
            return;
        }
        self.notice = None;
        self.selected = index;
        self.focus = Focus::List;
        self.log_view = None;
    }

    /// Every tab's foreground process, as (tab id, pid), for the agents poll.
    pub fn foreground_pids(&self) -> Vec<(u64, u32)> {
        self.workspaces
            .values()
            .flat_map(|ws| ws.tabs.iter())
            .filter_map(|t| t.pane.foreground_pid().map(|pid| (t.id, pid)))
            .collect()
    }

    /// The poll's answer: what each tab's foreground process is.
    pub fn set_agent_kinds(&mut self, kinds: HashMap<u64, &'static str>) {
        self.agent_kinds = kinds;
    }

    /// The agents running in tabs, in worktree-list order, then tab order.
    pub fn agents(&self) -> Vec<AgentRow> {
        let visible = self
            .selected_workspace()
            .and_then(Workspace::active_tab)
            .map(|t| t.id);
        let mut rows = Vec::new();
        for w in &self.worktrees {
            let Some(ws) = self.workspaces.get(&w.name) else {
                continue;
            };
            for (i, tab) in ws.tabs.iter().enumerate() {
                let Some(kind) = self.agent_kinds.get(&tab.id).copied() else {
                    continue;
                };
                let p = &tab.pane;
                rows.push(AgentRow {
                    worktree: w.name.clone(),
                    tab_id: tab.id,
                    tab_index: i,
                    title: tab
                        .name
                        .clone()
                        .or_else(|| p.title())
                        .unwrap_or_else(|| kind.to_string()),
                    status: agents::status(
                        p.since_output(),
                        p.bell_pending(),
                        p.unseen_output(),
                        visible == Some(tab.id),
                    ),
                });
            }
        }
        rows
    }

    /// Bring an agent's tab on screen and type into it.
    fn jump_to(&mut self, worktree: &str, tab_index: usize) {
        if let Some(i) = self.worktrees.iter().position(|w| w.name == worktree) {
            self.selected = i;
            self.switch_tab(tab_index);
        }
    }

    /// `n`: the next agent that wants you, else the next busy one, else any —
    /// cycling past the one on screen, so pressing it again moves on.
    fn next_agent(&mut self) {
        let mut rows = self.agents();
        if rows.is_empty() {
            self.notice = Some("no agent is running".into());
            return;
        }
        rows.sort_by_key(|r| r.status);
        let current = self
            .selected_workspace()
            .and_then(Workspace::active_tab)
            .map(|t| t.id);
        let at = current.and_then(|id| rows.iter().position(|r| r.tab_id == id));
        let next = &rows[at.map_or(0, |i| (i + 1) % rows.len())];
        let (w, t) = (next.worktree.clone(), next.tab_index);
        self.jump_to(&w, t);
    }

    /// A click on a row of the agents pane.
    pub fn click_agent(&mut self, index: usize) {
        if self.modal() {
            return;
        }
        if let Some(r) = self.agents().get(index) {
            let (w, t) = (r.worktree.clone(), r.tab_index);
            self.jump_to(&w, t);
        }
    }

    /// The open log's command has finished, so it can be run again.
    pub fn log_retryable(&self) -> bool {
        self.log_view
            .and_then(|id| self.jobs.get(id))
            .is_some_and(|j| matches!(j.state, JobState::Done { .. }))
    }

    fn modal(&self) -> bool {
        self.menu.is_some()
            || self.rename.is_some()
            || self.confirm_close
            || self.form.is_some()
            || self.password.is_some()
    }

    /// Typing into the password popup. Enter sends it to the job, Esc (or
    /// Ctrl-C) declines; either way what was typed is gone.
    fn password_key(&mut self, key: KeyEvent) {
        let Some(p) = self.password.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Enter => {
                let (job, value) = (p.job, std::mem::take(&mut p.value));
                self.jobs.answer(job, &value);
                self.password = None;
            }
            KeyCode::Esc => self.decline_password(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.decline_password()
            }
            KeyCode::Backspace => {
                p.value.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => p.value.push(c),
            _ => {}
        }
    }

    fn decline_password(&mut self) {
        if let Some(p) = self.password.take() {
            self.jobs.cancel_prompt(p.job);
        }
    }

    /// Open a form. The branch list comes from the add-on, once: until it is
    /// here the form's list says so, and asks the event loop to fetch it.
    pub fn open_form(&mut self, kind: FormKind) -> Effect {
        let taken: Vec<String> = self.worktrees.iter().map(|w| w.name.clone()).collect();
        let base = |name: Option<&str>| {
            self.worktrees
                .iter()
                .find(|w| Some(w.name.as_str()) == name)
                .and_then(|w| w.base.clone().or_else(|| w.branch.clone()))
                .unwrap_or_else(|| "main".into())
        };
        let default = match &kind {
            FormKind::Checkout(n) => base(Some(n)),
            _ => base(self.selected().map(|w| w.name.as_str())),
        };
        let wants = kind.wants_branches();
        let patches = kind.wants_patches().map(String::from);
        let mut form = Form::new(kind, taken, &default);
        self.menu = None;
        if wants && let Some(b) = &self.branches {
            form.set_branches(b);
        }
        self.form = Some(form);
        if let Some(site) = patches {
            // Always fresh: the open changes move by the minute.
            Effect::LoadPatches(site)
        } else if wants && self.branches.is_none() {
            Effect::LoadBranches
        } else {
            Effect::None
        }
    }

    /// The open changes arrived (or Gerrit could not be asked).
    pub fn set_patches(&mut self, result: anyhow::Result<Vec<crate::tui::forms::Change>>) {
        let Some(f) = self.form.as_mut() else { return };
        match result {
            Ok(c) => f.set_changes(c),
            Err(e) => f.error = Some(format!("no open changes: {e:#}")),
        }
    }

    /// The branch list arrived (or could not be had).
    pub fn set_branches(&mut self, result: anyhow::Result<Vec<String>>) {
        match result {
            Ok(b) => {
                if let Some(f) = self.form.as_mut() {
                    f.set_branches(&b);
                }
                self.branches = Some(b);
            }
            Err(e) => {
                if let Some(f) = self.form.as_mut() {
                    f.error = Some(format!("no branch list: {e:#}"));
                }
            }
        }
    }

    /// What closing the session would end, for the question that asks.
    pub fn close_cost(&self) -> (usize, usize) {
        let tabs = self.workspaces.values().map(|w| w.tabs.len()).sum();
        (tabs, self.agents().len())
    }

    fn tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.workspaces
            .values_mut()
            .flat_map(|ws| ws.tabs.iter_mut())
            .find(|t| t.id == id)
    }

    /// Open the rename prompt, starting from the tab's current label so a small
    /// edit stays small.
    fn start_rename(&mut self, id: u64) {
        if let Some(tab) = self.tab_mut(id) {
            let text = tab.label();
            self.rename = Some(Rename {
                tab_id: id,
                text,
                selected: true,
            });
        }
    }

    /// The prompt owns the keyboard: every printable key is text, so `q` is a
    /// letter here, not quit.
    fn rename_key(&mut self, key: KeyEvent) -> Effect {
        let Some(r) = self.rename.as_mut() else {
            return Effect::None;
        };
        match key.code {
            KeyCode::Esc => self.rename = None,
            KeyCode::Enter => {
                let (id, text) = (r.tab_id, r.text.trim().to_string());
                self.rename = None;
                if let Some(tab) = self.tab_mut(id) {
                    tab.name = (!text.is_empty()).then_some(text);
                }
            }
            KeyCode::Backspace if r.selected => r.text.clear(),
            KeyCode::Backspace => {
                r.text.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => r.text.clear(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if r.selected {
                    r.text.clear();
                }
                r.text.push(c);
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {}
            _ => return Effect::None,
        }
        if let Some(r) = self.rename.as_mut() {
            r.selected = false;
        }
        Effect::None
    }

    /// Shift the active tab one place left or right; it stays the active one.
    fn move_tab(&mut self, delta: isize) {
        if let Some(ws) = self.selected_workspace_mut() {
            let to = ws.active as isize + delta;
            if to >= 0 && (to as usize) < ws.tabs.len() {
                ws.tabs.swap(ws.active, to as usize);
                ws.active = to as usize;
            }
        }
    }

    /// A tab dragged onto another: it takes that one's place, and is active.
    pub fn drop_tab(&mut self, dragged: u64, onto: u64) {
        if self.modal() || dragged == onto {
            return;
        }
        if let Some(ws) = self.selected_workspace_mut() {
            let from = ws.tabs.iter().position(|t| t.id == dragged);
            let to = ws.tabs.iter().position(|t| t.id == onto);
            if let (Some(from), Some(to)) = (from, to) {
                let tab = ws.tabs.remove(from);
                ws.tabs.insert(to, tab);
                ws.active = to;
            }
        }
    }

    /// A click on a tab at a given moment: switch to it, and a second click on
    /// the same tab soon after opens its rename prompt.
    pub fn click_tab_at(&mut self, id: u64, at: std::time::Instant) {
        let double = self
            .last_tab_click
            .is_some_and(|(last, t)| last == id && at.duration_since(t) <= DOUBLE_CLICK);
        self.click_tab(id);
        if double {
            self.last_tab_click = None;
            self.start_rename(id);
        } else {
            self.last_tab_click = Some((id, at));
        }
    }

    /// A click on a tab in the tab bar: make it active and type into it.
    pub fn click_tab(&mut self, id: u64) {
        if self.modal() {
            return;
        }
        if let Some(i) = self
            .selected_workspace()
            .and_then(|ws| ws.tabs.iter().position(|t| t.id == id))
        {
            self.switch_tab(i);
        }
    }

    /// A click on the URL in the pane title: open the site, exactly as the
    /// menu's "launch frontend" does — in the background, a failure in the
    /// footer.
    pub fn click_url(&mut self) -> Effect {
        if self.modal() {
            return Effect::None;
        }
        let Some(w) = self.selected().filter(|w| w.served()) else {
            return Effect::None;
        };
        let want = ["launch".to_string(), w.name.clone()];
        actions::for_worktree(w)
            .into_iter()
            .find_map(|e| match e {
                Entry::Action(a) if a.args == want => Some(a),
                _ => None,
            })
            .map_or(Effect::None, Effect::Run)
    }

    /// The "+ new" button (or `+`): `ddev tryout worktree add` in a popup.
    pub fn new_worktree(&mut self) -> Effect {
        if self.modal() {
            return Effect::None;
        }
        self.open_form(FormKind::NewWorktree)
    }

    /// A right-click on a worktree: select it, and open its `ddev tryout
    /// worktree` commands where the pointer is. Replaces a menu already open.
    pub fn context_menu(&mut self, index: usize, at: (u16, u16)) {
        if self.form.is_some() || self.rename.is_some() {
            return;
        }
        self.menu = None;
        self.click_worktree(index);
        if let Some(w) = self.worktrees.get(index) {
            let items = actions::for_worktree(w);
            if !items.is_empty() {
                self.menu = Some(Menu::new(&w.name.clone(), items, Some(at)));
            }
        }
    }

    /// A click while a menu is open: on an item runs it, anywhere else closes
    /// the menu — and goes no further, so it cannot select behind it.
    /// A choice in the open menu, by click or by key. An action runs and the
    /// menu closes; a `▸` opens its submenu; a separator does nothing; outside
    /// the menu (None) closes it — and goes no further.
    pub fn click_menu(&mut self, hit: Option<MenuHit>) -> Effect {
        let Some(menu) = self.menu.as_mut() else {
            return Effect::None;
        };
        let action = match hit {
            None => None,
            Some(MenuHit::Top(i)) => match menu.items.get(i) {
                Some(Entry::Action(a)) => Some(a.clone()),
                Some(Entry::Sub { .. }) => {
                    menu.selected = i;
                    menu.sub = Some(0);
                    return Effect::None;
                }
                _ => return Effect::None,
            },
            Some(MenuHit::Sub(i)) => menu.sub_items().and_then(|s| s.get(i)).cloned(),
        };
        self.menu = None;
        match action {
            Some(Action {
                run: Run::Form(kind),
                ..
            }) => self.open_form(kind),
            Some(a) => Effect::Run(a),
            None => Effect::None,
        }
    }

    /// A click on the tab bar's `+`.
    pub fn click_new_tab(&mut self) -> Effect {
        if self.modal() {
            return Effect::None;
        }
        self.selected()
            .map_or(Effect::None, |w| Effect::NewTab(w.name.clone()))
    }

    /// A click inside the terminal area focuses it, when there is one to focus.
    pub fn click_pane(&mut self) {
        if !self.modal() && self.selected_workspace().is_some() {
            self.focus = Focus::Pane;
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
            php_versions: Vec::new(),
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

    #[test]
    fn a_job_asking_for_a_password_gets_a_popup_that_answers_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(dir.path().to_path_buf());
        a.jobs = Jobs::new(crate::tui::jobs::tests::asking(dir.path()));
        let action = Action {
            label: "Serve".into(),
            hint: String::new(),
            args: vec!["x".into()],
            run: Run::Background,
        };
        let id = a.jobs.enqueue(&action, false);
        let t = std::time::Instant::now();
        while a.password.is_none() {
            a.tick();
            assert!(t.elapsed() < std::time::Duration::from_secs(10), "no popup");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(a.password.as_ref().unwrap().prompt, "Password:");
        // Everything goes to the popup while it is open — even q.
        for c in "secreq".chars() {
            a.handle_key(KeyEvent::new(KeyCode::Char(c), M::NONE));
        }
        a.handle_key(KeyEvent::new(KeyCode::Backspace, M::NONE));
        a.handle_key(KeyEvent::new(KeyCode::Char('t'), M::NONE));
        assert_eq!(a.password.as_ref().unwrap().value, "secret");
        a.handle_key(KeyEvent::new(KeyCode::Enter, M::NONE));
        assert!(a.password.is_none());
        while !matches!(a.jobs.get(id).unwrap().state, JobState::Done { .. }) {
            a.tick();
            assert!(
                t.elapsed() < std::time::Duration::from_secs(10),
                "never finished"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(matches!(
            a.jobs.get(id).unwrap().state,
            JobState::Done { ok: true, .. }
        ));
    }

    #[test]
    fn escape_declines_the_password_and_the_job_fails() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(dir.path().to_path_buf());
        a.jobs = Jobs::new(crate::tui::jobs::tests::asking(dir.path()));
        let action = Action {
            label: "Serve".into(),
            hint: String::new(),
            args: vec!["x".into()],
            run: Run::Background,
        };
        let id = a.jobs.enqueue(&action, false);
        let t = std::time::Instant::now();
        while a.password.is_none() {
            a.tick();
            assert!(t.elapsed() < std::time::Duration::from_secs(10), "no popup");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        a.handle_key(KeyEvent::new(KeyCode::Esc, M::NONE));
        assert!(a.password.is_none());
        while !matches!(a.jobs.get(id).unwrap().state, JobState::Done { .. }) {
            a.tick();
            assert!(
                t.elapsed() < std::time::Duration::from_secs(10),
                "never finished"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(matches!(
            a.jobs.get(id).unwrap().state,
            JobState::Done { ok: false, .. }
        ));
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
        assert_eq!(press(&mut a, KeyCode::Enter), Effect::NewTab("v13".into()));
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
    fn the_menu_offers_the_selected_worktrees_commands_and_runs_one() {
        let mut a = app();
        press(&mut a, KeyCode::End);
        press(&mut a, KeyCode::Char('a'));
        let menu = a.menu.as_ref().unwrap();
        assert_eq!(menu.worktree, "bugfix");
        assert_eq!(menu.items[0].label(), "Serve");
        let Effect::Run(action) = press(&mut a, KeyCode::Enter) else {
            panic!("no run")
        };
        assert_eq!(action.command_line(), "ddev tryout worktree serve bugfix");
        assert!(a.menu.is_none());
    }

    #[test]
    fn the_php_submenu_is_entered_and_left_with_the_arrow_keys() {
        let mut a = app();
        a.worktrees[2].php_versions = vec!["8.2".into(), "8.3".into()];
        press(&mut a, KeyCode::End);
        press(&mut a, KeyCode::Char('a'));
        press(&mut a, KeyCode::Down); // Serve on PHP ▸
        press(&mut a, KeyCode::Right);
        assert_eq!(a.menu.as_ref().unwrap().sub, Some(0));
        press(&mut a, KeyCode::Left);
        assert_eq!(a.menu.as_ref().unwrap().sub, None, "left steps back out");
        press(&mut a, KeyCode::Enter); // Enter opens it too
        press(&mut a, KeyCode::Down);
        let Effect::Run(action) = press(&mut a, KeyCode::Enter) else {
            panic!("no run")
        };
        assert_eq!(
            action.command_line(),
            "ddev tryout worktree serve bugfix --php 8.3"
        );
    }

    #[test]
    fn the_selection_steps_over_separators() {
        let mut a = app();
        a.context_menu(2, (1, 1)); // Serve, Make primary, —, Rename…, Remove…
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Down);
        assert_eq!(a.menu.as_ref().unwrap().selected, 3, "the rule is skipped");
        for _ in 0..5 {
            press(&mut a, KeyCode::Down);
        }
        assert_eq!(
            a.menu.as_ref().unwrap().selected,
            4,
            "and it stops at the end"
        );
    }

    #[test]
    fn escape_closes_the_menu_and_runs_nothing() {
        let mut a = app();
        press(&mut a, KeyCode::Char(' '));
        assert_eq!(press(&mut a, KeyCode::Esc), Effect::None);
        assert!(a.menu.is_none());
    }

    #[test]
    fn a_click_selects_like_the_arrow_keys_and_leaves_the_shell() {
        let mut a = app();
        a.focus = Focus::Pane;
        a.click_worktree(2);
        assert_eq!(a.selected().unwrap().name, "bugfix");
        assert_eq!(a.focus, Focus::List);
    }

    #[test]
    fn a_click_does_not_reach_behind_the_menu() {
        let mut a = app();
        press(&mut a, KeyCode::Char('a'));
        a.click_worktree(2);
        assert_eq!(a.selected, 0);
        assert!(a.menu.is_some());
    }

    fn quiet_pane() -> Pane {
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "cat"]);
        Pane::spawn(cmd, 5, 30).unwrap()
    }

    /// The app with `n` tabs open in the primary ("main").
    fn with_tabs(n: usize) -> App {
        let mut a = app();
        for _ in 0..n {
            a.add_tab("main", quiet_pane());
        }
        a
    }

    fn active(a: &App) -> usize {
        a.selected_workspace().unwrap().active
    }

    #[test]
    fn t_opens_another_tab_and_enter_reuses_the_active_one() {
        let mut a = app();
        assert_eq!(press(&mut a, KeyCode::Enter), Effect::NewTab("main".into()));
        let mut a = with_tabs(1);
        a.focus = Focus::List;
        assert_eq!(press(&mut a, KeyCode::Enter), Effect::None);
        assert_eq!(a.focus, Focus::Pane);
        a.focus = Focus::List;
        assert_eq!(
            press(&mut a, KeyCode::Char('t')),
            Effect::NewTab("main".into())
        );
    }

    #[test]
    fn a_new_tab_is_the_active_one_with_focus() {
        let a = with_tabs(3);
        assert_eq!(a.selected_workspace().unwrap().tabs.len(), 3);
        assert_eq!(active(&a), 2);
        assert_eq!(a.focus, Focus::Pane);
    }

    #[test]
    fn digits_and_brackets_switch_tabs() {
        let mut a = with_tabs(3);
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char('1'));
        assert_eq!((active(&a), a.focus), (0, Focus::Pane));
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char('['));
        assert_eq!(active(&a), 2, "wraps backwards");
        // Switching focused the shell, so the next tab key comes via Ctrl-G.
        a.handle_key(KeyEvent::new(KeyCode::Char('g'), M::CONTROL));
        press(&mut a, KeyCode::Char(']'));
        assert_eq!(active(&a), 0, "and forwards");
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char('9'));
        assert_eq!(active(&a), 0, "a tab that does not exist changes nothing");
    }

    #[test]
    fn w_closes_the_active_tab_and_the_last_one_takes_the_workspace() {
        let mut a = with_tabs(2);
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char('w'));
        assert_eq!(a.selected_workspace().unwrap().tabs.len(), 1);
        press(&mut a, KeyCode::Char('w'));
        assert!(a.selected_workspace().is_none());
        assert_eq!(a.focus, Focus::List);
    }

    #[test]
    fn a_tab_whose_program_exited_closes_itself() {
        let mut a = with_tabs(1);
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "exit 0"]);
        a.add_tab("main", Pane::spawn(cmd, 5, 30).unwrap());
        let t = std::time::Instant::now();
        while a.selected_workspace().unwrap().tabs[1].pane.is_alive() {
            assert!(t.elapsed().as_secs() < 5);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        a.tick();
        let ws = a.selected_workspace().unwrap();
        assert_eq!((ws.tabs.len(), ws.active), (1, 0));
    }

    #[test]
    fn clicks_on_a_tab_and_on_plus() {
        let mut a = with_tabs(2);
        let first = a.selected_workspace().unwrap().tabs[0].id;
        a.focus = Focus::List;
        a.click_tab(first);
        assert_eq!((active(&a), a.focus), (0, Focus::Pane));
        assert_eq!(a.click_new_tab(), Effect::NewTab("main".into()));
        a.focus = Focus::List;
        a.click_pane();
        assert_eq!(a.focus, Focus::Pane);
    }

    #[test]
    fn a_reload_keeps_the_tabs() {
        let mut a = with_tabs(2);
        a.set_worktrees(Ok(fixture()));
        assert_eq!(a.selected_workspace().unwrap().tabs.len(), 2);
    }

    /// Tabs in two worktrees, the 2nd and 3rd marked as agents.
    fn with_agents() -> (App, u64, u64) {
        let mut a = with_tabs(1);
        a.add_tab("main", quiet_pane());
        a.add_tab("bugfix", quiet_pane());
        let ids: Vec<u64> = a
            .workspaces
            .values()
            .flat_map(|w| w.tabs.iter().map(|t| t.id))
            .collect();
        let (main2, bug1) = (
            a.workspaces["main"].tabs[1].id,
            a.workspaces["bugfix"].tabs[0].id,
        );
        assert_eq!(ids.len(), 3);
        a.set_agent_kinds(HashMap::from([(main2, "claude"), (bug1, "codex")]));
        a.focus = Focus::List;
        (a, main2, bug1)
    }

    #[test]
    fn only_tabs_running_an_agent_are_listed_in_worktree_order() {
        let (a, main2, bug1) = with_agents();
        let rows = a.agents();
        let got: Vec<_> = rows
            .iter()
            .map(|r| (r.worktree.as_str(), r.tab_id, r.title.as_str()))
            .collect();
        assert_eq!(got, [("main", main2, "claude"), ("bugfix", bug1, "codex")]);
        assert_eq!(
            rows[0].title, "claude",
            "no terminal title yet: the agent's name"
        );
    }

    #[test]
    fn n_jumps_to_the_next_agent_and_cycles() {
        let (mut a, main2, bug1) = with_agents();
        let on = |a: &App| a.selected_workspace().unwrap().active_tab().unwrap().id;
        press(&mut a, KeyCode::Char('n'));
        let first = on(&a);
        assert_eq!(a.focus, Focus::Pane);
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char('n'));
        let second = on(&a);
        assert_ne!(first, second);
        assert!([main2, bug1].contains(&first) && [main2, bug1].contains(&second));
    }

    #[test]
    fn a_click_on_an_agent_opens_its_tab() {
        let (mut a, _, bug1) = with_agents();
        a.click_agent(1);
        assert_eq!(a.selected().unwrap().name, "bugfix");
        assert_eq!(
            a.selected_workspace().unwrap().active_tab().unwrap().id,
            bug1
        );
        assert_eq!(a.focus, Focus::Pane);
    }

    #[test]
    fn an_agent_that_finishes_while_you_look_elsewhere_waits_for_you() {
        let mut a = app();
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "sleep 1; printf 'done\\007'; read x"]);
        a.add_tab("main", Pane::spawn(cmd, 5, 30).unwrap());
        let id = a.workspaces["main"].tabs[0].id;
        a.set_agent_kinds(HashMap::from([(id, "claude")]));
        a.tick(); // on screen: seen
        a.focus = Focus::List;
        press(&mut a, KeyCode::Down); // look elsewhere before it finishes
        let t = std::time::Instant::now();
        while !a.workspaces["main"].tabs[0].pane.bell_pending() {
            assert!(t.elapsed().as_secs() < 5, "the bell never arrived");
            a.tick();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        a.tick();
        assert_eq!(a.agents()[0].status, Status::Waiting);
    }

    fn ids(a: &App) -> Vec<u64> {
        a.selected_workspace()
            .unwrap()
            .tabs
            .iter()
            .map(|t| t.id)
            .collect()
    }

    fn typing(a: &mut App, text: &str) {
        for c in text.chars() {
            press(a, KeyCode::Char(c));
        }
    }

    #[test]
    fn comma_renames_the_active_tab_and_an_empty_name_goes_back_to_automatic() {
        let mut a = with_tabs(2);
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char(','));
        let r = a.rename.as_ref().expect("no rename prompt");
        assert_eq!(r.text, "shell", "starts from the current label");
        press(&mut a, KeyCode::Backspace); // the whole old name is selected
        typing(&mut a, "tests q");
        press(&mut a, KeyCode::Enter);
        assert!(a.rename.is_none());
        let ws = a.selected_workspace().unwrap();
        assert_eq!(
            ws.tabs[1].label(),
            "tests q",
            "q is text in the prompt, not quit"
        );
        assert_eq!(ws.tabs[0].label(), "shell", "only the active tab");

        a.focus = Focus::List;
        press(&mut a, KeyCode::Char(','));
        press(&mut a, KeyCode::Backspace);
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.selected_workspace().unwrap().tabs[1].name, None);
    }

    #[test]
    fn the_old_name_starts_selected_so_typing_replaces_it() {
        let mut a = with_tabs(1);
        a.workspaces.get_mut("main").unwrap().tabs[0].name =
            Some("user@host: ~/Development/a/very/long/path".into());
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char(','));
        typing(&mut a, "api");
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.selected_workspace().unwrap().tabs[0].label(), "api");

        // Editing instead: a Right arrow keeps the name and appends after it.
        press(&mut a, KeyCode::Char(','));
        press(&mut a, KeyCode::Right);
        typing(&mut a, "-v2");
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.selected_workspace().unwrap().tabs[0].label(), "api-v2");

        // And Ctrl-U clears whatever is there.
        press(&mut a, KeyCode::Char(','));
        press(&mut a, KeyCode::Right);
        a.handle_key(KeyEvent::new(KeyCode::Char('u'), M::CONTROL));
        typing(&mut a, "x");
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.selected_workspace().unwrap().tabs[0].label(), "x");
    }

    #[test]
    fn escape_leaves_the_name_as_it_was() {
        let mut a = with_tabs(1);
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char(','));
        typing(&mut a, "zzz");
        press(&mut a, KeyCode::Esc);
        assert!(a.rename.is_none());
        assert_eq!(a.selected_workspace().unwrap().tabs[0].label(), "shell");
    }

    #[test]
    fn angle_brackets_move_the_active_tab_and_it_stays_active() {
        let mut a = with_tabs(3);
        let before = ids(&a);
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char('<'));
        assert_eq!(ids(&a), [before[0], before[2], before[1]]);
        assert_eq!(active(&a), 1, "the moved tab is still the active one");
        press(&mut a, KeyCode::Char('<'));
        press(&mut a, KeyCode::Char('<'));
        assert_eq!(ids(&a)[0], before[2], "stops at the left end");
        press(&mut a, KeyCode::Char('>'));
        assert_eq!(ids(&a), [before[0], before[2], before[1]]);
    }

    #[test]
    fn dropping_a_dragged_tab_on_another_moves_it_there() {
        let mut a = with_tabs(3);
        let before = ids(&a);
        a.drop_tab(before[0], before[2]);
        assert_eq!(ids(&a), [before[1], before[2], before[0]]);
        assert_eq!(
            a.selected_workspace().unwrap().active_tab().unwrap().id,
            before[0]
        );
        a.drop_tab(before[1], before[1]);
        assert_eq!(
            ids(&a),
            [before[1], before[2], before[0]],
            "onto itself: nothing"
        );
    }

    #[test]
    fn a_double_click_on_a_tab_renames_it_a_slow_one_does_not() {
        let mut a = with_tabs(2);
        let id = ids(&a)[0];
        let t0 = std::time::Instant::now();
        a.click_tab_at(id, t0);
        a.click_tab_at(id, t0 + std::time::Duration::from_millis(900));
        assert!(a.rename.is_none(), "too slow for a double-click");
        a.click_tab_at(id, t0 + std::time::Duration::from_millis(1100));
        assert_eq!(a.rename.as_ref().map(|r| r.tab_id), Some(id));
    }

    #[test]
    fn a_renamed_agent_keeps_your_name_in_the_agents_pane() {
        let (mut a, main2, _) = with_agents();
        a.workspaces.get_mut("main").unwrap().tabs[1].name = Some("refactor".into());
        let row = a.agents().into_iter().find(|r| r.tab_id == main2).unwrap();
        assert_eq!(row.title, "refactor");
    }

    #[test]
    fn a_click_on_the_url_launches_that_worktrees_site() {
        let mut a = app();
        press(&mut a, KeyCode::Down); // v13, served
        let Effect::Run(action) = a.click_url() else {
            panic!("no launch")
        };
        assert_eq!(action.args, ["launch", "v13"]);
        assert_eq!(action.run, Run::Background, "a browser, outside the queue");
        press(&mut a, KeyCode::Down); // bugfix, not served: no URL, no launch
        assert_eq!(a.click_url(), Effect::None);
        press(&mut a, KeyCode::Up);
        press(&mut a, KeyCode::Char('a'));
        assert_eq!(a.click_url(), Effect::None, "not behind the menu");
    }

    #[test]
    fn plus_opens_the_new_worktree_form_and_fetches_the_branches_once() {
        let mut a = app();
        assert_eq!(press(&mut a, KeyCode::Char('+')), Effect::LoadBranches);
        assert_eq!(a.form.as_ref().unwrap().kind, FormKind::NewWorktree);
        a.set_branches(Ok(vec!["main".into(), "13.4".into()]));
        press(&mut a, KeyCode::Esc);
        assert!(a.form.is_none());
        // The second time the list is already here: no fetch.
        assert_eq!(press(&mut a, KeyCode::Char('+')), Effect::None);
    }

    #[test]
    fn a_filled_in_form_becomes_a_queued_job() {
        let mut a = app();
        press(&mut a, KeyCode::Char('+'));
        a.set_branches(Ok(vec!["main".into(), "13.4".into()]));
        typing(&mut a, "feature-x");
        let Effect::Run(action) = press(&mut a, KeyCode::Enter) else {
            panic!("no job")
        };
        assert_eq!(
            action.command_line(),
            "ddev tryout worktree add feature-x main --serve"
        );
        assert_eq!(action.run, Run::Job { reveal: false });
        assert!(a.form.is_none());
    }

    #[test]
    fn keys_go_to_the_form_not_the_list_behind_it() {
        let mut a = app();
        press(&mut a, KeyCode::Char('+'));
        typing(&mut a, "q");
        assert!(a.form.is_some(), "q is a letter in the name, not detach");
    }

    #[test]
    fn a_right_click_selects_the_worktree_and_opens_its_commands_there() {
        let mut a = app();
        a.context_menu(2, (10, 7));
        assert_eq!(a.selected().unwrap().name, "bugfix");
        let menu = a.menu.as_ref().unwrap();
        assert_eq!(menu.anchor, Some((10, 7)));
        assert_eq!(menu.items[0].label(), "Serve");
        // Another right-click moves the menu to the other worktree.
        a.context_menu(1, (10, 5));
        assert_eq!(a.menu.as_ref().unwrap().worktree, "v13");
    }

    #[test]
    fn a_click_on_a_menu_item_runs_it_and_anywhere_else_closes_it() {
        let mut a = app();
        a.context_menu(2, (10, 7));
        assert_eq!(a.click_menu(Some(MenuHit::Top(2))), Effect::None);
        assert!(a.menu.is_some(), "a click on the rule does nothing");
        // Remove… asks first, natively; only y runs it, fully argued.
        assert_eq!(a.click_menu(Some(MenuHit::Top(4))), Effect::None);
        assert!(a.menu.is_none());
        assert_eq!(
            a.form.as_ref().unwrap().kind,
            FormKind::Remove("bugfix".into())
        );
        let Effect::Run(action) = press(&mut a, KeyCode::Char('y')) else {
            panic!("no run")
        };
        assert_eq!(
            action.command_line(),
            "ddev tryout worktree remove bugfix --yes"
        );
        a.context_menu(2, (10, 7));
        assert_eq!(a.click_menu(None), Effect::None);
        assert!(a.menu.is_none());
    }

    #[test]
    fn q_detaches_and_only_a_confirmed_capital_q_closes() {
        let mut a = app();
        assert_eq!(press(&mut a, KeyCode::Char('q')), Effect::Detach);
        assert_eq!(press(&mut a, KeyCode::Char('Q')), Effect::None);
        assert!(a.confirm_close);
        assert_eq!(press(&mut a, KeyCode::Char('n')), Effect::None);
        assert!(!a.confirm_close, "anything but y keeps the session");
        press(&mut a, KeyCode::Char('Q'));
        assert_eq!(press(&mut a, KeyCode::Char('y')), Effect::CloseSession);
    }

    #[test]
    fn closing_says_what_it_would_end() {
        let (a, _, _) = with_agents();
        assert_eq!(a.close_cost(), (3, 2));
    }

    /// The app with jobs run by the fake ddev from jobs::tests.
    fn with_fake_jobs(dir: &std::path::Path) -> App {
        let mut a = app();
        a.root = dir.to_path_buf();
        a.jobs = Jobs::new(crate::tui::jobs::tests::fake(dir));
        a
    }

    fn run_to_end(a: &mut App) -> Effect {
        let t = std::time::Instant::now();
        let mut last = Effect::None;
        while a
            .jobs
            .list()
            .iter()
            .any(|j| !matches!(j.state, JobState::Done { .. }))
        {
            assert!(t.elapsed().as_secs() < 10, "the job never ended");
            let e = a.tick();
            if e != Effect::None {
                last = e;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        last
    }

    fn job_action(args: &str, reveal: bool) -> Action {
        Action {
            label: args.into(),
            hint: String::new(),
            args: args.split_whitespace().map(String::from).collect(),
            run: Run::Job { reveal },
        }
    }

    #[test]
    fn a_status_job_shows_its_output_when_done() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = with_fake_jobs(dir.path());
        let id = a.jobs.enqueue(&job_action("status", true), true);
        assert_eq!(run_to_end(&mut a), Effect::None, "status changes nothing");
        assert_eq!(a.log_view, Some(id));
        assert_eq!(a.notice.as_deref(), Some("✓ status"));
        press(&mut a, KeyCode::Esc);
        assert_eq!(a.log_view, None);
    }

    #[test]
    fn a_failed_job_says_why_and_where_the_log_is() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = with_fake_jobs(dir.path());
        a.jobs.enqueue(&job_action("fail", false), false);
        run_to_end(&mut a);
        assert_eq!(
            a.notice.as_deref(),
            Some("✗ fail: it broke — R retries · L shows the log")
        );
        assert_eq!(
            a.log_view, None,
            "a failure does not take the pane by itself"
        );
        press(&mut a, KeyCode::Char('L'));
        assert!(a.log_view.is_some());
    }

    #[test]
    fn a_job_that_changes_worktrees_reloads_the_list() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = with_fake_jobs(dir.path());
        a.jobs
            .enqueue(&job_action("worktree serve v13", false), false);
        assert_eq!(run_to_end(&mut a), Effect::Reload);
        assert_eq!(a.listing, Listing::Loading);
    }

    #[test]
    fn the_log_scrolls_back_and_forth_within_its_length() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = with_fake_jobs(dir.path());
        let id = a.jobs.enqueue(&job_action("status", true), true);
        run_to_end(&mut a);
        a.open_log(id);
        // A log shorter than the screen has nothing to scroll back to.
        press(&mut a, KeyCode::PageUp);
        assert_eq!(a.log_scroll, 0);
        // A worktree click leaves the log.
        a.click_worktree(1);
        assert_eq!(a.log_view, None);
    }

    #[test]
    fn a_long_log_scrolls_to_its_top_and_back_without_dead_keys() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("long");
        std::fs::write(
            &script,
            "#!/bin/sh\nfor i in $(seq 1 100); do echo \"line $i\"; done\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut a = app();
        a.root = dir.path().to_path_buf();
        a.jobs = Jobs::new(script);
        let id = a.jobs.enqueue(&job_action("status", true), true);
        run_to_end(&mut a);
        a.open_log(id);
        let max = crate::tui::ui::log_max_scroll(
            a.jobs.get(id).unwrap(),
            crate::tui::ui::log_inner(a.screen, &a),
        );
        assert!(max > 50, "100 lines on a 24-row screen: {max}");
        press(&mut a, KeyCode::Home);
        assert_eq!(a.log_scroll, max);
        press(&mut a, KeyCode::Up);
        assert_eq!(a.log_scroll, max, "the top is the top");
        press(&mut a, KeyCode::Down);
        assert_eq!(
            a.log_scroll,
            max - 1,
            "and the first key back moves at once"
        );
        a.scroll_log(-3);
        assert_eq!(a.log_scroll, max - 4, "the wheel's three rows");
        press(&mut a, KeyCode::End);
        assert_eq!(a.log_scroll, 0);
    }

    #[test]
    fn shift_r_retries_the_command_that_just_failed() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = with_fake_jobs(dir.path());
        press(&mut a, KeyCode::Char('R'));
        assert_eq!(a.notice.as_deref(), Some("nothing failed to retry"));
        a.jobs.enqueue(&job_action("fail", false), false);
        run_to_end(&mut a);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("R retries")));
        press(&mut a, KeyCode::Char('R'));
        assert!(
            a.notice
                .as_deref()
                .is_some_and(|n| n.contains("queued again"))
        );
        run_to_end(&mut a);
        assert_eq!(
            a.jobs.list().iter().filter(|j| j.label == "fail").count(),
            2
        );
        // A success since then leaves nothing to retry.
        a.jobs.enqueue(&job_action("ok", false), false);
        run_to_end(&mut a);
        press(&mut a, KeyCode::Char('R'));
        assert_eq!(a.notice.as_deref(), Some("nothing failed to retry"));
    }

    #[test]
    fn r_runs_a_finished_command_again_and_follows_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = with_fake_jobs(dir.path());
        let first = a.jobs.enqueue(&job_action("fail", true), true);
        run_to_end(&mut a);
        a.open_log(first);
        assert!(a.log_retryable());
        press(&mut a, KeyCode::Char('r'));
        let again = a.log_view.unwrap();
        assert_ne!(again, first, "the log follows the new run");
        assert!(
            a.notice
                .as_deref()
                .is_some_and(|n| n.contains("queued again"))
        );
        a.tick();
        assert!(!a.log_retryable(), "not while it runs");
        press(&mut a, KeyCode::Char('r'));
        assert_eq!(a.log_view, Some(again));
        run_to_end(&mut a);
        assert_eq!(
            a.jobs.list().iter().filter(|j| j.label == "fail").count(),
            2
        );
    }

    #[test]
    fn dragging_the_divider_follows_the_pointer_from_where_it_was_grabbed() {
        let mut a = app();
        let t = std::time::Instant::now();
        // Grabbed on the pane's side of the divider (column 34, width 34).
        a.press_divider(34, t);
        assert!(a.resizing());
        a.drag_divider(50);
        assert_eq!(a.sidebar_width, 50, "the grab offset is kept");
        a.drag_divider(10);
        assert_eq!(
            a.sidebar_width,
            crate::tui::ui::SIDEBAR_MIN,
            "not below the minimum"
        );
        a.release_divider();
        assert!(!a.resizing());
        a.drag_divider(60);
        assert_eq!(
            a.sidebar_width,
            crate::tui::ui::SIDEBAR_MIN,
            "no drag without a grab"
        );
    }

    #[test]
    fn a_double_click_on_the_divider_resets_it() {
        let mut a = app();
        a.sidebar_width = 60;
        let t = std::time::Instant::now();
        a.press_divider(59, t);
        a.release_divider();
        a.press_divider(59, t + std::time::Duration::from_millis(200));
        assert_eq!(a.sidebar_width, SIDEBAR_DEFAULT);
        assert!(!a.resizing(), "a reset is not a drag");
    }

    #[test]
    fn braces_resize_the_sidebar_by_four_within_what_the_screen_allows() {
        let mut a = app();
        a.sidebar_max = 60;
        press(&mut a, KeyCode::Char('}'));
        assert_eq!(a.sidebar_width, SIDEBAR_DEFAULT + 4);
        for _ in 0..20 {
            press(&mut a, KeyCode::Char('}'));
        }
        assert_eq!(
            a.sidebar_width, 60,
            "stops where the pane would get too small"
        );
        for _ in 0..20 {
            press(&mut a, KeyCode::Char('{'));
        }
        assert_eq!(a.sidebar_width, crate::tui::ui::SIDEBAR_MIN);
    }

    #[test]
    fn n_without_agents_says_so() {
        let mut a = with_tabs(1);
        a.focus = Focus::List;
        press(&mut a, KeyCode::Char('n'));
        assert_eq!(a.notice.as_deref(), Some("no agent is running"));
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
