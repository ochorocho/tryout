//! The host side of `ddev tryout`: resolve arguments (asking where someone can
//! answer), then either do the host-only work or delegate to the container.

use std::io::Write;

use crate::core::ctx::Ctx;
use crate::core::out::{self, DIM, NC, RED, YELLOW};
use crate::core::prompt::{self, explain_missing};
use crate::core::{ddev, gerrit, site, status, worktree};

use super::verbs::{self, Verb};
use super::{Exit, Res, help, reject_args, require_core};

pub fn run(ctx: &Ctx, args: &[String]) -> Res {
    let (action, rest) = match args.split_first() {
        Some((a, r)) => (a.as_str(), r),
        None => ("help", &[][..]),
    };
    let Some(spec) = verbs::find(action) else {
        out::error(format!("Unknown command: {action}"));
        print(&help::main());
        return Err(Exit(1));
    };
    match spec.verb {
        Verb::Status => status(ctx, rest),
        Verb::Help => {
            reject_args("help", rest)?;
            print(&help::main());
            Ok(())
        }
        Verb::Launch => launch(ctx, rest),
        Verb::Patch => patch(ctx, rest),
        Verb::Worktree => worktree(ctx, rest),
        Verb::Cs => cs(ctx, rest),
        Verb::Ui => ui(ctx, rest),
        Verb::Download
        | Verb::Checkout
        | Verb::Composer
        | Verb::Exec
        | Verb::Reset
        | Verb::Delete => not_ported(action),
    }
}

/// A verb the Rust side does not do yet; the bash implementation still does.
fn not_ported(what: &str) -> Res {
    out::error(format!("'{what}' is not in this build yet"));
    out::error("  → run it without TRYOUT_BIN");
    Err(Exit(70))
}

pub fn print(s: &str) {
    let mut o = std::io::stdout().lock();
    let _ = o.write_all(s.as_bytes());
    let _ = o.flush();
}

fn delegate(ctx: &Ctx, args: &[&str]) -> Res {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    // The verbs that change nothing skip the Mutagen flush.
    let quiet = matches!(
        (
            args.first().map(String::as_str),
            args.get(1).map(String::as_str)
        ),
        (Some("status" | "exec" | "composer"), _)
            | (Some("worktree"), Some("list"))
            | (Some("cs"), Some("doctor"))
    );
    match ddev::delegate(ctx, &args, !quiet) {
        0 => Ok(()),
        c => Err(Exit(c)),
    }
}

// ─── status ─────────────────────────────────────────────────────────────────

fn status(ctx: &Ctx, args: &[String]) -> Res {
    reject_args("status", args)?;
    // On the host, before anything: an older install is why a command or its
    // completion can look broken, and this must work with the containers down.
    if status::addon_is_stale(ctx, crate::core::PAYLOAD_VERSION) {
        print(&format!(
            "\n  {YELLOW}!{NC} This project runs an older copy of the tryout add-on\n\
\x20   {DIM}the command and its tab-completion offer the previous feature set{NC}\n\
\x20   {DIM}→ ddev add-on get <path-to-tryout> && ddev restart{NC}\n"
        ));
    }
    if !ctx.has_core() {
        let lines = vec![
            format!("  Core:      {RED}✗{NC} not cloned"),
            format!("             {DIM}→ ddev tryout download{NC}"),
        ];
        print(&format!(
            "\n{}\n",
            status::boxed("TYPO3 tryout — Status", &lines)
        ));
        return Ok(());
    }
    delegate(ctx, &["status"])
}

// ─── launch ─────────────────────────────────────────────────────────────────

fn launch(ctx: &Ctx, args: &[String]) -> Res {
    let mut target = String::new();
    let mut backend = false;
    for a in args {
        match a.as_str() {
            "--backend" | "-b" => backend = true,
            "-h" | "--help" => {
                print(&help::launch());
                return Ok(());
            }
            a if a.starts_with('-') => {
                out::error(format!("Unknown option: {a}"));
                print(&help::launch());
                return Err(Exit(1));
            }
            a => target = a.to_string(),
        }
    }

    // Standing in a served worktree answers the question.
    if target.is_empty()
        && let Some(here) = std::env::var_os("PWD")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .and_then(|p| worktree::name_for_path(ctx, &p))
    {
        if here == ctx.active_worktree_name() || site::is_served(ctx, &here) {
            target = here;
        } else {
            out::error(format!("Worktree '{here}' is not served — it has no URL"));
            out::error(format!("  → ddev tryout worktree serve {here}"));
            out::error(format!(
                "  → or: ddev tryout worktree use {here}   (serve it as the primary)"
            ));
            return Err(Exit(1));
        }
    }
    if target.is_empty() {
        target = prompt::ask_site(ctx, "Which site to open?", &[]).ok_or_else(|| {
            explain_missing("ddev tryout launch [<worktree>] [--backend]");
            Exit(1)
        })?;
    }

    let target = site::for_name(ctx, &target);
    if !site::is_served(ctx, &target) {
        out::error(format!("No served site '{target}'"));
        out::error("  → ddev tryout worktree list");
        return Err(Exit(1));
    }
    let mut url = if site::is_primary(&target) && !ctx.env.primary_url.is_empty() {
        ctx.env.primary_url.clone()
    } else {
        format!("https://{}", site::hostname(ctx, &target))
    };
    if backend {
        url = format!("{}/typo3/", url.trim_end_matches('/'));
    }
    ddev::open_url(ctx, &url);
    Ok(())
}

