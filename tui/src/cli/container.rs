//! `tryout ctr <verb>`: the far end of `ddev tryout`, inside the web container.
//! Arguments arrive resolved — nothing in here prompts; a missing one is an
//! error with the usage line. Nothing in here runs `ddev`.

use crate::core::ctx::Ctx;
use crate::core::out::{self, BOLD, CYAN, DIM, GREEN, NC, TEXT, YELLOW};
use crate::core::{php, status, worktree};

use super::host::print;
use super::verbs::{self, Verb};
use super::{Exit, Res, require_core};

pub fn run(ctx: &Ctx, args: &[String]) -> Res {
    let (action, rest) = match args.split_first() {
        Some((a, r)) => (a.as_str(), r),
        None => ("", &[][..]),
    };
    let verb = verbs::find(action).filter(|s| !s.host_only).map(|s| s.verb);
    match verb {
        Some(Verb::Status) => {
            let lines = status::body(ctx, &std::env::var("TRYOUT_PATCHES").unwrap_or_default());
            print(&format!(
                "\n{}\n",
                status::boxed("TYPO3 tryout — Status", &lines)
            ));
            Ok(())
        }
        Some(Verb::Worktree) => worktree(ctx, rest),
        Some(Verb::Composer) => composer(ctx),
        Some(_) => {
            out::error(format!("'{action}' is not in this build yet"));
            Err(Exit(70))
        }
        None => {
            out::error(format!("tryout-container: unknown verb '{action}'"));
            Err(Exit(64))
        }
    }
}

/// Regenerate the primary overlay's require block from the sysexts of the Core
/// it serves — `active_core_dir`, which `worktree use` moves.
pub fn composer(ctx: &Ctx) -> Res {
    require_core(ctx)?;
    out::info("Syncing composer.tryout.json with available system extensions...");
    match crate::core::composer::sync(&ctx.instance_dir(), &ctx.active_core_dir()) {
        Ok(msg) => {
            print(&format!("{msg}\n"));
            Ok(())
        }
        Err(e) => {
            eprintln!("{e}");
            Err(Exit(1))
        }
    }
}

fn worktree(ctx: &Ctx, args: &[String]) -> Res {
    let (sub, rest) = match args.split_first() {
        Some((s, r)) => (s.as_str(), r),
        None => ("list", &[][..]),
    };
    require_core(ctx)?;
    match sub {
        "list" => list(ctx, rest.first().map(String::as_str).unwrap_or("")),
        "branches" => {
            // Read-only and never prompts: the TUI's branch picker reads it.
            worktree::ensure_branch_refs_quiet(ctx);
            let branches = ctx.local_core_branches();
            if rest.first().map(String::as_str) == Some("--json") {
                print(&worktree::branches_json(&branches));
            } else {
                print(
                    &branches
                        .iter()
                        .map(|b| format!("{b}\n"))
                        .collect::<String>(),
                );
            }
            Ok(())
        }
        _ => {
            out::error(format!("Unknown worktree command: {sub}"));
            Err(Exit(1))
        }
    }
}

fn list(ctx: &Ctx, flag: &str) -> Res {
    let mut s = String::new();
    match flag {
        // The TUI's contract: the JSON and nothing else on stdout.
        "--json" => {
            print(&worktree::infos_json(&worktree::infos(
                ctx,
                &php::available_versions(),
            )));
            return Ok(());
        }
        // The machine-readable contract other tools parse (tests/e2e).
        "--plain" => {
            s.push_str(&format!("\n{BOLD}Core worktrees{NC}\n"));
            s.push_str(&plain_row([
                "NAME", "HEAD", "BRANCH", "STATE", "PHP", "DB", "URL",
            ]));
            for r in worktree::rows(ctx) {
                let dirty = if worktree::is_dirty(&ctx.core_checkout_dir(&r.name)) {
                    "dirty"
                } else {
                    "clean"
                };
                // "-" for a checkout that serves nothing; a served one says
                // what it has, even an empty value.
                let serves = crate::core::site::is_served(ctx, &r.name) || r.active;
                let (url, php, db) = if serves {
                    worktree::site_info(ctx, &r.name, r.active)
                } else {
                    (String::new(), "-".into(), "-".into())
                };
                let marker = if r.active { " ← primary" } else { "" };
                s.push_str(&plain_row([
                    &r.name,
                    &r.head,
                    &r.branch,
                    dirty,
                    &php,
                    &db,
                    &format!("{url}{marker}"),
                ]));
            }
        }
        "" => {
            let rows = worktree::rows(ctx);
            s.push_str(&format!(
                "\n{BOLD}{TEXT}Core worktrees{NC} {DIM}{TEXT}· {}{NC}\n",
                rows.len()
            ));
            for r in &rows {
                s.push('\n');
                s.push_str(&card(ctx, r));
            }
        }
        other => {
            out::error(format!("Unknown option: {other}"));
            out::error("  → ddev tryout worktree list [--plain|--json]");
            return Err(Exit(1));
        }
    }
    s.push_str(&format!(
        "\n  {DIM}served sites have their own URL, PHP and database;{NC}\n  {DIM}the primary is whichever worktree 'use' points at{NC}\n\n"
    ));
    print(&s);
    Ok(())
}

