//! Which tabs are running a coding agent, and what state it is in. Pure: the
//! event loop feeds it `ps` output and pane timings, so all of it runs under test.

use std::collections::HashMap;
use std::time::Duration;

/// Agents by the name their executable goes by.
const AGENTS: [&str; 6] = ["claude", "codex", "gemini", "aider", "opencode", "amp"];

/// Output this recent means the agent is busy: its spinner redraws continuously.
const WORKING_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// It wants you: it rang the bell, or finished while you were elsewhere.
    Waiting,
    Working,
    Idle,
}

/// The agent a command line is, if it is one. The executable's own name
/// decides; under a runtime (an npm install runs as `node …/claude-code/cli.js`)
/// the package path does. A shell is never an agent.
pub fn classify(args: &str) -> Option<&'static str> {
    let mut words = args.split_whitespace();
    let exe = words.next()?.rsplit('/').next()?;
    if let Some(agent) = AGENTS.iter().find(|a| **a == exe) {
        return Some(agent);
    }
    if matches!(exe, "node" | "bun" | "deno") {
        let script = words.next()?;
        if script.contains("claude-code") {
            return Some("claude");
        }
        return AGENTS
            .iter()
            .find(|a| script.contains(&format!("/{a}")))
            .copied();
    }
    None
}

/// Where an agent stands, from how its terminal behaved.
///
/// `visible`: its tab is the one on screen — whatever happens there, you are
/// already looking at it, so it never waits for you.
pub fn status(since_output: Duration, bell: bool, unseen_output: bool, visible: bool) -> Status {
    if bell && !visible {
        Status::Waiting
    } else if since_output < WORKING_WINDOW {
        Status::Working
    } else if unseen_output && !visible {
        // It worked while you were elsewhere and has gone quiet: done, or asking.
        Status::Waiting
    } else {
        Status::Idle
    }
}

/// `ps -o pid=,args=` output, by pid.
pub fn parse_ps(output: &str) -> HashMap<u32, String> {
    output
        .lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let (pid, args) = l.split_once(char::is_whitespace)?;
            Some((pid.parse().ok()?, args.trim().to_string()))
        })
        .collect()
}

/// Ask `ps` what each pid is running. One process for all of them; the same
/// flags work on macOS and Linux. Blocking, so callers keep it off the UI thread.
pub fn lookup(pids: &[u32]) -> HashMap<u32, String> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    std::process::Command::new("ps")
        .args(["-o", "pid=,args=", "-p", &list])
        .output()
        .map(|o| parse_ps(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_are_known_by_their_executable() {
        assert_eq!(classify("claude"), Some("claude"));
        assert_eq!(classify("/usr/local/bin/claude --resume"), Some("claude"));
        assert_eq!(classify("codex exec fix"), Some("codex"));
        assert_eq!(classify("/opt/homebrew/bin/aider --model x"), Some("aider"));
    }

    #[test]
    fn an_npm_install_is_known_by_its_package() {
        assert_eq!(
            classify("node /opt/homebrew/lib/node_modules/@anthropic-ai/claude-code/cli.js"),
            Some("claude")
        );
        assert_eq!(
            classify("node /usr/lib/node_modules/@openai/codex/bin/codex.js"),
            Some("codex")
        );
    }

    #[test]
    fn shells_and_other_programs_are_not_agents() {
        for args in [
            "-zsh",
            "zsh",
            "/bin/bash",
            "vim README.md",
            "node server.js",
            "",
        ] {
            assert_eq!(classify(args), None, "{args:?}");
        }
        assert_eq!(classify("claudette"), None, "a prefix is not a match");
    }

    #[test]
    fn status_follows_output_bell_and_whether_you_are_looking() {
        let quiet = Duration::from_secs(30);
        let busy = Duration::from_millis(300);
        assert_eq!(status(busy, false, true, false), Status::Working);
        assert_eq!(status(quiet, true, false, false), Status::Waiting, "bell");
        assert_eq!(
            status(busy, true, true, false),
            Status::Waiting,
            "bell beats busy"
        );
        assert_eq!(
            status(quiet, false, true, false),
            Status::Waiting,
            "went quiet unseen"
        );
        assert_eq!(status(quiet, false, false, false), Status::Idle);
        assert_eq!(
            status(quiet, true, true, true),
            Status::Idle,
            "you are looking at it"
        );
        assert_eq!(status(busy, false, true, true), Status::Working);
    }

    #[test]
    fn ps_output_is_read_by_pid() {
        let m = parse_ps("  4242 /usr/local/bin/claude --resume\n   17 -zsh\ngarbage\n");
        assert_eq!(
            m.get(&4242).map(String::as_str),
            Some("/usr/local/bin/claude --resume")
        );
        assert_eq!(m.get(&17).map(String::as_str), Some("-zsh"));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn a_real_process_is_looked_up() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        let m = lookup(&[child.id()]);
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            m.get(&child.id()).is_some_and(|a| a.contains("sleep")),
            "{m:?}"
        );
    }
}
