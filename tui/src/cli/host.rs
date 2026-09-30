//! The host side of `ddev tryout`: resolve arguments (asking where someone can
//! answer), then either do the host-only work or delegate to the container.

use crate::core::ctx::Ctx;
use crate::core::ctx::PRIMARY_SITE;
use crate::core::db::Db;
use crate::core::kind::Mode;
use crate::core::out::{self, DIM, NC, RED, YELLOW, print};
use crate::core::prompt::{self, explain_missing};
use crate::core::{ddev, gerrit, review, serve, site, status, webserver, worktree};

use super::verbs::{self, Verb};
use super::{Exit, Res, help, reject_args, require_core, require_served};

/// Some verbs only mean something for some kinds of project (`patch` needs a
/// review system, `cs` TYPO3 Core's, `worktree use` the Composer overlay).
fn available(ctx: &Ctx, verb: &str) -> Res {
    if ctx.kind().supports(verb) {
        return Ok(());
    }
    out::error(format!(
        "`{verb}` is not available for {}",
        ctx.kind().label()
    ));
    out::error("  → ddev tryout help   lists what is");
    Err(Exit(1))
}

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
    available(ctx, spec.name)?;
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
        Verb::Composer => composer(ctx, rest),
        Verb::Download => download(ctx, rest),
        Verb::Checkout => checkout(ctx, rest),
        Verb::Exec => exec(ctx, rest),
        Verb::Reset => reset(ctx, rest),
        Verb::Delete => delete(ctx, rest),
    }
}