/// `  %-12s %-12s %-12s %-6s %-5s %-10s %s`
fn plain_row(c: [&str; 7]) -> String {
    format!(
        "  {:<12} {:<12} {:<12} {:<6} {:<5} {:<10} {}\n",
        c[0], c[1], c[2], c[3], c[4], c[5], c[6]
    )
}

/// One `worktree list` card: where the checkout stands, what is on it, what it
/// serves.
fn card(ctx: &Ctx, r: &worktree::Row) -> String {
    let dir = ctx.core_checkout_dir(&r.name);
    let mut s = if r.active {
        format!(
            "{CYAN}●{NC} {BOLD}{TEXT}{}{NC}  {CYAN}← primary{NC}\n",
            r.name
        )
    } else {
        format!("{TEXT}○{NC} {BOLD}{TEXT}{}{NC}\n", r.name)
    };
    let (base, count) = worktree::base_info(&dir, &r.branch);
    let branch = if r.branch == "(detached)" {
        format!("detached from {base}")
    } else {
        r.branch.clone()
    };
    let age = crate::core::git::out(&dir, &["log", "-1", "--format=%cr"]).unwrap_or_default();
    let age = if age.is_empty() {
        String::new()
    } else {
        format!(" {DIM}{TEXT}· {age}")
    };
    s.push_str(&format!(
        "  {TEXT}{branch} {DIM}{TEXT}@{NC} {TEXT}{}{age}{NC}\n",
        r.head
    ));

    let mut subject =
        crate::core::git::out(&dir, &["log", "-1", "--format=%s"]).unwrap_or_default();
    if subject.chars().count() > 70 {
        subject = format!("{}…", subject.chars().take(69).collect::<String>());
    }
    if count > 0 {
        let noun = if count == 1 { "patch" } else { "patches" };
        s.push_str(&format!(
            "  {YELLOW}{count} {noun} on top{NC} {DIM}{TEXT}·{NC} {TEXT}{subject}{NC}\n"
        ));
    } else if !subject.is_empty() {
        s.push_str(&format!("  {DIM}{TEXT}{subject}{NC}\n"));
    }

    let changes = worktree::change_summary(&dir);
    let changes = if changes == "clean" {
        format!("{GREEN}clean{NC}")
    } else {
        format!("{YELLOW}{changes}{NC}")
    };
    let (url, php, db) = worktree::site_info(ctx, &r.name, r.active);
    if !url.is_empty() || !php.is_empty() {
        s.push_str(&format!("  {changes}\n"));
        if !url.is_empty() {
            s.push_str(&format!("  {CYAN}{url}{NC}\n"));
        }
        let php = if php.is_empty() { "-".to_string() } else { php };
        s.push_str(&format!("  {DIM}{TEXT}PHP {php} · {db}{NC}\n"));
    } else {
        s.push_str(&format!("  {changes} {DIM}{TEXT}· not served{NC}\n"));
        s.push_str(&format!(
            "    {DIM}{TEXT}→ ddev tryout worktree serve {}{NC}\n",
            r.name
        ));
    }
    s
}
