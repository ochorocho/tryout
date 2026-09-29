//! The prompts as a person meets them: through the real binary, on a real
//! pseudo-terminal, and — where nobody can answer — with stdin piped.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

/// A PATH with nothing on it: no `open`/`xdg-open`, so launch prints the URL
/// instead of opening a browser.
const NO_TOOLS: &str = "/nonexistent";

/// A project with one served site (v13) beside the primary.
fn project() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("TYPO3-Instances/v13")).unwrap();
    std::fs::write(
        d.path().join("TYPO3-Instances/v13/.tryout-site"),
        "php=8.2\n",
    )
    .unwrap();
    d
}

fn env(cmd: &mut Command, root: &Path) {
    cmd.env_clear()
        .env("PATH", NO_TOOLS)
        .env("DDEV_APPROOT", root)
        .env("DDEV_SITENAME", "parity")
        .env("DDEV_PRIMARY_URL", "https://parity.ddev.site")
        .current_dir("/");
}

#[test]
fn a_piped_answer_picks_without_a_terminal() {
    let d = project();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tryout"));
    env(&mut cmd, d.path());
    let mut child = cmd
        .arg("launch")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"v13\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Open: https://v13.parity.ddev.site"));
}

#[test]
fn with_nobody_to_answer_the_usage_line_is_the_answer() {
    let d = project();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tryout"));
    env(&mut cmd, d.path());
    let out = cmd.arg("launch").stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("Usage: ddev tryout launch [<worktree>] [--backend]")
    );
}

/// Run the binary on a pseudo-terminal, typing `keys` once `prompt` is on
/// screen; returns everything it printed.
fn on_a_terminal(root: &Path, args: &[&str], prompt: &str, keys: &[u8]) -> String {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_tryout"));
    cmd.args(args);
    cmd.env_clear();
    cmd.env("PATH", NO_TOOLS);
    cmd.env("TERM", "xterm-256color");
    cmd.env("DDEV_APPROOT", root);
    cmd.env("DDEV_SITENAME", "parity");
    cmd.env("DDEV_PRIMARY_URL", "https://parity.ddev.site");
    cmd.cwd("/");
    let mut child = pty.slave.spawn_command(cmd).unwrap();
    drop(pty.slave);

    let mut reader = pty.master.try_clone_reader().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut writer = pty.master.take_writer().unwrap();
    let mut seen = Vec::new();
    let mut typed = false;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
            seen.extend(chunk);
        }
        if !typed && String::from_utf8_lossy(&seen).contains(prompt) {
            writer.write_all(keys).unwrap();
            writer.flush().unwrap();
            typed = true;
        }
        if typed && child.try_wait().unwrap().is_some() {
            // Drain what is left.
            while let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
                seen.extend(chunk);
            }
            break;
        }
    }
    let _ = child.kill();
    assert!(
        typed,
        "the prompt never appeared: {}",
        String::from_utf8_lossy(&seen)
    );
    String::from_utf8_lossy(&seen).into_owned()
}

#[test]
fn enter_picks_the_highlighted_site() {
    let d = project();
    let out = on_a_terminal(d.path(), &["launch"], "Which site to open?", b"\r");
    assert!(out.contains("Open: https://parity.ddev.site"), "{out}");
}

#[test]
fn moving_down_picks_the_next_site() {
    let d = project();
    let out = on_a_terminal(d.path(), &["launch"], "Which site to open?", b"\x1b[B\r");
    assert!(out.contains("Open: https://v13.parity.ddev.site"), "{out}");
}

#[test]
fn escape_cancels() {
    let d = project();
    let out = on_a_terminal(d.path(), &["launch"], "Which site to open?", b"\x1b");
    assert!(out.contains("Cancelled"), "{out}");
    assert!(!out.contains("Open:"), "{out}");
}

/// A project whose root is a git checkout, with a worktree to remove.
fn project_with_worktree() -> tempfile::TempDir {
    let d = project();
    let git = |args: &[&str]| {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(d.path())
                .args(args)
                .output()
                .unwrap()
                .status
                .success(),
            "{args:?}"
        );
    };
    git(&["init", "-q"]);
    std::fs::create_dir_all(d.path().join("worktrees/old")).unwrap();
    d
}

#[test]
fn enter_never_confirms_a_removal() {
    let d = project_with_worktree();
    let out = on_a_terminal(
        d.path(),
        &["worktree", "remove", "old"],
        "Remove worktree 'old'",
        b"\r",
    );
    assert!(out.contains("Aborted."), "{out}");
}

#[test]
fn y_confirms_a_removal() {
    let d = project_with_worktree();
    let out = on_a_terminal(
        d.path(),
        &["worktree", "remove", "old"],
        "Remove worktree 'old'",
        b"y\r",
    );
    assert!(!out.contains("Aborted."), "{out}");
    // Confirmed, it goes on to the container — here there is no ddev to reach.
    assert!(out.contains("Could not run ddev"), "{out}");
}

#[test]
fn a_removal_with_nobody_to_ask_is_refused_not_declined() {
    let d = project_with_worktree();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tryout"));
    env(&mut cmd, d.path());
    let out = cmd
        .args(["worktree", "remove", "old"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("pass --yes"));
}
