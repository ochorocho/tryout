//! `tryout ctr <verb>`: the far end of `ddev tryout`, inside the web container.
//! Arguments arrive resolved — nothing in here prompts; a missing one is an
//! error with the usage line. Nothing in here runs `ddev`.

use crate::core::ctx::{Ctx, PRIMARY_SITE};
use crate::core::out::{self, BOLD, CYAN, DIM, GREEN, NC, TEXT, YELLOW};
use crate::core::{Step, git, patch, php, proc, prompt, serve, site, status, vsort, worktree};

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
        Some(Verb::Download) => download(ctx, rest),
        Some(Verb::Checkout) => checkout(ctx, rest),
        Some(Verb::Patch) => patch(ctx, rest),
        Some(Verb::Reset) => reset(ctx, rest),
        Some(Verb::Delete) => delete(ctx, rest),
        Some(Verb::Exec) => exec(ctx, rest),
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
        "add" => worktree_add(ctx, rest),
        "use" => {
            // No --force any more: switching touches no checkout. Tolerated, ignored.
            let name = rest.first().map(String::as_str).unwrap_or("");
            if name.is_empty() {
                return usage("ddev tryout worktree use <name>");
            }
            step(worktree::use_core(ctx, name))?;
            print("\n");
            out::success(format!("Now on Core '{name}' — {}", ctx.env.primary_url));
            Ok(())
        }
        "remove" | "rm" => {
            let name = rest.first().map(String::as_str).unwrap_or("");
            let force = rest.get(1).map(String::as_str) == Some("--force");
            if name.is_empty() {
                return usage("ddev tryout worktree remove <name> [--force]");
            }
            // A served site owns a tree and a database; they go first.
            if site::is_served(ctx, name) {
                out::warn(format!(
                    "'{name}' is currently served — removing its site first"
                ));
                step(serve::unserve(ctx, name, false))?;
            }
            step(worktree::remove(ctx, name, force))
        }
        "serve" => {
            let name = rest.first().map(String::as_str).unwrap_or("");
            let php = php_flag(&rest[rest.len().min(1)..]);
            if name.is_empty() {
                return usage("ddev tryout worktree serve <name> [--php 8.2]");
            }
            step(serve::serve(ctx, name, &php))
        }
        "unserve" => {
            let name = rest.first().map(String::as_str).unwrap_or("");
            let keep_db = rest.get(1).map(String::as_str) != Some("--drop-db");
            if name.is_empty() {
                return usage("ddev tryout worktree unserve <name> [--drop-db]");
            }
            step(serve::unserve(ctx, name, keep_db))
        }
        "rename" => {
            let (old, new) = (
                rest.first().map(String::as_str).unwrap_or(""),
                rest.get(1).map(String::as_str).unwrap_or(""),
            );
            if old.is_empty() || new.is_empty() {
                out::error("Usage: ddev tryout worktree rename <old> <new>");
                out::error("  Renames the checkout only — the branch is untouched.");
                return Err(Exit(1));
            }
            step(worktree::rename(ctx, old, new))
        }
        _ => {
            out::error(format!("Unknown worktree command: {sub}"));
            Err(Exit(1))
        }
    }
}

// ─── the verbs that change things ───────────────────────────────────────────

/// A core step's failure, already reported, as exit 1.
fn step(r: Step) -> Res {
    r.map_err(|_| Exit(1))
}

fn usage(line: &str) -> Res {
    out::error(format!("Usage: {line}"));
    Err(Exit(1))
}

/// `--php 8.2` or `--php=8.2` anywhere in the arguments; the last one wins.
fn php_flag(args: &[String]) -> String {
    let mut php = String::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--php" {
            php = it.next().cloned().unwrap_or_default();
        } else if let Some(v) = a.strip_prefix("--php=") {
            php = v.to_string();
        }
    }
    php
}

/// A served site's name, or an error when nothing is served under it.
fn served(ctx: &Ctx, name: &str) -> Res {
    if site::is_primary(name) || site::is_served(ctx, name) {
        return Ok(());
    }
    out::error(format!("No served site '{name}'"));
    out::error("  → ddev tryout worktree list");
    Err(Exit(1))
}

