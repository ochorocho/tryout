//! tryout-tui — a terminal workspace for a ddev tryout project: its Core
//! worktrees on the left, a live shell in the selected one on the right.

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

use app::{App, Effect, Focus};
use pane::Pane;

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
    spawn_load(&app.root, &tx);
    let mut redraw = true;

    loop {
        let size = terminal.size()?;
        let inner = ui::areas(Rect::new(0, 0, size.width, size.height)).pane_inner;
        for pane in app.panes.values_mut() {
            pane.resize(inner.height, inner.width);
        }
        redraw |= receive(&rx, &mut app);
        // Every pane's flag is cleared, not just the first dirty one found.
        redraw |= app
            .panes
            .values()
            .fold(false, |any, p| p.take_dirty() | any);

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
                }
            }
            Event::Paste(text) if app.focus == Focus::Pane => {
                if let Some(pane) = app
                    .selected()
                    .map(|w| w.name.clone())
                    .and_then(|n| app.panes.get_mut(&n))
                {
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
