//! Running the tools the add-on drives (git, composer, php, mysql, nginx, …).
//! Output is flushed before a child starts, so its lines land after ours.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn flush() {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
}

/// Run with the terminal's stdout and stderr; true on success.
pub fn run(program: &str, args: &[&str], dir: Option<&Path>) -> bool {
    flush();
    let mut c = Command::new(program);
    c.args(args).stdin(Stdio::null());
    if let Some(d) = dir {
        c.current_dir(d);
    }
    c.status().is_ok_and(|s| s.success())
}

/// Run with all output discarded; true on success.
pub fn quiet(program: &str, args: &[&str], dir: Option<&Path>) -> bool {
    let mut c = Command::new(program);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(d) = dir {
        c.current_dir(d);
    }
    c.status().is_ok_and(|s| s.success())
}

/// Run and capture both streams; None when it could not start.
pub fn capture(program: &str, args: &[&str], dir: Option<&Path>) -> Option<Output> {
    let mut c = Command::new(program);
    c.args(args).stdin(Stdio::null());
    if let Some(d) = dir {
        c.current_dir(d);
    }
    c.output().ok()
}

/// `git -C <dir> <args>` with its output shown.
pub fn git(dir: &Path, args: &[&str]) -> bool {
    let _lock = super::git::lock_for(dir, args);
    let d = dir.to_string_lossy();
    let mut all = vec!["-C", &d];
    all.extend_from_slice(args);
    run("git", &all, None)
}

/// `git -C <dir> <args>` with stdout shown and stderr discarded (`2>/dev/null`).
pub fn git_no_stderr(dir: &Path, args: &[&str]) -> bool {
    let _lock = super::git::lock_for(dir, args);
    flush();
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Remove everything inside a directory, keeping the directory (`rm -rf dir/*`).
/// Hidden entries stay, as a shell glob leaves them.
pub fn clear_dir(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let p = e.path();
        let _ = if p.is_dir() && !p.is_symlink() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
    }
}
