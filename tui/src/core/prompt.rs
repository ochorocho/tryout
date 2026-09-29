//! Asking for a missing argument. A command run without one asks when someone
//! can answer, and otherwise falls through to `explain_missing`. Every answer
//! comes back as a value; None means cancelled or nobody to ask.
//!
//! The prompts are drawn on stderr (inquire's screen): DDEV pipes a host
//! command's stdout, always, so stdin and stderr are what must be terminals.
//! Without them a piped answer is read as a plain line, as before.

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

/// Pick one of `items`: a list to move through, ESC to cancel. With no
/// terminal a piped answer still works.
pub fn choose(prompt: &str, items: &[String]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    if have_tty() {
        return inquire::Select::new(prompt, items.to_vec())
            .with_page_size(15)
            .prompt()
            .ok();
    }
    if stderr_tty() {
        let mut e = std::io::stderr();
        let _ = writeln!(e, "  {}", items.join(" "));
        let _ = write!(e, "  {prompt}: ");
    }
    let answer = clean_answer(&read_line()?);
    (!answer.is_empty()).then_some(answer)
}

/// Pick several of `items`. None on a cancel or an empty pick.
pub fn choose_multi(prompt: &str, items: &[String]) -> Option<Vec<String>> {
    if items.is_empty() {
        return None;
    }
    if have_tty() {
        let picked = inquire::MultiSelect::new(prompt, items.to_vec())
            .with_page_size(15)
            .prompt()
            .ok()?;
        return (!picked.is_empty()).then_some(picked);
    }
    // One per line until EOF or a blank.
    if stderr_tty() {
        let mut e = std::io::stderr();
        for i in items {
            let _ = writeln!(e, "  {i}");
        }
        let _ = write!(e, "  {prompt} (one per line, blank to finish): ");
    }
    let mut got = Vec::new();
    while let Some(line) = read_line() {
        let answer: String = line.chars().filter(|c| (*c as u32) >= 0x20).collect();
        if answer.is_empty() {
            break;
        }
        got.push(answer);
    }
    (!got.is_empty()).then_some(got)
}

/// Free text; None when empty or cancelled.
pub fn input(prompt: &str, placeholder: &str) -> Option<String> {
    if have_tty() {
        let v = inquire::Text::new(prompt)
            .with_placeholder(placeholder)
            .prompt()
            .ok()?;
        return (!v.is_empty()).then_some(v);
    }
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

/// Free text for a missing argument.
pub fn ask_text(prompt: &str, placeholder: &str) -> Option<String> {
    input(prompt, placeholder)
}

/// Pick a branch from the local refs, in picker order (main, releases newest
/// first, legacy last). The list is fetched first when the clone carried one.
pub fn ask_branch(ctx: &Ctx, prompt: &str) -> Option<String> {
    worktree::ensure_branch_refs(ctx);
    choose(
        prompt,
        &super::vsort::picker_order(&ctx.local_core_branches()),
    )
}

/// The branch a NEW worktree is based on. Three outcomes, and the difference
/// matters: a branch already given is kept (not a question); with no terminal it
/// is the branch in play, silently — `ddev start` and scripts must not block;
/// a cancel is None, and the caller stops.
pub fn ask_new_worktree_branch(ctx: &Ctx, given: &str, usage: &str) -> Option<String> {
    if !given.is_empty() {
        return Some(given.into());
    }
    if !have_tty() {
        return Some(ctx.branch().into());
    }
    let picked = ask_branch(ctx, "Based on which branch?").filter(|b| !b.is_empty());
    if picked.is_none() {
        explain_missing(usage);
    }
    picked
}

/// Pick several open changes; answers with their numbers.
pub fn pick_patches(prompt: &str, changes: &[super::gerrit::Change]) -> Vec<u64> {
    let labels: Vec<String> = changes
        .iter()
        .map(|c| {
            format!(
                "{:<7} {} {} {}",
                c.number,
                pad_display(&c.subject, 68),
                pad_display(&c.owner, 18),
                c.scores
            )
        })
        .collect();
    choose_multi(prompt, &labels)
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.split_whitespace().next()?.parse().ok())
        .collect()
}

/// "<number> - <subject>" for each picked change, or the bare number when it is
/// not in the list (a hand-typed one).
pub fn describe_patches(picked: &[u64], changes: &[super::gerrit::Change]) -> Vec<String> {
    picked
        .iter()
        .map(|n| match changes.iter().find(|c| c.number == *n) {
            Some(c) if !c.subject.is_empty() => format!("{n} - {}", c.subject),
            _ => n.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod ask_tests {
    use super::*;
    use crate::core::ctx::DdevEnv;

    #[test]
    fn an_explicitly_given_branch_is_not_asked_about() {
        let ctx = Ctx::new("/nonexistent", DdevEnv::default());
        assert_eq!(
            ask_new_worktree_branch(&ctx, "13.4", "usage").as_deref(),
            Some("13.4")
        );
    }

    #[test]
    fn picked_patches_come_back_as_numbers_described_by_their_subjects() {
        let c = |n, s: &str| super::super::gerrit::Change {
            number: n,
            subject: s.into(),
            owner: "Ada".into(),
            scores: String::new(),
        };
        let changes = [c(91234, "[BUGFIX] One"), c(91000, "[TASK] Two")];
        assert_eq!(
            describe_patches(&[91000, 5], &changes),
            ["91000 - [TASK] Two".to_string(), "5".to_string()]
        );
    }
}
