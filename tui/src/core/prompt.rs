//! Asking for a missing argument. A command run without one asks when someone
//! can answer, and otherwise falls through to `explain_missing`. Every answer
//! comes back as a value; None means cancelled or nobody to ask.
//!
//! The terminal UI of these prompts is drawn on stderr: DDEV pipes a host
//! command's stdout, always, so stdin and stderr are what must be terminals.

use std::io::{BufRead, IsTerminal, Write};

use super::ctx::{Ctx, PRIMARY_SITE};
use super::out::{self, pad_display};
use super::{site, worktree};

pub fn have_tty() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

fn stderr_tty() -> bool {
    std::io::stderr().is_terminal()
}

/// "Cancelled" where someone could have answered, the usage line where nobody
/// could.
pub fn explain_missing(usage: &str) {
    if have_tty() {
        out::warn("Cancelled")
    } else {
        out::error(format!("Usage: {usage}"))
    }
}

fn read_line() -> Option<String> {
    let mut s = String::new();
    match std::io::stdin().lock().read_line(&mut s) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(s.trim_end_matches(['\n', '\r']).to_string()),
    }
}

/// Drop escape sequences and control characters from a typed answer: an arrow
/// key arrives as ESC [ B, and a stray letter in one must not read as input.
pub fn clean_answer(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while chars
                    .peek()
                    .is_some_and(|c| c.is_ascii_digit() || *c == ';')
                {
                    chars.next();
                }
            } else {
                while chars.peek().is_some_and(|c| matches!(c, 'N' | 'O' | 'P')) {
                    chars.next();
                }
            }
            if chars.peek().is_some_and(char::is_ascii_alphabetic) {
                chars.next();
            }
            continue;
        }
        if (c as u32) >= 0x20 {
            out.push(c);
        }
    }
    out
}

/// Pick one of `items`. With no terminal a piped answer still works.
pub fn choose(prompt: &str, items: &[String]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    if stderr_tty() {
        let mut e = std::io::stderr();
        let _ = writeln!(e, "  {}", items.join(" "));
        let _ = write!(e, "  {prompt}: ");
    }
    let answer = clean_answer(&read_line()?);
    (!answer.is_empty()).then_some(answer)
}

/// Free text; None when empty.
pub fn input(prompt: &str, _placeholder: &str) -> Option<String> {
    if stderr_tty() {
        let _ = write!(std::io::stderr(), "  {prompt}: ");
    }
    let answer = read_line()?;
    (!answer.is_empty()).then_some(answer)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    Yes,
    No,
    /// Nobody to ask: callers say "pass --yes", not "Aborted."
    NoTty,
}

/// Confirm a destructive action. Defaults to No, so Enter never confirms.
pub fn confirm(prompt: &str) -> Confirm {
    if !have_tty() {
        return Confirm::NoTty;
    }
    let _ = write!(std::io::stderr(), "  {prompt} [y/N] ");
    let Some(answer) = read_line() else {
        return Confirm::No;
    };
    match clean_answer(&answer).to_ascii_lowercase().as_str() {
        "y" | "yes" => Confirm::Yes,
        _ => Confirm::No,
    }
}

/// Pick a site: shows what each one is, answers with its name — `@primary`
/// for the primary. `extra` entries are offered below and come back unchanged.
pub fn ask_site(ctx: &Ctx, prompt: &str, extra: &[&str]) -> Option<String> {
    let mut names = vec![PRIMARY_SITE.to_string()];
    names.extend(site::served_names(ctx));
    let mut labels: Vec<String> = names
        .iter()
        .map(|n| {
            if site::is_primary(n) {
                format!("{}  {}", pad_display("primary", 12), ctx.env.primary_url)
            } else {
                format!(
                    "{}  https://{}  PHP {}",
                    pad_display(n, 12),
                    site::hostname(ctx, n),
                    site::php_version(ctx, n)
                )
            }
        })
        .collect();
    labels.extend(extra.iter().map(|s| s.to_string()));
    let picked = choose(prompt, &labels)?;
    if let Some(i) = labels.iter().position(|l| *l == picked) {
        return Some(names.get(i).cloned().unwrap_or(picked));
    }
    // A typed name, or a label with other spacing: the first word decides.
    let first = picked.split(' ').next().unwrap_or_default().to_string();
    Some(if first == "primary" {
        PRIMARY_SITE.into()
    } else {
        first
    })
}

/// Which worktrees a picker offers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    NonPrimary,
    Served,
    Unserved,
}

/// Pick a worktree: one row each with branch, HEAD and state; answers with the
/// bare name. Runs a dirty check per worktree — fine for a picker opened by hand.
pub fn ask_worktree(ctx: &Ctx, prompt: &str, filter: Filter) -> Option<String> {
    let labels: Vec<String> = worktree::rows(ctx)
        .into_iter()
        .filter(|r| match filter {
            Filter::All => true,
            Filter::NonPrimary => !r.active,
            Filter::Served => site::is_served(ctx, &r.name),
            Filter::Unserved => !site::is_served(ctx, &r.name),
        })
        .map(|r| {
            let dir = ctx.core_checkout_dir(&r.name);
            let dirty = if worktree::is_dirty(&dir) {
                "dirty"
            } else {
                "clean"
            };
            let serves = if r.active {
                "← primary".to_string()
            } else if site::is_served(ctx, &r.name) {
                format!("PHP {}", site::php_version(ctx, &r.name))
            } else {
                String::new()
            };
            format!(
                "{}  {}  {}  {}  {serves}",
                pad_display(&r.name, 14),
                pad_display(&r.branch, 20),
                r.head,
                pad_display(dirty, 5)
            )
        })
        .collect();
    if labels.is_empty() {
        out::error("No worktree to choose from");
        out::error("  → ddev tryout worktree add <name> [<branch>]");
        return None;
    }
    let picked = choose(prompt, &labels)?;
    Some(picked.split(' ').next().unwrap_or_default().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_control_sequence_is_never_an_answer() {
        assert_eq!(clean_answer("\x1b[B"), "");
        assert_eq!(clean_answer("v13\x1b[A"), "v13");
        assert_eq!(clean_answer("\x1bOAy"), "y");
        assert_eq!(clean_answer("a\x01b"), "ab");
    }
}
