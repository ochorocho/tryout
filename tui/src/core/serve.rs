//! Building, serving and wiping sites: the work behind `worktree serve`,
//! `unserve`, `delete`, `exec` and every rebuild. Container-side.

use std::path::Path;
use std::process::{Command, Stdio};

use super::ctx::Ctx;
use super::out::{self, BOLD, DIM, NC, YELLOW};
use super::{Failed, Step, composer, db, fpm, git, php, proc, site, webserver, worktree};

/// Run a command in a site's context: its PHP, its composer root, its database.
/// The binary gets the arguments as they are — no shell in between. Returns the
/// exit code.
pub fn exec(
    ctx: &Ctx,
    name: &str,
    args: &[String],
    extra_env: &[(&str, &str)],
    quiet: bool,
) -> i32 {
    let php = site::php_version(ctx, name);
    let bin = if !php.is_empty() && php != ctx.env.php_version {
        format!("php{php}")
    } else {
        "php".into()
    };
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut c = Command::new(&bin);
    c.args(args)
        .current_dir(site::dir(ctx, name))
        .env("TYPO3_DB_DBNAME", site::database(name))
        .env("TRYOUT_SITE", name);
    for (k, v) in extra_env {
        c.env(k, v);
    }
    if quiet {
        c.stdout(Stdio::null()).stderr(Stdio::null());
    }
    match c.status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(_) => {
            if !quiet {
                eprintln!("{bin}: command not found");
            }
            127
        }
    }
}

fn exec_ok(ctx: &Ctx, name: &str, args: &[&str], quiet: bool) -> bool {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    exec(ctx, name, &args, &[], quiet) == 0
}

/// The highest PHP a worktree's Core accepts, among those the image provides;
/// the project's own when the constraint cannot be read.
pub fn best_php(ctx: &Ctx, name: &str) -> String {
    let fallback = if ctx.env.php_version.is_empty() {
        "8.5".to_string()
    } else {
        ctx.env.php_version.clone()
    };
    let Some(c) = php::core_constraint(&ctx.core_worktree_dir(name).join("composer.json")) else {
        return fallback;
    };
    php::matching(&c, &php::available_versions())
        .pop()
        .unwrap_or(fallback)
}

/// Refuse early when a site's PHP cannot run the Core it is built on — Composer
/// reports it too, but as a resolver trace pointing the wrong way.
pub fn check_php_for_core(dir: &Path, php_version: &str, site_name: &str) -> Step {
    if php_version.is_empty() {
        return Ok(());
    }
    let Some(constraint) = php::core_constraint(&dir.join("composer.json")) else {
        return Ok(());
    };
    if php::satisfies(&constraint, php_version) {
        return Ok(());
    }
    let branch = git::out(dir, &["branch", "--show-current"]).unwrap_or_default();
    let branch = if branch.is_empty() {
        String::new()
    } else {
        format!(" ({branch})")
    };
    let who = if site_name.is_empty() {
        "the project runs".to_string()
    } else {
        format!("site '{site_name}' runs")
    };
    out::error(format!(
        "TYPO3 Core{branch} requires PHP {constraint}, but {who} PHP {php_version}"
    ));
    let best = php::matching(&constraint, &php::available_versions())
        .pop()
        .unwrap_or_else(|| "<version>".into());
    if site_name.is_empty() {
        out::error(format!(
            "  → ddev config --php-version={best} && ddev restart"
        ));
    } else {
        out::error(format!(
            "  → ddev tryout worktree serve {site_name} --php {best}"
        ));
    }
    out::error("  → or switch Core to a branch this PHP can run: ddev tryout checkout <branch>");
    Err(Failed)
}

/// Drop a site's vendor/ before Composer installs against a different Core:
/// the plugins already loaded from it would run against the new code.
pub fn wipe_vendor(ctx: &Ctx, name: &str) -> Step {
    let rel = if site::is_primary(name) {
        String::new()
    } else {
        format!("TYPO3-Instances/{name}/")
    };
    out::info(format!(
        "Removing {rel}vendor/ — Core changed, a stale install cannot be updated in place"
    ));
    let v = site::vendor(ctx, name);
    if v.exists() && std::fs::remove_dir_all(&v).is_err() {
        out::error(format!("Could not remove {rel}vendor/"));
        return Err(Failed);
    }
    Ok(())
}