// ─── patch ──────────────────────────────────────────────────────────────────

/// The branch whose open changes a site should be offered: the project's for
/// the primary, the site's own base otherwise; "-" is every branch.
fn patch_branch_for(ctx: &Ctx, target: &str, all: bool) -> String {
    if all {
        "-".into()
    } else if !target.is_empty() && !site::is_primary(target) {
        worktree::detect_detached_base_branch(&site::core_dir(ctx, target))
    } else {
        ctx.branch().to_string()
    }
}

fn patch(ctx: &Ctx, args: &[String]) -> Res {
    require_core(ctx)?;
    let (mut target, mut all, mut list, mut json) = (String::new(), false, false, false);
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--all-branches" => all = true,
            "--list" => list = true,
            "--json" => json = true,
            "--site" => target = it.next().cloned().unwrap_or_default(),
            a if a.starts_with("--site=") => target = a["--site=".len()..].to_string(),
            _ => rest.push(a.clone()),
        }
    }
    if list {
        if target.is_empty() {
            target = rest.first().cloned().unwrap_or_default();
        }
        let target = site::for_name(ctx, &target);
        let changes = gerrit::list_open(&patch_branch_for(ctx, &target, all), 50)
            .ok()
            .filter(|c| !c.is_empty())
            .ok_or_else(|| {
                out::error("Could not reach Gerrit, or no open changes");
                Exit(1)
            })?;
        if json {
            print(&gerrit::changes_json(&changes));
        } else {
            let rows: Vec<String> = changes.iter().map(gerrit::Change::tsv).collect();
            print(&format!("{}\n", rows.join("\n")));
        }
        return Ok(());
    }
    not_ported("patch")
}

// ─── worktree ───────────────────────────────────────────────────────────────

fn worktree(ctx: &Ctx, args: &[String]) -> Res {
    let (sub, rest) = match args.split_first() {
        Some((s, r)) => (s.as_str(), r),
        None => ("list", &[][..]),
    };
    match sub {
        "list" | "branches" => {
            require_core(ctx)?;
            let mut a = vec!["worktree", sub];
            a.extend(rest.iter().map(String::as_str));
            delegate(ctx, &a)
        }
        "help" | "-h" | "--help" => {
            print(&help::worktree());
            Ok(())
        }
        "add" | "use" | "remove" | "rm" | "serve" | "unserve" | "rename" => {
            not_ported(&format!("worktree {sub}"))
        }
        _ => {
            out::error(format!("Unknown worktree command: {sub}"));
            print(&help::worktree());
            Err(Exit(1))
        }
    }
}

// ─── cs ─────────────────────────────────────────────────────────────────────

fn cs(_ctx: &Ctx, args: &[String]) -> Res {
    match args.first().map(String::as_str).unwrap_or("setup") {
        "help" | "-h" | "--help" => {
            print(&help::cs());
            Ok(())
        }
        "setup" | "doctor" | "uninstall" => not_ported("cs"),
        sub => {
            out::error(format!("Unknown cs command: {sub}"));
            print(&help::cs());
            Err(Exit(1))
        }
    }
}

// ─── ui ─────────────────────────────────────────────────────────────────────

fn ui(ctx: &Ctx, args: &[String]) -> Res {
    let root = ctx.root.to_string_lossy().into_owned();
    match args.first().map(String::as_str) {
        Some("stop") => {
            reject_args("ui stop", &args[1..])?;
            return crate::tui::run(&["stop".into(), root]).map_err(|e| {
                out::error(format!("{e:#}"));
                Exit(1)
            });
        }
        None => {}
        Some(other) => {
            out::error(format!("Unknown option: {other}"));
            out::error("  → ddev tryout ui        attach (starting the session)");
            out::error("  → ddev tryout ui stop   close the session");
            return Err(Exit(1));
        }
    }
    require_core(ctx)?;
    // A DDEV host command gets pipes, not the terminal, on stdin and stdout —
    // but /dev/tty still reaches it. Draw there, or there is nothing to draw on.
    attach_dev_tty().map_err(|_| {
        out::error("ddev tryout ui needs a terminal");
        out::error("  → run it from an interactive shell");
        Exit(1)
    })?;
    crate::tui::run(&[root]).map_err(|e| {
        out::error(format!("{e:#}"));
        Exit(1)
    })
}

/// Point stdin, stdout and stderr at the controlling terminal.
fn attach_dev_tty() -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?;
    for fd in 0..3 {
        // SAFETY: dup2 onto the standard descriptors of this process, with a
        // descriptor we own and keep open for the duration of the call.
        if unsafe { libc::dup2(tty.as_raw_fd(), fd) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}