fn delegate(ctx: &Ctx, args: &[&str]) -> Res {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    // The verbs that change nothing skip the Mutagen flush. `composer` is not
    // one: it rewrites the overlay the host may read next.
    let quiet = matches!(
        (
            args.first().map(String::as_str),
            args.get(1).map(String::as_str)
        ),
        (Some("status" | "exec"), _)
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
    if status::addon_is_stale(ctx, crate::core::payload_version()) {
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

// ─── composer ───────────────────────────────────────────────────────────────

fn composer(ctx: &Ctx, args: &[String]) -> Res {
    if !args.is_empty() {
        let got = args.join(" ");
        out::error(format!("'composer' takes no arguments, but got: {got}"));
        out::error("  It regenerates composer.tryout.json from the Core sysexts.");
        out::error(format!(
            "  → ddev composer {got}   (to run Composer itself)"
        ));
        return Err(Exit(1));
    }
    require_core(ctx)?;
    delegate(ctx, &["composer"])
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
    require_served(ctx, &target)?;
    let mut url = if site::is_primary(&target) && !ctx.env.primary_url.is_empty() {
        ctx.env.primary_url.clone()
    } else {
        format!("https://{}", site::hostname(ctx, &target))
    };
    if backend {
        let Some(path) = ctx.kind().backend_path() else {
            out::error(format!("{} has no backend to open", ctx.kind().label()));
            out::error("  → ddev tryout launch   opens the site");
            return Err(Exit(1));
        };
        url = format!("{}{path}", url.trim_end_matches('/'));
    }
    ddev::open_url(ctx, &url);
    Ok(())
}

// ─── patch ──────────────────────────────────────────────────────────────────

/// The branch whose open changes a site should be offered: the base of the
/// Core it runs on; "-" is every branch.
fn patch_branch_for(ctx: &Ctx, target: &str, all: bool) -> String {
    if all {
        "-".into()
    } else {
        site::core_and_base(ctx, target).1
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

    // `patch <id> <site>` keeps working; --site wins.
    if let Some((id, more)) = rest.split_first() {
        if target.is_empty() {
            target = more.first().cloned().unwrap_or_default();
        }
        let target = site::for_name(ctx, &target);
        let mut a = vec!["patch"];
        if !target.is_empty() {
            a.extend(["--site", &target]);
        }
        a.push(id);
        return delegate(ctx, &a);
    }

    let mut target = site::for_name(ctx, &target);
    // A configured list is a deliberate choice: apply it rather than asking.
    let configured = !crate::core::patch::configured().is_empty();
    if configured || !prompt::have_tty() {
        let mut a = vec!["patch"];
        if !target.is_empty() {
            a.extend(["--site", &target]);
        }
        return delegate(ctx, &a);
    }
    // The site first: it decides which Core is patched, so which branch's
    // open changes are the right list.
    if target.is_empty() && !site::served_names(ctx).is_empty() {
        target = prompt::ask_site(ctx, "Patch which site?", &[]).ok_or_else(|| {
            explain_missing("ddev tryout patch [<change-id>] [<site>]");
            Exit(1)
        })?;
    }
    let branch = patch_branch_for(ctx, &target, all);
    let changes = prompt::spin(
        &format!(
            "Fetching open changes for {}",
            if branch == "-" {
                "every branch"
            } else {
                &branch
            }
        ),
        || gerrit::list_open(&branch, 50),
    )
    .ok()
    .filter(|c| !c.is_empty());
    let Some(changes) = changes else {
        out::error(format!(
            "Could not reach Gerrit, or no open changes on {branch}"
        ));
        out::error("  → ddev tryout patch <change-id>   to apply one by number");
        return Err(Exit(1));
    };
    let picks = prompt::pick_patches("Apply which changes?", &changes);
    if picks.is_empty() {
        explain_missing("ddev tryout patch <change-id> [<site>]");
        return Err(Exit(1));
    }
    // "@primary" is the host's sentinel; the container takes --site as a name.
    if site::is_primary(&target) {
        target.clear();
    }
    let ids: Vec<String> = picks.iter().map(u64::to_string).collect();
    let mut a = vec!["patch"];
    if !target.is_empty() {
        a.extend(["--site", &target]);
    }
    a.extend(ids.iter().map(String::as_str));
    delegate(ctx, &a)?;

    // Applying is per checkout; the patch list is what survives a reset.
    let mut s = String::from("\n  Add to your patch list, so they reapply on every ddev start:\n");
    for l in prompt::describe_patches(&picks, &changes) {
        s.push_str(&format!("    {l}\n"));
    }
    print(&format!("{s}\n"));
    if prompt::confirm("Add them?") == prompt::Confirm::Yes {
        persist_patches(ctx, &ids)?;
    }
    Ok(())
}

/// Append change numbers to TRYOUT_PATCHES in config.tryout-patches.yaml, once
/// each, keeping the rest of the file (comments included) as it is.
fn persist_patches(ctx: &Ctx, ids: &[String]) -> Res {
    let file = ctx.root.join(".ddev/config.tryout-patches.yaml");
    let Ok(text) = std::fs::read_to_string(&file) else {
        out::error("No patch list at .ddev/config.tryout-patches.yaml");
        return Err(Exit(1));
    };
    let key = "TRYOUT_PATCHES=";
    let current: String = text
        .lines()
        .find_map(|l| {
            l.trim_start()
                .strip_prefix('-')
                .map(str::trim_start)
                .and_then(|r| r.strip_prefix(key))
        })
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut list: Vec<String> = current
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    for id in ids {
        if !list.contains(id) {
            list.push(id.clone());
        }
    }
    let joined = list.join(",");
    if joined == current {
        return Ok(());
    }
    let updated: Vec<String> = text
        .lines()
        .map(|l| {
            let indent = &l[..l.len() - l.trim_start().len()];
            match l.trim_start().strip_prefix('-') {
                Some(r) if r.trim_start().starts_with(key) => {
                    let gap = &r[..r.len() - r.trim_start().len()];
                    format!("{indent}-{gap}{key}{joined}")
                }
                _ => l.to_string(),
            }
        })
        .collect();
    let mut body = updated.join("\n");
    if text.ends_with('\n') {
        body.push('\n');
    }
    if std::fs::write(&file, body).is_err() {
        out::error("Could not write .ddev/config.tryout-patches.yaml");
        return Err(Exit(1));
    }
    out::success(format!("Patch list is now: {joined}"));
    Ok(())
}

// ─── download, checkout, reset, delete, exec ────────────────────────────────

fn download(ctx: &Ctx, args: &[String]) -> Res {
    let args: Vec<String> = args
        .iter()
        .map(|a| {
            if a.starts_with('-') {
                a.clone()
            } else {
                site::for_name(ctx, a)
            }
        })
        .collect();
    let mut a = vec!["download"];
    a.extend(args.iter().map(String::as_str));
    delegate(ctx, &a)
}

fn checkout(ctx: &Ctx, args: &[String]) -> Res {
    let mut target = String::new();
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--site" => target = it.next().cloned().unwrap_or_default(),
            a if a.starts_with("--site=") => target = a["--site=".len()..].to_string(),
            _ => rest.push(a.clone()),
        }
    }
    let mut branch = rest.first().cloned().unwrap_or_default();
    if target.is_empty() {
        target = rest.get(1).cloned().unwrap_or_default();
    }
    require_core(ctx)?;
    if branch.is_empty() {
        match prompt::ask_branch(ctx, "Switch Core to which branch?") {
            Some(b) => branch = b,
            None => {
                explain_missing("ddev tryout checkout <branch>");
                if prompt::have_tty() {
                    return Err(Exit(1));
                }
            }
        }
    }
    if branch.is_empty() {
        out::error("Usage: ddev tryout checkout <branch>");
        let mut s = String::from(
            "\nExamples:\n  ddev tryout checkout main     # latest development (v14)\n\
\x20 ddev tryout checkout 13.4     # v13 LTS\n  ddev tryout checkout 12.4     # v12 LTS\n\n",
        );
        worktree::ensure_branch_refs_quiet(ctx);
        s.push_str("Branches known locally:\n");
        for b in ctx.local_core_branches() {
            s.push_str(&format!("  {b}\n"));
        }
        print(&s);
        return Err(Exit(1));
    }
    let target = site::for_name(ctx, &target);
    let mut a = vec!["checkout", branch.as_str()];
    if !target.is_empty() {
        a.push(&target);
    }
    delegate(ctx, &a)
}

fn reset(ctx: &Ctx, args: &[String]) -> Res {
    require_core(ctx)?;
    let mut target = args.first().cloned().unwrap_or_default();
    // Several sites served: ask which one a reset rebuilds.
    if target.is_empty() && !site::served_names(ctx).is_empty() {
        match prompt::ask_site(ctx, "Reset which site?", &[]) {
            Some(t) => target = t,
            None => {
                explain_missing("ddev tryout reset [<site>]");
                if prompt::have_tty() {
                    return Err(Exit(1));
                }
            }
        }
    }
    let target = site::for_name(ctx, &target);
    let mut a = vec!["reset"];
    if !target.is_empty() {
        a.push(&target);
    }
    delegate(ctx, &a)
}

fn delete(ctx: &Ctx, args: &[String]) -> Res {
    let mut target = String::new();
    let mut yes = false;
    for a in args {
        match a.as_str() {
            "--yes" | "-y" => yes = true,
            a if target.is_empty() => target = a.to_string(),
            _ => {}
        }
    }
    // Bare with sites served: ask, "--all" being one of the answers. `--yes`
    // asks nothing — no target is the primary, as without served sites.
    if target.is_empty() && !yes && !site::served_names(ctx).is_empty() {
        match prompt::ask_site(
            ctx,
            "Wipe which site?",
            &["--all         every site, primary included"],
        ) {
            Some(t) => {
                target = if t.starts_with("--all") {
                    "--all".into()
                } else {
                    t
                }
            }
            None => {
                explain_missing("ddev tryout delete [<site>|--all]");
                if prompt::have_tty() {
                    return Err(Exit(1));
                }
            }
        }
    }
    // The active worktree's name means the primary, here as everywhere.
    if target != "--all" {
        target = site::for_name(ctx, &target);
    }
    let sites: Vec<String> = if target == "--all" {
        site::wipeable(ctx)
    } else if !site::is_primary(&target) {
        require_served(ctx, &target)?;
        vec![target.clone()]
    } else {
        vec![PRIMARY_SITE.to_string()]
    };
    if sites.iter().any(|s| site::is_primary(s)) && ctx.mode() == Mode::Project {
        // Refused before anyone is asked to confirm it.
        let _ = serve::delete_site(ctx, PRIMARY_SITE);
        return Err(Exit(1));
    }
    if sites.is_empty() {
        out::info("No served sites to wipe.");
        return Ok(());
    }
    if !yes {
        print(&serve::delete_warning(ctx, &target, &sites));
        match prompt::confirm("Are you sure?") {
            prompt::Confirm::Yes => {}
            prompt::Confirm::No => {
                out::info("Aborted.");
                return Ok(());
            }
            prompt::Confirm::NoTty => {
                out::error("Refusing to wipe without confirmation — nothing to ask on.");
                let t = if target.is_empty() {
                    String::new()
                } else {
                    format!(" {target}")
                };
                out::error(format!("  → ddev tryout delete{t} --yes"));
                return Err(Exit(1));
            }
        }
    }
    if site::is_primary(&target) {
        target.clear();
    }
    let mut a = vec!["delete"];
    if !target.is_empty() {
        a.push(&target);
    }
    a.push("--yes");
    delegate(ctx, &a)
}

fn exec(ctx: &Ctx, args: &[String]) -> Res {
    let mut target = args.first().cloned().unwrap_or_default();
    let mut cmd: Vec<String> = args.iter().skip(1).cloned().collect();
    let usage = "ddev tryout exec <site> <command> ...";
    if target.is_empty() {
        match prompt::ask_site(ctx, "Run in which site?", &[]) {
            Some(t) => target = t,
            None => {
                explain_missing(usage);
                if prompt::have_tty() {
                    return Err(Exit(1));
                }
            }
        }
    }
    if !target.is_empty() && cmd.is_empty() && prompt::have_tty() {
        let typed = prompt::ask_text(
            &format!("Command to run in {target} (after php)"),
            "vendor/bin/typo3 cache:flush",
        )
        .ok_or_else(|| {
            explain_missing(usage);
            Exit(1)
        })?;
        // A typed command line is meant to be split into words.
        cmd = typed.split_whitespace().map(String::from).collect();
    }
    if target.is_empty() || cmd.is_empty() {
        out::error(format!("Usage: {usage}"));
        let sites: String = site::served_names(ctx)
            .iter()
            .map(|n| format!(", {n}"))
            .collect();
        print(&format!(
            "\n  Examples:\n    ddev tryout exec v13 vendor/bin/typo3 cache:flush\n\
\x20   ddev tryout exec v13 composer show typo3/cms-core\n\n  Sites: primary{sites}\n"
        ));
        return Err(Exit(1));
    }
    let target = site::for_name(ctx, &target);
    require_served(ctx, &target)?;
    let mut a = vec!["exec", target.as_str()];
    a.extend(cmd.iter().map(String::as_str));
    delegate(ctx, &a)
}

/// A site served on the other database engine needs that engine's server
/// running before its TYPO3 setup: declare the service and restart DDEV first.
/// Nothing to do when `--db` names no engine, the project's own, or one that is
/// declared already.
fn ensure_db_service(ctx: &Ctx, args: &[String], no_restart: bool) -> Res {
    let mut value = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--db" {
            value = it.next().cloned();
        } else if let Some(v) = a.strip_prefix("--db=") {
            value = Some(v.to_string());
        }
    }
    // An unknown one is the container's to refuse, with the choices.
    let Some(engine) = value.as_deref().and_then(Db::parse) else {
        return Ok(());
    };
    let mut declared = webserver::declared_db_services(ctx);
    if !engine.needs_service(ctx) || declared.contains(&engine) {
        return Ok(());
    }
    declared.push(engine.clone());
    declared.sort();
    if let Err(e) = webserver::write_db_services(ctx, &declared) {
        out::error(format!(
            "Could not write {}: {e}",
            ctx.db_services_file().display()
        ));
        return Err(Exit(1));
    }
    if no_restart || std::env::var("TRYOUT_NO_RESTART").as_deref() == Ok("1") {
        out::error(format!(
            "The {} server is declared but runs only after a restart",
            engine.label()
        ));
        out::error("  → ddev restart   then run this again");
        return Err(Exit(1));
    }
    out::info(format!(
        "Restarting DDEV to start the {} server ({})...",
        engine.label(),
        engine.host(ctx)
    ));
    if ddev::restart() {
        Ok(())
    } else {
        out::error("ddev restart failed");
        out::error("  → ddev restart");
        Err(Exit(1))
    }
}