/// `sync` with its outcome printed the way the PHP script printed it.
pub fn sync_composer(instance: &Path, core: &Path) -> bool {
    match composer::sync(instance, core) {
        Ok(msg) => {
            out::print_line(&msg);
            true
        }
        Err(e) => {
            eprintln!("{e}");
            false
        }
    }
}

/// composer install, extension:setup and a cache flush for a site, under its own
/// PHP, composer root and database.
pub fn rebuild(ctx: &Ctx, name: &str) -> Step {
    if site::is_primary(name) {
        check_php_for_core(&ctx.active_core_dir(), &ctx.env.php_version, "")?;
        let instance = ctx.instance_dir();
        out::info("Running composer install...");
        if !proc::run("composer", &["install"], Some(&instance)) {
            out::error("Composer install failed");
            return Err(Failed);
        }
        out::info("Running extension:setup...");
        typo3(&instance, &["extension:setup"]);
        out::info("Flushing caches...");
        proc::clear_dir(&instance.join("var/cache"));
        typo3(&instance, &["cache:flush"]);
        out::success("Rebuild complete");
        return Ok(());
    }
    let php = site::php_version(ctx, name);
    check_php_for_core(&site::core_dir(ctx, name), &php, name)?;
    out::info(format!(
        "Running composer install for '{name}' on PHP {php}..."
    ));
    let dir = site::dir(ctx, name);
    let working = format!("--working-dir={}", dir.display());
    if !proc::run(
        &format!("php{php}"),
        &[
            "/usr/local/bin/composer",
            "install",
            &working,
            "--no-interaction",
        ],
        None,
    ) {
        out::error(format!("Composer install failed for {name}"));
        return Err(Failed);
    }
    out::info(format!("Running extension:setup for '{name}'..."));
    exec_ok(ctx, name, &["vendor/bin/typo3", "extension:setup"], true);
    out::info(format!("Flushing caches for '{name}'..."));
    proc::clear_dir(&dir.join("var/cache"));
    exec_ok(ctx, name, &["vendor/bin/typo3", "cache:flush"], true);
    out::success(format!("Rebuild complete for '{name}'"));
    Ok(())
}

