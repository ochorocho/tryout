//! The terminal UI — a terminal workspace for a ddev tryout project: its Core
//! worktrees on the left, live shells and agents on the right. Everything runs
//! in a session server that outlives the terminal: `q` detaches, and running
//! it again attaches to the same session.

pub mod actions;
pub mod agents;
pub mod app;
pub mod forms;
pub mod jobs;
pub mod keys;
pub mod pane;
pub mod session;
pub mod ui;
pub mod worktrees;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const USAGE: &str = "\
Usage: tryout ui [<project-dir>]        attach to the project's session (starting it)
       tryout ui stop [<project-dir>]   close the session: every shell, agent and command in it

  <project-dir> defaults to the current directory; any directory inside the
  project works. q detaches — the session keeps running.";

/// `tryout ui …`: attach, `stop`, or (spawned by a client) `server <root>`.
pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("-h" | "--help") => {
            println!("{USAGE}");
            Ok(())
        }
        // Started by the client, detached; not meant to be typed.
        Some("server") => {
            let root = PathBuf::from(args.get(1).context("server needs the project root")?);
            session::server::serve(root.clone(), &session::socket_path(&root), worktrees::load)
        }
        Some("stop") => {
            let root = project(args.get(1))?;
            if session::stop(&root)? {
                println!("Session closed.");
            } else {
                println!("No session running for {}.", root.display());
            }
            Ok(())
        }
        other => {
            let root = project(other.map(String::from).as_ref())?;
            if session::is_inside(&root, std::env::var_os(session::SESSION_ENV).as_deref()) {
                anyhow::bail!(
                    "already inside this project's session — it cannot show itself\n  → Ctrl-G q detaches the terminal you are in"
                );
            }
            offer_restart_on_new_build(&root)?;
            let stream = session::connect_or_start(&root)?;
            let reason = session::client::attach(stream)?;
            println!("{reason}");
            Ok(())
        }
    }
}

/// A session keeps running the build it started with — an add-on update or a
/// rebuild does not reach it, so its new features stay invisible. Say so, and
/// offer to restart it (which closes everything in it); the default keeps it.
fn offer_restart_on_new_build(root: &Path) -> Result<()> {
    if !session::runs_another_build(root) || !crate::core::prompt::have_tty() {
        return Ok(());
    }
    eprintln!("This session was started by another build of tryout, and keeps running that one.");
    eprintln!(
        "Restarting it uses this build — and closes every shell, agent and running command in it."
    );
    if crate::core::prompt::confirm("Restart the session now?") == crate::core::prompt::Confirm::Yes
    {
        session::stop(root)?;
        // The old server says goodbye before it lets go of its socket; attach
        // only once it has, or the new client reaches the old one on its way out.
        let socket = session::socket_path(root);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while socket.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    Ok(())
}

/// The tryout project a directory (default: the current one) belongs to.
fn project(dir: Option<&String>) -> Result<PathBuf> {
    let start = match dir {
        Some(d) => PathBuf::from(d),
        None => std::env::current_dir().context("no current directory")?,
    };
    let root = worktrees::find_project_root(&start).with_context(|| {
        format!(
            "{} is not inside a ddev tryout project (no .ddev/tryout/)\n  → cd into one, or pass its path",
            start.display()
        )
    })?;
    // One session per project, however it was reached: resolve symlinks and /var
    // vs /private/var, or the same project could get two sockets.
    Ok(canonical(&root))
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}