/// A database server no site and no kept database needs any more has just been
/// stopped: its volume holds nothing worth keeping. Left, it would outlive the
/// project — `ddev delete` removes only the volumes of servers it runs — and
/// hand its databases to the next project of the same name.
fn remove_unneeded_volumes(ctx: &Ctx, before: &[String]) {
    let now = webserver::restart_key(ctx);
    for key in before.iter().filter(|k| !now.contains(k)) {
        let Some(svc) = key.strip_prefix("service ") else {
            continue;
        };
        let volume = format!("ddev-{}-{svc}", ctx.env.sitename);
        if ctx.env.sitename.is_empty() || !ddev::remove_volume(&volume) {
            out::warn(format!("Could not remove the volume {volume}"));
            out::warn(format!("  → docker volume rm {volume}"));
        } else {
            out::info(format!(
                "Removed {volume}: no site or kept database uses it"
            ));
        }
    }
}

/// Restart DDEV when the served hostnames or database services changed: its
/// routing rule, certificate and containers are keyed on them and cannot be
/// refreshed from the container. `before` is the key from before the command.
fn restart_if_hosts_changed(ctx: &Ctx, before: &[String], no_restart: bool) -> Res {
    if webserver::restart_key(ctx) == before {
        return Ok(());
    }
    if no_restart || std::env::var("TRYOUT_NO_RESTART").as_deref() == Ok("1") {
        out::warn("Hostnames or database services changed — run 'ddev restart' to apply them.");
        return Ok(());
    }
    out::info("Restarting DDEV to apply the new hostnames and database services...");
    if ddev::restart() {
        out::success("DDEV restarted.");
        remove_unneeded_volumes(ctx, before);
        Ok(())
    } else {
        out::error("ddev restart failed");
        out::error("  → ddev restart");
        Err(Exit(1))
    }
}