/// The primary's console, run directly from its instance; stdout shown, stderr
/// dropped. True on success — callers that only warn may ignore it.
pub fn typo3(instance: &Path, args: &[&str]) -> bool {
    let _ = std::io::Write::flush(&mut std::io::stdout());
    Command::new(instance.join("vendor/bin/typo3"))
        .args(args)
        .current_dir(instance)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Where a site's settings.php waits while the site is gone — beside the site
/// directory, which unserve removes.
pub fn saved_settings(ctx: &Ctx, name: &str) -> std::path::PathBuf {
    ctx.instances_dir().join(format!(".{name}.settings.php"))
}

fn setup_args(ctx: &Ctx) -> (Vec<String>, &'static str) {
    let server_type =
        if ctx.env.webserver_type.starts_with("apache") || ctx.env.webserver_type.is_empty() {
            "apache"
        } else {
            "other"
        };
    let driver = if ctx.env.is_postgres() {
        "postgres"
    } else {
        "mysqli"
    };
    (
        vec![
            "vendor/bin/typo3".into(),
            "setup".into(),
            "--no-interaction".into(),
            "--force".into(),
            format!("--server-type={server_type}"),
        ],
        driver,
    )
}

/// First-run TYPO3 setup for a site. A database that outlived its site (unserve
/// keeps it) gets its saved settings.php back instead: TYPO3 refuses to set up
/// into a populated database.
pub fn setup_typo3(ctx: &Ctx, name: &str) -> Step {
    if !site::is_served(ctx, name) {
        out::error(format!("Site '{name}' is not served"));
        return Err(Failed);
    }
    let db_name = site::database(name);
    let php = site::php_version(ctx, name);
    let settings = site::dir(ctx, name).join("config/system/settings.php");
    if settings.is_file() {
        out::info(format!("Site '{name}' already configured"));
        return Ok(());
    }
    if db::has_tables(ctx, name) {
        let saved = saved_settings(ctx, name);
        if saved.is_file() {
            let _ = std::fs::create_dir_all(settings.parent().expect("has a directory"));
            if std::fs::copy(&saved, &settings).is_ok() {
                out::success(format!("Site '{name}' restored — existing database kept"));
                return Ok(());
            }
        }
        out::error(format!(
            "Database {db_name} already holds an install, and there is"
        ));
        out::error(format!("no saved settings.php for '{name}' to go with it."));
        out::error(format!(
            "  → ddev tryout worktree unserve {name} --drop-db   then serve again"
        ));
        return Err(Failed);
    }
    let (args, driver) = setup_args(ctx);
    out::info(format!(
        "Running TYPO3 setup for '{name}' (db {db_name}, PHP {php})..."
    ));
    if exec(ctx, name, &args, &[("TYPO3_DB_DRIVER", driver)], false) != 0 {
        out::error(format!("TYPO3 setup failed for {name}"));
        return Err(Failed);
    }
    out::success(format!("Site '{name}' set up"));
    setup_frontend(ctx, name);
    Ok(())
}

/// A rendered frontend for a fresh site: EXT:styleguide's demo where its
/// generator exists (13.4+), nothing where it does not. Best-effort — the
/// backend already works — and every DB write waits for the generate.
pub fn setup_frontend(ctx: &Ctx, name: &str) {
    if !exec_ok(
        ctx,
        name,
        &["vendor/bin/typo3", "help", "styleguide:generate"],
        true,
    ) {
        out::info(format!(
            "No frontend generator for this TYPO3 version — '{name}' serves the backend only"
        ));
        return;
    }
    out::info(format!(
        "Generating a styleguide demo frontend for '{name}'..."
    ));
    if !exec_ok(
        ctx,
        name,
        &[
            "vendor/bin/typo3",
            "styleguide:generate",
            "frontend",
            "--create",
        ],
        true,
    ) {
        out::warn(format!(
            "styleguide frontend generation failed for '{name}' — backend still works"
        ));
        return;
    }
    // The generator hides the site root; unhide exactly styleguide's own.
    let revealed = db::site_sql(
        ctx,
        &site::database(name),
        "UPDATE pages SET hidden=0 WHERE is_siteroot=1 AND tx_styleguide_containsdemo='tx_styleguide_frontend_root'",
    )
    .is_some_and(|o| o.status.success());
    if !revealed {
        out::warn(format!(
            "Could not reveal the styleguide frontend page for '{name}'"
        ));
    }
    exec_ok(ctx, name, &["vendor/bin/typo3", "cache:flush"], true);
    out::success(format!("Frontend ready for '{name}' (styleguide demo)"));
}

/// Make a worktree a live site: its own tree, overlay, database, vhost and FPM.
pub fn serve(ctx: &Ctx, name: &str, php_version: &str) -> Step {
    worktree::validate_name(name).map_err(super::fail)?;
    if site::is_primary(name) {
        out::error(format!("'{name}' is reserved"));
        return Err(Failed);
    }
    if let Some(other) = site::database_taken_by(ctx, name) {
        out::error(format!(
            "'{name}' would share database {} with worktree '{other}'",
            site::database(name)
        ));
        out::error(format!(
            "  → rename one of them: ddev tryout worktree rename {name} <new-name>"
        ));
        return Err(Failed);
    }
    // Before the marker exists: what DDEV last registered.
    let hosts_before = webserver::served_hostname_set(ctx);
    let core = ctx.core_worktree_dir(name);
    if !core.is_dir() {
        out::error(format!("No worktree '{name}'"));
        out::error(format!("  → ddev tryout worktree add {name} <branch>"));
        return Err(Failed);
    }
    // It names a binary, a socket and a vhost line: digits and one dot only.
    let valid_php = |v: &str| {
        v.split_once('.').is_some_and(|(a, b)| {
            [a, b]
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
        })
    };
    if !php_version.is_empty() && !valid_php(php_version) {
        out::error(format!("Invalid PHP version '{php_version}'"));
        out::error(format!("  → ddev tryout worktree serve {name} --php 8.4"));
        return Err(Failed);
    }
    let php = if php_version.is_empty() {
        let p = best_php(ctx, name);
        out::info(format!(
            "PHP {p} (highest this Core accepts; --php overrides)"
        ));
        p
    } else {
        php_version.to_string()
    };
    check_php_for_core(&core, &php, name)?;
    let dir = site::dir(ctx, name);
    let _ = std::fs::create_dir_all(dir.join("config/system"));
    let _ = std::fs::create_dir_all(dir.join("var"));

    // The marker IS "served"; it only stands if we get to the end.
    let marker = site::marker(ctx, name);
    let _ = std::fs::write(&marker, format!("php={php}\n"));
    let result = build_site(ctx, name, &php, &core, &dir);
    if result.is_err() {
        let _ = std::fs::remove_file(&marker);
        return result;
    }

    out::success(format!(
        "Site '{name}' prepared — PHP {php}, db {}",
        site::database(name)
    ));
    // Before the reload, which would route to a socket nobody listens on.
    if ctx.in_container {
        fpm::ensure_running(ctx, &php)?;
    }
    if webserver::apply_site_config(ctx, &hosts_before) {
        out::success("Applied without a restart.");
    } else {
        out::info(format!(
            "New hostname {} — it needs DDEV restarted.",
            site::hostname(ctx, name)
        ));
    }
    out::print_line(&format!(
        "  {DIM}then: https://{}/typo3/  (admin / Password.1){NC}",
        site::hostname(ctx, name)
    ));
    Ok(())
}

fn build_site(ctx: &Ctx, name: &str, php: &str, core: &Path, dir: &Path) -> Step {
    // TYPO3 loads additional.php from its own root; one source of truth, three
    // levels up (system → config → <name>) and into primary/.
    let link = dir.join("config/system/additional.php");
    let _ = std::fs::remove_file(&link);
    let _ = std::os::unix::fs::symlink("../../../primary/config/system/additional.php", &link);

    out::info(format!(
        "Generating TYPO3-Instances/{name}/composer.tryout.json..."
    ));
    if let Err(e) = composer::site_overlay(ctx, name, php) {
        eprintln!("{e}");
        out::error(format!(
            "Failed to generate the Composer overlay for {name}"
        ));
        return Err(Failed);
    }
    out::info(format!(
        "Syncing TYPO3-Instances/{name}/composer.tryout.json with its Core sysexts..."
    ));
    if !sync_composer(dir, core) {
        out::error(format!("composer sync failed for {name}"));
        return Err(Failed);
    }
    db::ensure_site_database(ctx, name)?;
    out::info(format!(
        "Installing dependencies for {name} on PHP {php} (this takes a moment)..."
    ));
    let working = format!("--working-dir={}", dir.display());
    if !proc::run(
        &format!("php{php}"),
        &[
            "/usr/local/bin/composer",
            "install",
            &working,
            "--no-interaction",
        ],
        None,
    ) {
        out::error(format!("composer install failed for {name}"));
        return Err(Failed);
    }
    // Without these the site is marked served yet nothing routes to it.
    if let Err(e) = webserver::write_vhost(ctx, name, php) {
        out::error(format!("Could not write the vhost for {name}: {e}"));
        return Err(Failed);
    }
    if let Err(e) = webserver::write_worktree_config(ctx) {
        out::error(format!("Could not write .ddev/config.worktrees.yaml: {e}"));
        return Err(Failed);
    }
    setup_typo3(ctx, name)
}

/// Drop a site, keep its worktree — and, unless `keep_db` is false, its
/// database with the settings.php that goes with it.
pub fn unserve(ctx: &Ctx, name: &str, keep_db: bool) -> Step {
    worktree::validate_name(name).map_err(super::fail)?;
    if !site::is_served(ctx, name) {
        out::error(format!("Site '{name}' is not served"));
        return Err(Failed);
    }
    let saved = saved_settings(ctx, name);
    let settings = site::dir(ctx, name).join("config/system/settings.php");
    if keep_db && settings.is_file() {
        // The kept database is useless without it: stop before deleting anything.
        let kept = saved
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::copy(&settings, &saved));
        if let Err(e) = kept {
            out::error(format!("Could not keep {}: {e}", saved.display()));
            out::error(format!(
                "  → nothing was removed; or discard the database too: ddev tryout worktree unserve {name} --drop-db"
            ));
            return Err(Failed);
        }
    } else {
        let _ = std::fs::remove_file(&saved);
    }
    let _ = std::fs::remove_file(webserver::vhost_file(ctx, name));
    let _ = std::fs::remove_dir_all(site::dir(ctx, name));
    if let Err(e) = webserver::write_worktree_config(ctx) {
        out::warn(format!(
            "Could not rewrite .ddev/config.worktrees.yaml: {e}"
        ));
    }
    if !keep_db {
        let db_name = site::database(name);
        out::info(format!("Dropping database {db_name}..."));
        if !db::drop(ctx, &db_name) {
            out::error(format!("Could not drop database {db_name}"));
            out::error(format!(
                "  → the site is gone; drop it by hand: ddev mysql (or ddev psql) → DROP DATABASE {db_name};"
            ));
            return Err(Failed);
        }
    }
    out::success(format!("Site '{name}' removed (worktree kept)"));
    // The hostname set shrank, so the restart stays; the dead vhost must stop
    // answering now all the same.
    if ctx.in_container && webserver::sync_and_reload(ctx) {
        out::info("Stopped serving it now; the host releases the hostname.");
    } else {
        out::info("Its hostname is released when DDEV restarts.");
    }
    Ok(())
}

