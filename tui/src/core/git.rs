//! The git CLI, which stays a CLI: `worktree.useRelativePaths` and
//! `worktree repair` need git ≥ 2.48, which no library implements.

use std::path::Path;
use std::process::{Command, Stdio};

/// Whether `git <args>` writes the repository's shared state — refs, config,
/// worktree metadata — which every worktree sees. Two of those at once fail on
/// git's own `.lock` files instead of waiting, and the TUI runs jobs on
/// different worktrees side by side.
fn writes_shared_state(args: &[&str]) -> bool {
    // The subcommand: the first word that is not an option or a `-c` value.
    let mut words = args.iter();
    let sub = loop {
        match words.next() {
            Some(&"-c") => {
                words.next();
            }
            Some(w) if w.starts_with('-') => {}
            Some(w) => break *w,
            None => return false,
        }
    };
    let rest: Vec<&str> = words.copied().collect();
    match sub {
        "fetch" | "pull" | "branch" | "gc" | "prune" | "pack-refs" => true,
        "worktree" => rest.first() != Some(&"list"),
        "remote" => !matches!(rest.first(), None | Some(&"get-url" | &"show" | &"-v")),
        "config" => !rest
            .iter()
            .any(|a| a.starts_with("--get") || *a == "--list" || *a == "-l"),
        "checkout" | "switch" => rest.iter().any(|a| matches!(*a, "-b" | "-B" | "-c" | "-C")),
        _ => false,
    }
}

/// Hold the repository's own lock while a shared-state write runs, so a second
/// job waits its turn rather than failing. Released when dropped. None when the
/// command needs none, or `dir` is not (yet) a repository.
pub fn lock_for(dir: &Path, args: &[&str]) -> Option<std::fs::File> {
    if !writes_shared_state(args) {
        return None;
    }
    let common = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--git-common-dir"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let common = dir.join(String::from_utf8_lossy(&common.stdout).trim_end());
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(common.join("tryout.lock"))
        .ok()?;
    file.lock().ok()?;
    Some(file)
}

/// stdout of `git -C <dir> <args>` without its trailing newlines, or None when
/// git fails. stderr is discarded.
pub fn out(dir: &Path, args: &[&str]) -> Option<String> {
    let _lock = lock_for(dir, args);
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    o.status.success().then(|| {
        String::from_utf8_lossy(&o.stdout)
            .trim_end_matches('\n')
            .to_string()
    })
}

/// Did `git -C <dir> <args>` succeed? Output is discarded.
pub fn ok(dir: &Path, args: &[&str]) -> bool {
    let _lock = lock_for(dir, args);
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The lines of `out`, empty lines dropped.
pub fn lines(dir: &Path, args: &[&str]) -> Vec<String> {
    out(dir, args)
        .map(|s| {
            s.lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// `git --version` as (major, minor).
pub fn version() -> Option<(u32, u32)> {
    let o = Command::new("git")
        .arg("--version")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    parse_version(&String::from_utf8_lossy(&o.stdout))
}

fn parse_version(s: &str) -> Option<(u32, u32)> {
    let v = s.strip_prefix("git version ")?;
    let mut it = v.split(|c: char| !c.is_ascii_digit());
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// git ≥ 2.48 writes worktree metadata with relative paths — what lets the host
/// and the container share one worktree.
pub fn supports_relative_worktrees() -> bool {
    version().is_some_and(|v| v >= (2, 48))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_read_not_the_platform() {
        assert_eq!(parse_version("git version 2.53.0\n"), Some((2, 53)));
        assert_eq!(
            parse_version("git version 2.39.5 (Apple Git-154)"),
            Some((2, 39))
        );
        assert_eq!(parse_version("git version 2.48.0.windows.1"), Some((2, 48)));
        assert_eq!(parse_version("nonsense"), None);
        assert!((2, 47) < (2, 48) && (3, 0) >= (2, 48));
    }

    #[test]
    fn only_writes_to_shared_state_take_the_lock() {
        for w in [
            "fetch origin",
            "pull --rebase origin main",
            "config tryout.change-I1 91003",
            "-c user.name=x config worktree.useRelativePaths true",
            "worktree add --detach worktrees/v13 origin/13.4",
            "worktree prune",
            "remote add gerrit ssh://x",
            "branch -d v13",
            "checkout -b 13.4 origin/13.4",
        ] {
            let a: Vec<&str> = w.split(' ').collect();
            assert!(writes_shared_state(&a), "{w}");
        }
        for r in [
            "cherry-pick FETCH_HEAD",
            "reset --hard origin/main",
            "clean -fdx",
            "checkout -q --detach origin/13.4",
            "config --get-regexp ^tryout",
            "worktree list --porcelain",
            "remote get-url gerrit",
            "log -1",
            "",
        ] {
            let a: Vec<&str> = r.split(' ').filter(|x| !x.is_empty()).collect();
            assert!(!writes_shared_state(&a), "{r}");
        }
    }

    #[test]
    fn concurrent_config_writes_wait_for_each_other_instead_of_failing() {
        let d = tempfile::tempdir().unwrap();
        assert!(ok(d.path(), &["init", "-q"]));
        let handles: Vec<_> = (0..12)
            .map(|i| {
                let dir = d.path().to_path_buf();
                std::thread::spawn(move || {
                    ok(&dir, &["config", &format!("tryout.change-i{i}"), "1"])
                })
            })
            .collect();
        for h in handles {
            assert!(h.join().unwrap(), "a config write failed on git's lock");
        }
        assert_eq!(
            lines(d.path(), &["config", "--get-regexp", "^tryout"]).len(),
            12
        );
    }
}