// ─── worktree ───────────────────────────────────────────────────────────────

fn worktree(ctx: &Ctx, args: &[String]) -> Res {
    let (sub, rest) = match args.split_first() {
        Some((s, r)) => (s.as_str(), r),
        None => ("list", &[][..]),
    };
    available(ctx, &format!("worktree {sub}"))?;
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
        "add" => worktree_add(ctx, rest),
        "use" => {
            require_core(ctx)?;
            let name = match rest.first() {
                Some(n) => n.clone(),
                None => prompt::ask_worktree(
                    ctx,
                    "Point the primary site at which worktree?",
                    prompt::Filter::NonPrimary,
                )
                .ok_or_else(|| {
                    explain_missing("ddev tryout worktree use <name>");
                    Exit(1)
                })?,
            };
            let mut a = vec!["worktree", "use", name.as_str()];
            a.extend(rest.iter().skip(1).map(String::as_str));
            delegate(ctx, &a)
        }
        "remove" | "rm" => worktree_remove(ctx, rest),
        "serve" | "unserve" => {
            require_core(ctx)?;
            let (prompt_text, filter, usage) = if sub == "serve" {
                (
                    "Serve which worktree?",
                    prompt::Filter::Unserved,
                    "ddev tryout worktree serve <name> [--php 8.2] [--db postgres]",
                )
            } else {
                (
                    "Stop serving which worktree?",
                    prompt::Filter::Served,
                    "ddev tryout worktree unserve <name> [--drop-db]",
                )
            };
            let name = match rest.first() {
                Some(n) => n.clone(),
                None => prompt::ask_worktree(ctx, prompt_text, filter).ok_or_else(|| {
                    explain_missing(usage);
                    Exit(1)
                })?,
            };
            // --no-restart is the host's word: the container would reject it.
            let no_restart = rest.iter().any(|a| a == "--no-restart");
            let mut a = vec!["worktree", sub, name.as_str()];
            a.extend(
                rest.iter()
                    .skip(1)
                    .filter(|a| *a != "--no-restart")
                    .map(String::as_str),
            );
            if sub == "serve" {
                ensure_db_service(ctx, rest, no_restart)?;
            }
            let before = webserver::restart_key(ctx);
            delegate(ctx, &a)?;
            restart_if_hosts_changed(ctx, &before, no_restart)
        }
        "rename" => {
            require_core(ctx)?;
            let usage = "ddev tryout worktree rename <old> <new>";
            let old = match rest.first() {
                Some(n) => n.clone(),
                None => prompt::ask_worktree(ctx, "Rename which worktree?", prompt::Filter::All)
                    .ok_or_else(|| {
                        explain_missing(usage);
                        Exit(1)
                    })?,
            };
            let new = match rest.get(1) {
                Some(n) => n.clone(),
                None => prompt::ask_text(&format!("New name for {old}"), "").ok_or_else(|| {
                    explain_missing(usage);
                    Exit(1)
                })?,
            };
            let before = webserver::restart_key(ctx);
            delegate(ctx, &["worktree", "rename", &old, &new])?;
            restart_if_hosts_changed(ctx, &before, false)
        }
        _ => {
            out::error(format!("Unknown worktree command: {sub}"));
            print(&help::worktree());
            Err(Exit(1))
        }
    }
}