fn basename(p: &std::path::Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn worktree_add(ctx: &Ctx, args: &[String]) -> Res {
    let name = args.first().cloned().unwrap_or_default();
    let (mut branch, mut serve_it, mut php) = (String::new(), false, String::new());
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--detach" => {}
            "--serve" => serve_it = true,
            "--php" => php = it.next().cloned().unwrap_or_default(),
            a if a.starts_with("--php=") => php = a["--php=".len()..].to_string(),
            a => branch = a.to_string(),
        }
    }
    step(worktree::validate_name(&name).map_err(serve::report))?;
    // A named PHP version only takes effect on a served site.
    if !php.is_empty() {
        serve_it = true;
    }
    let _ = std::fs::create_dir_all(ctx.worktrees_dir());
    let branch = if branch.is_empty() {
        ctx.branch().to_string()
    } else {
        branch
    };
    step(worktree::add(ctx, &name, &branch))?;
    print("\n");
    if serve_it {
        return step(serve::serve(ctx, &name, &php));
    }
    print(&format!(
        "  {DIM}→ ddev tryout worktree use {name}      (switch the primary site){NC}\n\
\x20 {DIM}→ ddev tryout worktree serve {name}    (give it its own URL){NC}\n"
    ));
    Ok(())
}

fn download(ctx: &Ctx, args: &[String]) -> Res {
    let mut reset = false;
    let mut target = PRIMARY_SITE.to_string();
    for a in args {
        match a.as_str() {
            "--reset" | "-r" => reset = true,
            "" => {}
            a => target = a.to_string(),
        }
    }
    let mut core = ctx.root.clone();
    let mut branch = ctx.branch().to_string();
    if !site::is_primary(&target) {
        served(ctx, &target)?;
        core = site::core_dir(ctx, &target);
        // A legacy worktree created attached still tracks its base; a detached
        // one has no upstream and answers from the remote branches instead.
        branch = git::out(&core, &["rev-parse", "--abbrev-ref", "@{upstream}"])
            .map(|u| u.strip_prefix("origin/").unwrap_or(&u).to_string())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| worktree::detect_detached_base_branch(&core));
    }
    let site_arg = if site::is_primary(&target) {
        String::new()
    } else {
        format!(" {target}")
    };

    if !ctx.has_core() {
        out::info(format!(
            "Cloning TYPO3 Core ({branch} branch) into the project root..."
        ));
        out::info("This may take a few minutes on first run.");
        if worktree::clone_into_root(ctx, &branch).is_err() {
            out::error("Failed to clone TYPO3 Core repository");
            return Err(Exit(1));
        }
        worktree::ensure_relative_paths(ctx);
        out::success("TYPO3 Core cloned into the project root");
        return Ok(());
    }
    patch::ensure_gerrit_remote(&core);

    if reset {
        out::warn(format!(
            "Resetting {} to origin/{branch}...",
            basename(&core)
        ));
        out::warn("All local changes and applied patches will be lost.");
        worktree::reset_to_base(ctx, &core, &branch, &target);
        out::success(format!("Reset to origin/{branch}"));
        return step(serve::rebuild(ctx, &target));
    }

    out::info("Updating TYPO3 Core...");
    proc::git(&core, &["fetch", "origin"]);
    let current = git::out(&core, &["branch", "--show-current"]).unwrap_or_default();
    if current.is_empty() {
        out::error(format!(
            "Detached checkout — update means resetting it to origin/{branch}"
        ));
        out::error(format!(
            "  → ddev tryout download{site_arg} --reset   (discards local changes)"
        ));
        let pinned = if site_arg.is_empty() {
            String::new()
        } else {
            format!(" --site {target}")
        };
        out::error(format!(
            "  → or switch version: ddev tryout checkout <branch>{pinned}"
        ));
        return Err(Exit(1));
    }
    if !git::ok(&core, &["diff", "--quiet"]) || !git::ok(&core, &["diff", "--cached", "--quiet"]) {
        out::error("Working tree has uncommitted changes");
        out::error(format!("  → Reset: ddev tryout download{site_arg} --reset"));
        return Err(Exit(1));
    }
    // Rebase, never merge: Gerrit takes one commit with a stable Change-Id.
    if !proc::git(&core, &["pull", "--rebase", "origin", &branch]) {
        out::error("Pull failed");
        out::error(format!("  → Reset: ddev tryout download{site_arg} --reset"));
        return Err(Exit(1));
    }
    out::success(format!(
        "{} updated to latest origin/{branch}",
        basename(&core)
    ));
    step(serve::rebuild(ctx, &target))
}