/// What `delete` is about to destroy, one line per site.
pub fn delete_warning(ctx: &Ctx, target: &str, sites: &[String]) -> String {
    let mut s = format!("\n{YELLOW}{BOLD}Warning:{NC} this destroys data for:\n");
    for n in sites {
        let label = if site::is_primary(n) { "primary" } else { n };
        s.push_str(&format!(
            "  {BOLD}{label}{NC} — database {}, {}/fileadmin, settings.php\n",
            site::database(n),
            site::docroot(ctx, n).display()
        ));
    }
    if target.is_empty() && !site::served_names(ctx).is_empty() {
        s.push_str(&format!(
            "  {DIM}served sites are untouched — use 'delete <site>' or 'delete --all'{NC}\n"
        ));
    }
    s.push('\n');
    s
}

/// Wipe one site back to a fresh install: its database, fileadmin and
/// settings.php, then setup.
pub fn delete_site(ctx: &Ctx, name: &str) -> Step {
    let db_name = site::database(name);
    let docroot = site::docroot(ctx, name);
    out::info(format!("[1/4] Recreating database {db_name}..."));
    if !db::recreate(ctx, &db_name) {
        out::error(format!("Failed to reset database {db_name}"));
        return Err(Failed);
    }
    out::success(format!("Database {db_name} recreated"));
    let rel = docroot
        .strip_prefix(&ctx.root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| docroot.display().to_string());
    out::info(format!("[2/4] Clearing {rel}/fileadmin..."));
    let fileadmin = docroot.join("fileadmin");
    for e in std::fs::read_dir(&fileadmin)
        .into_iter()
        .flatten()
        .flatten()
    {
        let p = e.path();
        let _ = if p.is_dir() && !p.is_symlink() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
    }
    out::success("fileadmin cleared");
    out::info("[3/4] Removing settings.php...");
    let _ = std::fs::remove_file(site::dir(ctx, name).join("config/system/settings.php"));
    out::success("Configuration removed");
    let (args, driver) = setup_args(ctx);
    out::info("[4/4] Running TYPO3 setup + extension:setup...");
    if exec(ctx, name, &args, &[("TYPO3_DB_DRIVER", driver)], false) != 0 {
        out::error(format!("TYPO3 setup failed for {name}"));
        return Err(Failed);
    }
    if !exec_ok(ctx, name, &["vendor/bin/typo3", "extension:setup"], true) {
        out::warn("extension:setup had warnings");
    }
    if !exec_ok(ctx, name, &["vendor/bin/typo3", "cache:flush"], true) {
        out::warn("cache:flush had warnings");
    }
    out::success("Setup complete");
    Ok(())
}