fn worktree_add(ctx: &Ctx, args: &[String]) -> Res {
    require_core(ctx)?;
    // The name is taken IN the loop: a flag in first position is never a name.
    let (mut name, mut branch, mut flags, mut no_restart) =
        (String::new(), String::new(), Vec::new(), false);
    let mut pr = String::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // Both no-ops now (worktrees are always detached); --detach is passed on.
            "--branch" => {}
            "--detach" | "--serve" | "--db-empty" => flags.push(a.clone()),
            "--no-restart" => no_restart = true,
            "--php" | "--db" | "--db-from" => {
                flags.push(format!("{a}={}", it.next().cloned().unwrap_or_default()))
            }
            a if a.starts_with("--php=")
                || a.starts_with("--db=")
                || a.starts_with("--db-from=") =>
            {
                flags.push(a.to_string())
            }
            "--pr" => pr = it.next().cloned().unwrap_or_default(),
            a if a.starts_with("--pr=") => pr = a["--pr=".len()..].to_string(),
            a if a.starts_with('-') => {
                out::error(format!("Unknown option: {a}"));
                out::error("  → ddev tryout worktree help");
                return Err(Exit(1));
            }
            a if name.is_empty() => name = a.to_string(),
            a => branch = a.to_string(),
        }
    }
    // A pull request: fetched here, with the user's credentials, into a ref
    // the container starts the worktree on. It names the worktree and its base.
    if !pr.is_empty() {
        let Some(number) = review::parse_number(&pr) else {
            out::error(format!("'{pr}' is not a pull request number"));
            out::error("  → ddev tryout worktree add --pr 123");
            return Err(Exit(1));
        };
        if !ctx.kind().opens_pull_requests() {
            out::error(format!(
                "{} takes its changes from Gerrit, not pull requests",
                ctx.kind().label()
            ));
            out::error(format!("  → ddev tryout patch {number}"));
            return Err(Exit(1));
        }
        if !branch.is_empty() {
            out::error("A pull request is its own base — leave out the branch");
            return Err(Exit(1));
        }
        review::fetch(&ctx.root, number).map_err(|l| {
            crate::core::fail(l);
            Exit(1)
        })?;
        if name.is_empty() {
            name = format!("pr-{number}");
        }
        flags.push(format!("--pr={number}"));
        ensure_db_service(ctx, &flags, no_restart)?;
        let before = webserver::restart_key(ctx);
        let mut a = vec!["worktree", "add", name.as_str()];
        a.extend(flags.iter().map(String::as_str));
        delegate(ctx, &a)?;
        return restart_if_hosts_changed(ctx, &before, no_restart);
    }
    if name.is_empty() {
        name = prompt::ask_text(
            "Name for the new worktree (becomes worktrees/<name>)",
            "e.g. bugfix-12345",
        )
        .ok_or_else(|| {
            explain_missing(
                "ddev tryout worktree add <name> [<branch>] [--serve] [--php 8.2] [--db postgres]",
            );
            Exit(1)
        })?;
    }
    worktree::validate_name(&name).map_err(|l| {
        crate::core::fail(l);
        Exit(1)
    })?;
    // Asked whether or not the name came in: naming a worktree says nothing
    // about which branch it sits on.
    let branch =
        prompt::ask_new_worktree_branch(ctx, &branch, "ddev tryout worktree add <name> [<branch>]")
            .ok_or(Exit(1))?;
    ensure_db_service(ctx, &flags, no_restart)?;
    let before = webserver::restart_key(ctx);
    let mut a = vec!["worktree", "add", name.as_str()];
    if !branch.is_empty() {
        a.push(&branch);
    }
    a.extend(flags.iter().map(String::as_str));
    delegate(ctx, &a)?;
    restart_if_hosts_changed(ctx, &before, no_restart)
}