fn checkout(ctx: &Ctx, args: &[String]) -> Res {
    let target_branch = args.first().cloned().unwrap_or_default();
    let target = args.get(1).cloned().unwrap_or_else(|| PRIMARY_SITE.into());
    if target_branch.is_empty() {
        return usage("ddev tryout checkout <branch>");
    }
    require_core(ctx)?;
    let mut core = ctx.root.clone();
    if !site::is_primary(&target) {
        served(ctx, &target)?;
        core = site::core_dir(ctx, &target);
        out::info(format!("Switching site '{target}' ({})", basename(&core)));
    }
    prompt::spin("Fetching latest branches", || {
        proc::git(&core, &["fetch", "origin"])
    });

    if !git::ok(
        &core,
        &[
            "ls-remote",
            "--exit-code",
            "--heads",
            "origin",
            &target_branch,
        ],
    ) {
        out::error(format!("Branch '{target_branch}' does not exist on origin"));
        let mut branches: Vec<String> = git::lines(&core, &["ls-remote", "--heads", "origin"])
            .iter()
            .filter_map(|l| l.rsplit('/').next().map(String::from))
            .collect();
        vsort::sort(&mut branches);
        let list: String = branches.iter().map(|b| format!("  {b}\n")).collect();
        print(&format!("\nAvailable branches:\n{list}"));
        return Err(Exit(1));
    }
    let current =
        git::out(&core, &["branch", "--show-current"]).unwrap_or_else(|| "detached".into());
    if current == target_branch {
        out::info(format!(
            "Already on {target_branch}, resetting to latest origin/{target_branch}..."
        ));
    } else {
        out::info(format!("Switching from {current} to {target_branch}..."));
    }
    // git refuses a branch checked out in another worktree; say which instead.
    if ctx.worktrees_dir().is_dir() {
        let mut holder = None;
        let mut current_wt = String::new();
        for l in git::lines(&core, &["worktree", "list", "--porcelain"]) {
            if let Some(w) = l.strip_prefix("worktree ") {
                current_wt = w.to_string();
            } else if l == format!("branch refs/heads/{target_branch}") {
                holder = Some(current_wt.clone());
                break;
            }
        }
        if let Some(h) = holder {
            let hp = std::path::PathBuf::from(&h);
            if hp.canonicalize().ok() != core.canonicalize().ok() {
                out::error(format!(
                    "Branch '{target_branch}' is checked out in {}",
                    basename(&hp)
                ));
                let prefix = format!("{}/", ctx.worktrees_dir().display());
                out::error(format!(
                    "  → ddev tryout worktree use {}",
                    h.strip_prefix(&prefix).unwrap_or(&h)
                ));
                return Err(Exit(1));
            }
        }
    }
    if !proc::git_no_stderr(&core, &["checkout", &target_branch])
        && !proc::git(
            &core,
            &[
                "checkout",
                "-b",
                &target_branch,
                &format!("origin/{target_branch}"),
            ],
        )
    {
        return Err(Exit(1));
    }
    if !proc::git(
        &core,
        &["reset", "--hard", &format!("origin/{target_branch}")],
    ) || !proc::git(&core, &["clean", "-fd"])
    {
        return Err(Exit(1));
    }
    proc::clear_dir(&site::dir(ctx, &target).join("var/cache"));
    out::success(format!("Core switched to {target_branch}"));
    if site::is_primary(&target) {
        composer(ctx)?;
    } else {
        out::info(format!(
            "Syncing TYPO3-Instances/{target}/composer.tryout.json..."
        ));
        if !serve::sync_composer(&site::dir(ctx, &target), &ctx.core_worktree_dir(&target)) {
            return Err(Exit(1));
        }
    }
    step(serve::wipe_vendor(ctx, &target))?;
    step(serve::rebuild(ctx, &target))?;
    print("\n");
    out::success(format!("Now on TYPO3 branch {target_branch}"));
    print(&format!(
        "  {BOLD}Site:{TEXT} https://{}{NC}\n",
        site::hostname(ctx, &target)
    ));
    Ok(())
}

