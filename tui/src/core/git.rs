//! The git CLI, which stays a CLI: `worktree.useRelativePaths` and
//! `worktree repair` need git ≥ 2.48, which no library implements.

use std::path::Path;
use std::process::{Command, Stdio};

/// stdout of `git -C <dir> <args>` without its trailing newlines, or None when
/// git fails. stderr is discarded, as the bash `2>/dev/null` did.
pub fn out(dir: &Path, args: &[&str]) -> Option<String> {
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
}