/// Remove ALWAYS asks: it deletes the directory, and git's refusal to drop a
/// dirty tree is no longer the backstop. `--yes` is for a caller that asked.
fn worktree_remove(ctx: &Ctx, args: &[String]) -> Res {
    require_core(ctx)?;
    let name = match args.first() {
        Some(n) => n.clone(),
        None => prompt::ask_worktree(ctx, "Remove which worktree?", prompt::Filter::NonPrimary)
            .ok_or_else(|| {
                explain_missing("ddev tryout worktree remove <name> [--force]");
                Exit(1)
            })?,
    };
    let rest: Vec<&String> = args.iter().skip(1).collect();
    let yes = rest.iter().any(|a| *a == "--yes" || *a == "-y");
    let no_restart = rest.iter().any(|a| *a == "--no-restart");
    let passed: Vec<&str> = rest
        .iter()
        .filter(|a| **a != "--yes" && **a != "-y" && **a != "--no-restart")
        .map(|a| a.as_str())
        .collect();
    if !yes {
        let dir = ctx.core_worktree_dir(&name);
        let mut what = format!("worktree '{name}' and its directory");
        if worktree::is_dirty(&dir) {
            what.push_str(", including uncommitted changes");
        }
        if site::is_served(ctx, &name) {
            what.push_str(", plus its site and database");
        }
        out::warn(dir.display().to_string());
        match prompt::confirm(&format!("Remove {what}?")) {
            prompt::Confirm::Yes => {}
            prompt::Confirm::No => {
                out::info("Aborted.");
                return Ok(());
            }
            prompt::Confirm::NoTty => {
                out::error("Refusing to remove without confirmation — nothing to ask on.");
                out::error("  → run it in a terminal, or pass --yes");
                return Err(Exit(1));
            }
        }
    }
    // Like unserve: a served site's hostname goes, and so may the last need for
    // a database server — both take effect only with a restart, which also
    // lets the stopped server's volume be removed.
    let before = webserver::restart_key(ctx);
    let mut a = vec!["worktree", "remove", name.as_str()];
    a.extend(passed);
    delegate(ctx, &a)?;
    restart_if_hosts_changed(ctx, &before, no_restart)
}

