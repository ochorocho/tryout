//! Applying Gerrit changes: a change number resolves to its latest patchset
//! ref, which is fetched and cherry-picked. Merged and abandoned changes are
//! skipped, an already applied one is recognised by its Change-Id, and a
//! conflict aborts the cherry-pick.

use std::path::Path;

use serde_json::Value;

use super::ctx::{GERRIT_REMOTE, GERRIT_URL};
use super::out::{self, BOLD, CYAN, GREEN, NC, RED, YELLOW};
use super::{gerrit, git, proc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Applied,
    AlreadyApplied,
    Merged,
    Abandoned,
    Conflict,
    Error,
}

impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Outcome::Applied => "applied",
            Outcome::AlreadyApplied => "already_applied",
            Outcome::Merged => "merged",
            Outcome::Abandoned => "abandoned",
            Outcome::Conflict => "conflict",
            Outcome::Error => "error",
        }
    }

    /// Failures stop a list; skips do not.
    pub fn failed(self) -> bool {
        matches!(self, Outcome::Conflict | Outcome::Error)
    }
}

pub struct Resolved {
    pub subject: String,
    pub reference: String,
    pub number: String,
    pub status: String,
}

pub fn ensure_gerrit_remote(core: &Path) {
    if !git::ok(core, &["remote", "get-url", "gerrit"]) {
        out::info("Adding Gerrit remote...");
        proc::git(core, &["remote", "add", "gerrit", GERRIT_REMOTE]);
    }
}

/// A change number's current patchset.
pub fn resolve(change: &str) -> Result<Resolved, gerrit::Error> {
    let v = gerrit::get(&format!("/changes/{change}?o=CURRENT_REVISION"))?;
    parse_resolved(&v)
}

pub fn parse_resolved(v: &Value) -> Result<Resolved, gerrit::Error> {
    let rev = v
        .get("current_revision")
        .and_then(Value::as_str)
        .ok_or(gerrit::Error::Parse)?;
    let r = v
        .get("revisions")
        .and_then(|x| x.get(rev))
        .ok_or(gerrit::Error::Parse)?;
    let subject = v
        .get("subject")
        .and_then(Value::as_str)
        .unwrap_or("No subject")
        .replace('\n', " ");
    // jq's ltrimstr/rtrimstr: one space off each end, not all of them.
    let subject = subject.strip_prefix(' ').unwrap_or(&subject);
    let subject = subject.strip_suffix(' ').unwrap_or(subject);
    Ok(Resolved {
        subject: subject.to_string(),
        reference: r
            .get("ref")
            .and_then(Value::as_str)
            .ok_or(gerrit::Error::Parse)?
            .to_string(),
        number: r
            .get("_number")
            .map(|n| n.to_string())
            .ok_or(gerrit::Error::Parse)?,
        status: v
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN")
            .to_string(),
    })
}