fn patch(ctx: &Ctx, args: &[String]) -> Res {
    require_core(ctx)?;
    patch::ensure_gerrit_remote(&ctx.root);
    let mut target = PRIMARY_SITE.to_string();
    let mut ids = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--site" => {
                target = it
                    .next()
                    .cloned()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| PRIMARY_SITE.into())
            }
            a if a.starts_with("--site=") => target = a["--site=".len()..].to_string(),
            a => ids.push(a.to_string()),
        }
    }
    let mut core = ctx.root.clone();
    let mut branch = ctx.branch().to_string();
    if !site::is_primary(&target) {
        served(ctx, &target)?;
        core = site::core_dir(ctx, &target);
        branch = worktree::detect_detached_base_branch(&core);
        out::info(format!(
            "Patching site '{target}' ({}, base {branch})",
            basename(&core)
        ));
    }
    if !ids.is_empty() {
        // One rebuild at the end, not a composer install per change.
        let mut applied = 0;
        let mut failed = false;
        for id in &ids {
            let (outcome, _) = patch::apply(&core, &branch, id);
            if outcome.failed() {
                failed = true;
                break;
            }
            if outcome == patch::Outcome::Applied {
                applied += 1;
            }
            if ids.len() > 1 {
                print("\n");
            }
        }
        if applied > 0 {
            step(serve::rebuild(ctx, &target))?;
        }
        return if failed { Err(Exit(1)) } else { Ok(()) };
    }
    let list: String = std::env::var("TRYOUT_PATCHES")
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if list.is_empty() {
        out::info("No patches configured.");
        print(
            "\nUsage:\n  ddev tryout patch              Browse the open changes and pick\n\
\x20 ddev tryout patch <change-id>  Apply a single Gerrit patch\n\n\
Configure in .ddev/config.tryout-patches.yaml:\n  TRYOUT_PATCHES=56947,12345\n",
        );
        return Ok(());
    }
    match patch::apply_all(&core, &branch, &list) {
        Ok(0) => Ok(()),
        Ok(_) => step(serve::rebuild(ctx, &target)),
        Err(_) => Err(Exit(1)),
    }
}

fn reset(ctx: &Ctx, args: &[String]) -> Res {
    require_core(ctx)?;
    let target = args.first().cloned().unwrap_or_else(|| PRIMARY_SITE.into());
    let mut core = ctx.root.clone();
    let mut branch = ctx.branch().to_string();
    if !site::is_primary(&target) {
        served(ctx, &target)?;
        core = site::core_dir(ctx, &target);
        branch = worktree::detect_detached_base_branch(&core);
    }
    print("\n");
    out::info(format!(
        "Resetting {} to latest origin/{branch}...",
        basename(&core)
    ));
    print("\n");
    out::info("[1/2] Resetting git repository...");
    worktree::reset_to_base(ctx, &core, &branch, &target);
    out::success(format!("Git reset to origin/{branch}"));
    out::info("[2/2] Rebuilding...");
    step(serve::rebuild(ctx, &target))?;
    print("\n");
    out::success(format!(
        "Reset complete! Site: {}",
        site::hostname(ctx, &target)
    ));
    Ok(())
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
    let sites: Vec<String> = if target == "--all" {
        std::iter::once(PRIMARY_SITE.to_string())
            .chain(site::served_names(ctx))
            .collect()
    } else if !target.is_empty() {
        if !site::is_served(ctx, &target) {
            out::error(format!("No served site '{target}'"));
            out::error("  → ddev tryout worktree list");
            return Err(Exit(1));
        }
        vec![target.clone()]
    } else {
        vec![PRIMARY_SITE.to_string()]
    };
    if !yes {
        print(&serve::delete_warning(ctx, &target, &sites));
        match prompt::confirm("Are you sure?") {
            prompt::Confirm::Yes => {}
            prompt::Confirm::No => {
                out::info("Aborted.");
                return Ok(());
            }
            prompt::Confirm::NoTty => {
                out::error("Refusing without confirmation — pass --yes");
                return Err(Exit(1));
            }
        }
    }
    for s in &sites {
        print("\n");
        let label = if site::is_primary(s) { "primary" } else { s };
        out::info(format!("── {label} ──"));
        step(serve::delete_site(ctx, s))?;
    }
    print("\n");
    out::success("Fresh setup complete!");
    for s in &sites {
        print(&format!("  {BOLD}{}/typo3/{NC}\n", site::hostname(ctx, s)));
    }
    print(&format!(
        "  {BOLD}Login:{TEXT}    admin / Password.1{NC}\n\n"
    ));
    Ok(())
}

fn exec(ctx: &Ctx, args: &[String]) -> Res {
    let Some((target, cmd)) = args.split_first().filter(|(_, c)| !c.is_empty()) else {
        return usage("ddev tryout exec <site> <command> ...");
    };
    if !site::is_primary(target) && !site::is_served(ctx, target) {
        out::error(format!("No served site '{target}'"));
        out::error("  → ddev tryout worktree list");
        return Err(Exit(1));
    }
    match serve::exec(ctx, target, cmd, &[], false) {
        0 => Ok(()),
        c => Err(Exit(c)),
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