// ─── cs ─────────────────────────────────────────────────────────────────────

fn cs(ctx: &Ctx, args: &[String]) -> Res {
    match args.first().map(String::as_str).unwrap_or("setup") {
        "help" | "-h" | "--help" => {
            print(&help::cs());
            Ok(())
        }
        "setup" => {
            require_core(ctx)?;
            // The username is the one thing setup may ask for: resolve it here,
            // where a prompt can be answered.
            let mut user = args.get(1).cloned().unwrap_or_default();
            if user.is_empty() {
                user = std::env::var("TRYOUT_GERRIT_USER").unwrap_or_default();
            }
            if user.is_empty() {
                user = crate::core::git::out(&ctx.root, &["config", "--get", "tryout.gerritUser"])
                    .unwrap_or_default();
            }
            if user.is_empty() {
                user = prompt::ask_text("Gerrit username (review.typo3.org)", "").ok_or_else(
                    || {
                        explain_missing("ddev tryout cs setup <username>");
                        Exit(1)
                    },
                )?;
            }
            delegate(ctx, &["cs", "setup", &user])?;
            crate::core::contrib::host_ssh_report(&user);
            Ok(())
        }
        "doctor" => {
            require_core(ctx)?;
            delegate(ctx, &["cs", "doctor"])?;
            let user = crate::core::git::out(&ctx.root, &["config", "--get", "tryout.gerritUser"])
                .unwrap_or_default();
            crate::core::contrib::host_ssh_report(&user);
            Ok(())
        }
        "uninstall" => {
            require_core(ctx)?;
            delegate(ctx, &["cs", "uninstall"])
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ctx::DdevEnv;

    #[test]
    fn picked_patches_are_appended_once_each_keeping_the_file() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(".ddev")).unwrap();
        let f = d.path().join(".ddev/config.tryout-patches.yaml");
        std::fs::write(
            &f,
            "# my patches\nweb_environment:\n  - TRYOUT_PATCHES=56947, 12345\n",
        )
        .unwrap();
        let ctx = Ctx::new(d.path(), DdevEnv::default());
        persist_patches(&ctx, &["12345".into(), "91234".into()]).unwrap();
        assert_eq!(
            std::fs::read_to_string(&f).unwrap(),
            "# my patches\nweb_environment:\n  - TRYOUT_PATCHES=56947,12345,91234\n"
        );
    }

    #[test]
    fn persisting_refuses_politely_without_a_patch_list() {
        let d = tempfile::tempdir().unwrap();
        let ctx = Ctx::new(d.path(), DdevEnv::default());
        assert_eq!(persist_patches(&ctx, &["1".into()]), Err(Exit(1)));
    }
}
