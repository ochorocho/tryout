//! tryout-tui — a terminal workspace for a ddev tryout project: its Core
//! worktrees on the left, a live shell in the selected one on the right.

mod actions;
mod app;
mod keys;
mod pane;
mod ui;
mod worktrees;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;

use actions::{Action, Run};
use app::{App, Effect, Focus, Popup};
use pane::Pane;
use portable_pty::CommandBuilder;

type Loaded = Result<Vec<worktrees::Worktree>>;

/// How long to wait for input before looking for pane output again. Short
/// enough that a busy pane scrolls smoothly, long enough to idle at ~0% CPU.
const TICK: Duration = Duration::from_millis(16);

fn main() -> Result<()> {
    let arg = std::env::args_os().nth(1);
    if arg.as_deref().is_some_and(|a| a == "-h" || a == "--help") {
        println!("Usage: tryout-tui [<project-dir>]");
        println!("  Opens the tryout project the directory belongs to (default: the current one).");
        return Ok(());
    }
    let start = match arg {
        Some(a) => PathBuf::from(a),
        None => std::env::current_dir().context("no current directory")?,
    };
    let root = worktrees::find_project_root(&start).with_context(|| {
        format!(
            "{} is not inside a ddev tryout project (no .ddev/tryout/)\n  → cd into one, or pass its path",
            start.display()
        )
    })?;

    // init() installs a panic hook that restores the terminal, so a crash never
    // leaves the user's shell in raw mode.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, App::new(root));
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, mut app: App) -> Result<()> {
    let (tx, rx) = mpsc::channel::<Loaded>();
    let (notice_tx, notice_rx) = mpsc::channel::<String>();
    spawn_load(&app.root, &tx);
    let mut redraw = true;

    loop {
        let size = terminal.size()?;
        let screen = Rect::new(0, 0, size.width, size.height);
        let inner = ui::areas(screen).pane_inner;
        let popup_inner = ui::popup_inner(screen);
        for pane in app.panes.values_mut() {
            pane.resize(inner.height, inner.width);
        }
        if let Some(p) = app.popup.as_mut() {
            p.pane.resize(popup_inner.height, popup_inner.width);
        }
        redraw |= receive(&rx, &mut app);
        if let Ok(notice) = notice_rx.try_recv() {
            app.notice = Some(notice);
            redraw = true;
        }
        // Every pane's flag is cleared, not just the first dirty one found.
        redraw |= app
            .panes
            .values()
            .chain(app.popup.as_ref().map(|p| &p.pane))
            .fold(false, |any, p| p.take_dirty() | any);
        if app.tick() == Effect::Reload {
            spawn_load(&app.root, &tx);
            redraw = true;
        }

        if redraw {
            terminal.draw(|f| ui::draw(f, &app))?;
            redraw = false;
        }

        if !event::poll(TICK)? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                redraw = true;
                match app.handle_key(key) {
                    Effect::None => {}
                    Effect::Quit => return Ok(()),
                    Effect::Reload => spawn_load(&app.root, &tx),
                    Effect::OpenPane(name) => open_pane(&mut app, &name, inner),
                    Effect::Run(action) => match action.run {
                        Run::Popup { .. } => open_popup(&mut app, action, popup_inner),
                        Run::Background => run_in_background(&app.root, action, &notice_tx),
                    },
                }
            }
            Event::Paste(text) => {
                let target = match (&mut app.popup, app.focus) {
                    (Some(p), _) => Some(&mut p.pane),
                    (None, Focus::Pane) => app
                        .worktrees
                        .get(app.selected)
                        .map(|w| w.name.clone())
                        .and_then(|n| app.panes.get_mut(&n)),
                    _ => None,
                };
                if let Some(pane) = target {
                    pane.write(text.as_bytes());
                }
            }
            Event::Resize(..) => redraw = true,
            _ => {}
        }
    }
}

fn receive(rx: &Receiver<Loaded>, app: &mut App) -> bool {
    match rx.try_recv() {
        Ok(result) => {
            app.set_worktrees(result);
            true
        }
        Err(_) => false,
    }
}

/// The listing is a `ddev exec` — most of a second — so it never runs on the
/// thread that draws and reads keys.
fn spawn_load(root: &Path, tx: &Sender<Loaded>) {
    let (root, tx) = (root.to_path_buf(), tx.clone());
    thread::spawn(move || {
        let _ = tx.send(worktrees::load(&root));
    });
}

fn open_pane(app: &mut App, name: &str, inner: Rect) {
    let dir = app.checkout_dir(name);
    match Pane::shell(&dir, inner.height.max(1), inner.width.max(1)) {
        Ok(pane) => {
            app.panes.insert(name.to_string(), pane);
            app.focus = Focus::Pane;
        }
        Err(e) => app.notice = Some(format!("could not start a shell: {e:#}")),
    }
}

/// `ddev tryout <args>` on a PTY of its own, at the project root, in a popup.
fn open_popup(app: &mut App, action: Action, inner: Rect) {
    let mut cmd = CommandBuilder::new("ddev");
    cmd.arg("tryout");
    cmd.args(&action.args);
    cmd.cwd(&app.root);
    match Pane::spawn(cmd, inner.height.max(1), inner.width.max(1)) {
        Ok(pane) => app.popup = Some(Popup { action, pane }),
        Err(e) => app.notice = Some(format!("could not run {}: {e:#}", action.command_line())),
    }
}

/// A command with no terminal: fast and silent on success, so only its failure
/// is worth a word — and that goes to the footer, since nothing else is open.
fn run_in_background(root: &Path, action: Action, notices: &Sender<String>) {
    let (root, notices) = (root.to_path_buf(), notices.clone());
    thread::spawn(move || {
        let out = std::process::Command::new("ddev")
            .arg("tryout")
            .args(&action.args)
            .current_dir(&root)
            .output();
        let notice = match out {
            Ok(o) if o.status.success() => format!("✓ {}", action.label),
            Ok(o) => {
                let err = worktrees::strip_ansi(&String::from_utf8_lossy(&o.stderr));
                let last = err
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("failed");
                format!("{}: {}", action.label, last.trim())
            }
            Err(e) => format!("{}: {e}", action.label),
        };
        let _ = notices.send(notice);
    });
}