/// Apply one change to the checkout at `core`, based on `branch`. Returns the
/// outcome and the subject (for the summary).
pub fn apply(core: &Path, branch: &str, change: &str) -> (Outcome, String) {
    ensure_gerrit_remote(core);
    out::info(format!("Resolving change {change}..."));
    let r = match resolve(change) {
        Ok(r) => r,
        Err(e) => {
            out::error(match e {
                gerrit::Error::Fetch => {
                    format!("Failed to fetch change {change} from Gerrit (HTTP error)")
                }
                gerrit::Error::Parse => {
                    format!("Failed to parse Gerrit response for change {change}")
                }
            });
            out::error(format!("  → Verify: {GERRIT_URL}{change}"));
            return (Outcome::Error, String::new());
        }
    };
    out::print_line(&format!("  {BOLD}Subject:{NC}  {}", r.subject));
    out::print_line(&format!("  {BOLD}Patchset:{NC} {}", r.number));
    out::print_line(&format!("  {BOLD}Status:{NC}   {}", r.status));
    if r.status == "MERGED" {
        out::info(format!("Change {change} is already merged — skipping"));
        return (Outcome::Merged, r.subject);
    }
    if r.status == "ABANDONED" {
        out::warn(format!("Change {change} is abandoned — skipping"));
        return (Outcome::Abandoned, r.subject);
    }
    out::info("Fetching from Gerrit...");
    if !proc::git(core, &["fetch", "gerrit", &r.reference]) {
        out::error(format!("Failed to fetch ref {} from Gerrit", r.reference));
        out::error(format!("  → Verify: {GERRIT_URL}{change}"));
        return (Outcome::Error, r.subject);
    }
    let body = git::out(core, &["log", "-1", "--format=%b", "FETCH_HEAD"]).unwrap_or_default();
    let change_id = body
        .lines()
        .find_map(|l| l.strip_prefix("Change-Id:"))
        .and_then(|s| s.split_whitespace().next());
    // Which change a Change-Id is, for the listing: a cherry-picked patchset
    // carries only its Change-Id, never its number.
    if let Some(id) = change_id {
        remember(core, id, change);
    }
    if let Some(id) = change_id {
        let applied = git::out(
            core,
            &["log", "--format=%b", &format!("origin/{branch}..HEAD")],
        )
        .unwrap_or_default();
        if applied.lines().any(|l| l == format!("Change-Id: {id}")) {
            out::info(format!("Change {change} is already applied — skipping"));
            return (Outcome::AlreadyApplied, r.subject);
        }
    }
    out::info(format!("Cherry-picking change {change}..."));
    let picked = std::process::Command::new("git")
        .arg("-C")
        .arg(core)
        .args(["cherry-pick", "FETCH_HEAD"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if picked {
        out::success(format!("Applied change {change}: {}", r.subject));
        (Outcome::Applied, r.subject)
    } else {
        git::ok(core, &["cherry-pick", "--abort"]);
        out::error(format!(
            "Cherry-pick failed for change {change} (merge conflict)"
        ));
        out::error("  Cherry-pick has been aborted automatically.");
        out::error(format!("  → Verify: {GERRIT_URL}{change}"));
        (Outcome::Conflict, r.subject)
    }
}

/// Note a Change-Id's change number in the shared git config (every worktree
/// reads the same one), so a listing can name the patches on top offline.
pub fn remember(core: &Path, change_id: &str, number: &str) {
    if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) {
        git::ok(
            core,
            &["config", &format!("tryout.change-{change_id}"), number],
        );
    }
}

/// Every Change-Id → change number noted so far.
pub fn remembered(core: &Path) -> std::collections::HashMap<String, u64> {
    git::lines(core, &["config", "--get-regexp", r"^tryout\.change-"])
        .iter()
        .filter_map(|l| {
            let (k, v) = l.split_once(' ')?;
            // git lowercases variable names; Change-Ids are compared lowercased.
            Some((
                k.strip_prefix("tryout.change-")?.to_lowercase(),
                v.trim().parse().ok()?,
            ))
        })
        .collect()
}

/// Apply TRYOUT_PATCHES in order, stopping at the first failure. Returns how
/// many were cherry-picked, or Err after a failure.
pub fn apply_all(core: &Path, branch: &str, list: &str) -> Result<usize, usize> {
    let patches: String = list.chars().filter(|c| !c.is_whitespace()).collect();
    if patches.is_empty() {
        out::info("No patches configured.");
        return Ok(0);
    }
    out::info(format!("Applying patches: {patches}"));
    out::print_line("");
    let (mut applied, mut skipped, mut failed) = (0, 0, 0);
    let mut summary = Vec::new();
    for id in patches.split(',').filter(|s| !s.is_empty()) {
        let (outcome, subject) = apply(core, branch, id);
        let subject = if subject.is_empty() {
            "unknown".to_string()
        } else {
            subject
        };
        summary.push((id.to_string(), subject, outcome));
        if outcome.failed() {
            failed += 1;
            out::warn("Stopping — remaining patches skipped due to failure.");
            break;
        }
        if outcome == Outcome::Applied {
            applied += 1;
        } else {
            skipped += 1;
        }
        out::print_line("");
    }
    print_summary(&summary);
    if failed > 0 {
        out::error(format!(
            "{failed} patch(es) failed. {applied} applied, {skipped} skipped."
        ));
        out::error("  → Reset and retry: ddev tryout reset");
        return Err(applied);
    }
    if applied == 0 {
        out::info(format!(
            "All {skipped} patch(es) already applied, merged or abandoned."
        ));
    } else {
        out::success(format!("{applied} patch(es) applied, {skipped} skipped."));
    }
    Ok(applied)
}

fn print_summary(rows: &[(String, String, Outcome)]) {
    if rows.is_empty() {
        return;
    }
    let mut s = format!("\n{BOLD}Patch Summary{NC}\n");
    s.push_str(&format!(
        "{:<10} {:<37} {}\n",
        "Change", "Subject", "Result"
    ));
    s.push_str(&format!(
        "{} {} {}\n",
        "─".repeat(10),
        "─".repeat(37),
        "─".repeat(10)
    ));
    for (id, subject, outcome) in rows {
        let subject = if subject.chars().count() > 35 {
            format!("{}...", subject.chars().take(32).collect::<String>())
        } else {
            subject.clone()
        };
        let colour = match outcome {
            Outcome::Applied => GREEN,
            Outcome::Merged | Outcome::AlreadyApplied => CYAN,
            Outcome::Abandoned => YELLOW,
            _ => RED,
        };
        s.push_str(&format!(
            "{id:<10} {} {colour}{}{NC}\n",
            out::pad_display_bytes(&subject, 37),
            outcome.name()
        ));
    }
    s.push('\n');
    out::print(&s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_resolves_to_its_current_patchset() {
        let v = gerrit::parse(
            ")]}'\n{\"subject\":\"  Two  spaces \",\"status\":\"NEW\",\"current_revision\":\"b\",\
             \"revisions\":{\"a\":{\"ref\":\"refs/changes/1\",\"_number\":1},\"b\":{\"ref\":\"refs/changes/2\",\"_number\":2}}}",
        )
        .unwrap();
        let r = parse_resolved(&v).unwrap();
        assert_eq!(
            (r.reference.as_str(), r.number.as_str(), r.status.as_str()),
            ("refs/changes/2", "2", "NEW")
        );
        // One space off each end, as jq's ltrimstr/rtrimstr did.
        assert_eq!(r.subject, " Two  spaces");
        assert!(parse_resolved(&serde_json::json!({"subject": "x"})).is_err());
    }
}
