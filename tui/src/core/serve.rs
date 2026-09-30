//! Building, serving and wiping sites: the work behind `worktree serve`,
//! `unserve`, `delete`, `exec` and every rebuild. Container-side.

use std::path::Path;
use std::process::{Command, Stdio};

use super::ctx::Ctx;
use super::ctx::PRIMARY_SITE;
use super::db::{Db, Seed};
use super::kind::Mode;
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
        .env("TRYOUT_SITE", name);
    match ctx.mode() {
        Mode::Core => {
            c.env("TYPO3_DB_DBNAME", site::database(name));
        }
        // The primary is DDEV's own site, on DDEV's own settings.
        Mode::Project if !site::is_primary(name) => {
            c.envs(site::db_env(ctx, name));
        }
        Mode::Project => {}
    }
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
    let Some(c) = ctx.kind().php_constraint(&ctx.core_worktree_dir(name)) else {
        return fallback;
    };
    php::matching(&c, &php::available_versions())
        .pop()
        .unwrap_or(fallback)
}

/// Refuse early when a site's PHP cannot run the Core it is built on — Composer
/// reports it too, but as a resolver trace pointing the wrong way.
pub fn check_php_for_core(ctx: &Ctx, dir: &Path, php_version: &str, site_name: &str) -> Step {
    if php_version.is_empty() {
        return Ok(());
    }
    let Some(constraint) = ctx.kind().php_constraint(dir) else {
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
        "{}{branch} requires PHP {constraint}, but {who} PHP {php_version}",
        ctx.kind().label()
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
    if ctx.kind().supports("checkout") {
        out::error(
            "  → or switch Core to a branch this PHP can run: ddev tryout checkout <branch>",
        );
    }
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
        check_php_for_core(ctx, &ctx.active_core_dir(), &ctx.env.php_version, "")?;
        let instance = ctx.instance_dir();
        out::info("Running composer install...");
        if !proc::run("composer", &["install"], Some(&instance)) {
            out::error("Composer install failed");
            return Err(Failed);
        }
        run_rebuild_commands(ctx, name);
        out::success("Rebuild complete");
        return Ok(());
    }
    let php = site::php_version(ctx, name);
    check_php_for_core(ctx, &site::core_dir(ctx, name), &php, name)?;
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
    run_rebuild_commands(ctx, name);
    out::success(format!("Rebuild complete for '{name}'"));
    Ok(())
}

/// The primary's console, run directly from its instance; stdout shown, stderr
/// dropped. True on success — callers that only warn may ignore it.
pub fn console(instance: &Path, cmd: &[&str]) -> bool {
    let Some((program, args)) = cmd.split_first() else {
        return true;
    };
    let _ = std::io::Write::flush(&mut std::io::stdout());
    Command::new(instance.join(program))
        .args(args)
        .current_dir(instance)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// What runs after every install or rebuild (for TYPO3 extension:setup and a
/// cache flush), with the site's var/cache emptied first. Each is said and
/// run; one that fails is a warning, not a failed rebuild. False when any did.
pub fn run_rebuild_commands(ctx: &Ctx, name: &str) -> bool {
    let dir = site::dir(ctx, name);
    proc::clear_dir(&dir.join("var/cache"));
    let mut all = true;
    for cmd in ctx.kind().rebuild_commands() {
        let what = cmd[1..].join(" ");
        out::info(format!("Running {what}..."));
        // The primary runs on the project's own PHP, straight from its instance.
        let ok = if site::is_primary(name) {
            console(&dir, cmd)
        } else {
            exec_ok(ctx, name, cmd, true)
        };
        if !ok {
            out::warn(format!("{what} had warnings"));
            all = false;
        }
    }
    all
}

/// Where a site's settings.php waits while the site is gone — beside the site
/// directory, which unserve removes.
///
/// One per database server: a site switched from MySQL to Postgres and back
/// must get the MySQL settings back, not the Postgres ones
/// (`.<name>.<type>-<version>.settings.php`). The project's own server keeps
/// the plain name every earlier version wrote. Read while the site's marker
/// exists — it says the server. A project's app has no settings of ours to
/// keep: its `.kept` note only says which server holds the database.
pub fn saved_settings(ctx: &Ctx, name: &str) -> std::path::PathBuf {
    let db = site::db(ctx, name);
    let ext = match ctx.mode() {
        Mode::Core => "settings.php",
        Mode::Project => "kept",
    };
    let file = if db == Db::of_project(ctx) {
        format!(".{name}.{ext}")
    } else {
        format!(".{name}.{}.{ext}", db.slug())
    };
    ctx.instances_dir().join(file)
}

/// Where `unserve` keeps a SQLite site's database: beside the saved
/// settings.php, outside the site directory it deletes.
fn saved_sqlite(ctx: &Ctx, name: &str) -> std::path::PathBuf {
    ctx.instances_dir().join(format!(".{name}.sqlite"))
}

/// `typo3 setup` and the database settings it reads from the environment, for
/// the site's own server. config.tryout.yaml sets TYPO3_DB_HOST=db and
/// TYPO3_DB_PORT=3306 for the whole container (it cannot know the type, nor
/// that a site runs on another server), so all three are passed here.
fn setup_args(ctx: &Ctx, name: &str) -> (Vec<String>, Vec<(&'static str, String)>) {
    let db = site::db(ctx, name);
    (
        ctx.kind().setup_command(&ctx.env.webserver_type),
        vec![
            ("TYPO3_DB_DRIVER", db.engine.setup_driver().to_string()),
            ("TYPO3_DB_HOST", db.host(ctx)),
            ("TYPO3_DB_PORT", db.engine.port().to_string()),
        ],
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
    // A SQLite database kept by unserve goes back where settings.php expects it.
    let kept = saved_sqlite(ctx, name);
    if kept.is_dir() {
        let dir = db::sqlite_dir(ctx, name);
        let _ = std::fs::create_dir_all(dir.parent().expect("has a directory"));
        if dir.exists() || std::fs::rename(&kept, &dir).is_err() {
            out::warn(format!("Could not restore {}", kept.display()));
        }
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
    let (args, db_env) = setup_args(ctx, name);
    let db_env: Vec<(&str, &str)> = db_env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    out::info(format!(
        "Running TYPO3 setup for '{name}' (db {db_name}, PHP {php})..."
    ));
    if exec(ctx, name, &args, &db_env, false) != 0 {
        // A half-finished setup can leave settings.php behind; kept, it would
        // read as "already configured" on every later start, over an empty
        // database. It did not exist before this run, so it goes.
        let _ = std::fs::remove_file(&settings);
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
        name,
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
/// `db`: the engine to run on; None keeps the one it has (the project's for a
/// site served the first time).
pub fn serve(ctx: &Ctx, name: &str, php_version: &str, db: Option<Db>, seed: &Seed) -> Step {
    serve_since(ctx, name, php_version, db, seed, None)
}

/// `serve`, judging "is this a new hostname?" against `before` — what DDEV had
/// registered before the command began. `--switch` unserves first, and a
/// snapshot taken after that would call the site's own hostname new: nothing
/// would reload the webserver, and the hostname would fall through to the
/// primary until the next restart.
pub fn serve_since(
    ctx: &Ctx,
    name: &str,
    php_version: &str,
    db: Option<Db>,
    seed: &Seed,
    before: Option<Vec<String>>,
) -> Step {
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
    let hosts_before = before.unwrap_or_else(|| webserver::restart_key(ctx));
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
            "PHP {p} (highest {} accepts; --php overrides)",
            ctx.kind().label()
        ));
        p
    } else {
        php_version.to_string()
    };
    check_php_for_core(ctx, &core, &php, name)?;
    let dir = site::dir(ctx, name);
    if ctx.mode() == Mode::Core {
        let _ = std::fs::create_dir_all(dir.join("config/system"));
        let _ = std::fs::create_dir_all(dir.join("var"));
    }

    let current = site::db(ctx, name);
    let server = db.unwrap_or_else(|| current.clone());
    // Its settings.php names the old server: switching goes through a fresh site.
    if site::is_served(ctx, name) && server != current {
        out::error(format!(
            "'{name}' runs on {} — its settings point there",
            current.label()
        ));
        out::error(format!(
            "  → ddev tryout worktree serve {name} --db {} --switch   (keeps the old database)",
            server.name()
        ));
        return Err(Failed);
    }
    if server.needs_service(ctx) && ctx.in_container {
        db::wait_until_ready(ctx, &server)?;
    }

    // The marker IS "served"; it only stands if we get to the end.
    let marker = site::marker(ctx, name);
    let _ = site::write_marker(ctx, name, &php, &server);
    let result = match ctx.mode() {
        Mode::Core => build_site(ctx, name, &php, &core, &dir),
        Mode::Project => build_project_site(ctx, name, &php, &dir, seed),
    };
    if result.is_err() {
        let _ = std::fs::remove_file(&marker);
        return result;
    }

    out::success(format!(
        "Site '{name}' prepared — PHP {php}, db {} ({})",
        site::database(name),
        server.label()
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
    match ctx.kind().backend_path() {
        Some(path) => out::print_line(&format!(
            "  {DIM}then: https://{}{path}  (admin / Password.1){NC}",
            site::hostname(ctx, name)
        )),
        None => out::print_line(&format!(
            "  {DIM}then: https://{}/{NC}",
            site::hostname(ctx, name)
        )),
    }
    Ok(())
}

/// A project's worktree is served as it is: its database (a fresh one seeded,
/// by default from the primary's), its own `composer install` on the site's
/// PHP, its vhost. What the app needs to reach its database is in the vhost's
/// environment (`site::db_env`).
fn build_project_site(ctx: &Ctx, name: &str, php: &str, dir: &Path, seed: &Seed) -> Step {
    // Kept by an earlier unserve: it stays as it is.
    let fresh = !db::has_tables(ctx, name);
    db::ensure_site_database(ctx, name)?;
    if fresh {
        let from = match seed {
            Seed::Empty => None,
            Seed::Default => Some((Db::of_project(ctx), site::database(PRIMARY_SITE))),
            Seed::Copy { db, name } => Some((db.clone(), name.clone())),
        };
        if let Some((db, from_name)) = from {
            db::copy_into(ctx, &db, &from_name, name);
        }
    }
    copy_local_config(ctx, dir);
    if dir.join("composer.json").is_file() {
        out::info(format!(
            "Installing dependencies for {name} on PHP {php} (this takes a moment)..."
        ));
        // Through `exec`: its scripts (post-install hooks) see the site's
        // database, not the primary's.
        if !exec_ok(
            ctx,
            name,
            &["/usr/local/bin/composer", "install", "--no-interaction"],
            false,
        ) {
            out::error(format!("composer install failed for {name}"));
            return Err(Failed);
        }
    }
    if let Err(e) = webserver::write_vhost(ctx, name, php) {
        out::error(format!("Could not write the vhost for {name}: {e}"));
        return Err(Failed);
    }
    if let Err(e) = webserver::write_worktree_config(ctx) {
        out::error(format!("Could not write .ddev/config.worktrees.yaml: {e}"));
        return Err(Failed);
    }
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
    if ctx.mode() == Mode::Project {
        return unserve_project(ctx, name, keep_db);
    }
    let saved = saved_settings(ctx, name);
    let settings = site::dir(ctx, name).join("config/system/settings.php");
    if keep_db && settings.is_file() {
        // The kept database is useless without it: stop before deleting anything.
        let kept = saved
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::copy(&settings, &saved));
        // A SQLite database is a file in the site: move it out too.
        let lite = db::sqlite_dir(ctx, name);
        let kept = kept.and_then(|_| {
            if lite.is_dir() {
                let to = saved_sqlite(ctx, name);
                let _ = std::fs::remove_dir_all(&to);
                std::fs::rename(&lite, &to)
            } else {
                Ok(())
            }
        });
        if let Err(e) = kept {
            out::error(format!("Could not keep {}: {e}", saved.display()));
            out::error(format!(
                "  → nothing was removed; or discard the database too: ddev tryout worktree unserve {name} --drop-db"
            ));
            return Err(Failed);
        }
    } else {
        // First, while the marker still says which server holds it: once the
        // site's directory is gone, it would read as the project's.
        let db_name = site::database(name);
        out::info(format!("Dropping database {db_name}..."));
        if !db::drop(ctx, name) {
            out::error(format!(
                "Could not drop database {db_name} — nothing was removed"
            ));
            out::error(format!(
                "  → keep it: ddev tryout worktree unserve {name}   (without --drop-db)"
            ));
            return Err(Failed);
        }
        let _ = std::fs::remove_file(&saved);
        let _ = std::fs::remove_dir_all(saved_sqlite(ctx, name));
    }
    let _ = std::fs::remove_dir_all(site::dir(ctx, name));
    stop_serving(ctx, name);
    Ok(())
}

/// A project site's worktree is the user's: only tryout's marker goes, and
/// the database, kept or dropped.
fn unserve_project(ctx: &Ctx, name: &str, keep_db: bool) -> Step {
    let note = saved_settings(ctx, name);
    if keep_db {
        let _ = std::fs::create_dir_all(ctx.instances_dir());
        if let Err(e) = std::fs::write(&note, format!("{}\n", site::db(ctx, name).name())) {
            out::error(format!("Could not keep {}: {e}", note.display()));
            return Err(Failed);
        }
    } else {
        let db_name = site::database(name);
        out::info(format!("Dropping database {db_name}..."));
        if !db::drop(ctx, name) {
            out::error(format!(
                "Could not drop database {db_name} — nothing was removed"
            ));
            return Err(Failed);
        }
        let _ = std::fs::remove_file(&note);
        let _ = std::fs::remove_dir_all(db::sqlite_dir(ctx, name));
    }
    let _ = std::fs::remove_file(site::marker(ctx, name));
    // Empty now, unless a SQLite database waits in it.
    let _ = std::fs::remove_dir(site::state_dir(ctx, name));
    stop_serving(ctx, name);
    Ok(())
}

/// The vhost and hostname go; the webserver stops answering for the site.
fn stop_serving(ctx: &Ctx, name: &str) {
    let _ = std::fs::remove_file(webserver::vhost_file(ctx, name));
    if let Err(e) = webserver::write_worktree_config(ctx) {
        out::warn(format!(
            "Could not rewrite .ddev/config.worktrees.yaml: {e}"
        ));
    }
    out::success(format!("Site '{name}' removed (worktree kept)"));
    // The hostname set shrank, so the restart stays; the dead vhost must stop
    // answering now all the same.
    if ctx.in_container && webserver::sync_and_reload(ctx) {
        out::info("Stopped serving it now; the host releases the hostname.");
    } else {
        out::info("Its hostname is released when DDEV restarts.");
    }
}

/// A project site's reset: its database recreated as a fresh copy of the
/// primary's, the way it started.
fn empty_project_database(ctx: &Ctx, name: &str) -> Step {
    // The project's own database is DDEV's: `ddev snapshot` and `ddev
    // import-db` look after it.
    if site::is_primary(name) {
        out::error("The primary's database is the project's own — tryout leaves it alone");
        out::error(
            "  → ddev snapshot, then ddev import-db   or name a served site: ddev tryout delete <site>",
        );
        return Err(Failed);
    }
    let db_name = site::database(name);
    out::info(format!("Recreating database {db_name}..."));
    if !db::recreate(ctx, name) || db::ensure_site_database(ctx, name).is_err() {
        out::error(format!("Failed to reset database {db_name}"));
        return Err(Failed);
    }
    if db::copy_into(
        ctx,
        &Db::of_project(ctx),
        &site::database(PRIMARY_SITE),
        name,
    ) {
        out::success(format!(
            "Database {db_name} is a fresh copy of the primary's"
        ));
    } else {
        out::success(format!("Database {db_name} is empty"));
    }
    Ok(())
}

/// A worktree checks out what is committed; the project's local settings
/// (`.env`, `.env.local` — ignored by git, APP_KEY and the like) are not. A
/// copy of the primary's goes in where the worktree has none; the site's own
/// database still wins, from the environment.
fn copy_local_config(ctx: &Ctx, dir: &Path) {
    for f in [".env", ".env.local"] {
        let (from, to) = (ctx.root.join(f), dir.join(f));
        if from.is_file() && !to.exists() && std::fs::copy(&from, &to).is_ok() {
            out::info(format!("Copied the project's {f} into the worktree"));
        }
    }
}

/// What `delete` is about to destroy, one line per site.
pub fn delete_warning(ctx: &Ctx, target: &str, sites: &[String]) -> String {
    let mut s = format!("\n{YELLOW}{BOLD}Warning:{NC} this destroys data for:\n");
    for n in sites {
        let label = if site::is_primary(n) { "primary" } else { n };
        if ctx.mode() == Mode::Project {
            s.push_str(&format!(
                "  {BOLD}{label}{NC} — database {}\n",
                site::database(n)
            ));
            continue;
        }
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
    if ctx.mode() == Mode::Project {
        return empty_project_database(ctx, name);
    }
    let db_name = site::database(name);
    let docroot = site::docroot(ctx, name);
    out::info(format!("[1/4] Recreating database {db_name}..."));
    if !db::recreate(ctx, name) {
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
    let (args, db_env) = setup_args(ctx, name);
    let db_env: Vec<(&str, &str)> = db_env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    out::info("[4/4] Running TYPO3 setup + extension:setup...");
    if exec(ctx, name, &args, &db_env, false) != 0 {
        out::error(format!("TYPO3 setup failed for {name}"));
        return Err(Failed);
    }
    run_rebuild_commands(ctx, name);
    out::success("Setup complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ctx::DdevEnv;

    #[test]
    fn unserving_a_project_site_keeps_the_users_worktree() {
        let d = crate::core::ctx::tests::project_repo();
        let c = Ctx::new(
            d.path(),
            DdevEnv {
                database: "mariadb:11.8".into(),
                sitename: "shop".into(),
                ..DdevEnv::default()
            },
        );
        let wt = c.core_worktree_dir("feat");
        std::fs::create_dir_all(wt.join("public")).unwrap();
        site::write_marker(&c, "feat", "8.4", &Db::parse("postgres:16").unwrap()).unwrap();
        assert!(site::is_served(&c, "feat"));

        unserve(&c, "feat", true).unwrap();
        assert!(!site::is_served(&c, "feat"));
        assert!(wt.join("public").is_dir(), "the worktree is the user's");
        // The kept database still holds its server.
        assert_eq!(site::extra_dbs(&c), [Db::parse("postgres:16").unwrap()]);
        assert!(c.instances_dir().join(".feat.postgres-16.kept").is_file());
    }

    #[test]
    fn a_projects_own_database_is_never_wiped() {
        let d = crate::core::ctx::tests::project_repo();
        let c = Ctx::new(d.path(), DdevEnv::default());
        assert!(delete_site(&c, "@primary").is_err());
    }

    #[test]
    fn setup_gets_the_driver_host_and_port_of_the_sites_database() {
        let d = tempfile::tempdir().unwrap();
        let ctx = |db: &str| {
            Ctx::new(
                d.path(),
                DdevEnv {
                    database: db.into(),
                    ..DdevEnv::default()
                },
            )
        };
        // The container's TYPO3_DB_PORT says 3306 whatever the database is.
        let env = |c: &Ctx, site: &str| -> Vec<(&'static str, String)> { setup_args(c, site).1 };
        let e = |d: &str, h: &str, p: &str| {
            vec![
                ("TYPO3_DB_DRIVER", d.to_string()),
                ("TYPO3_DB_HOST", h.to_string()),
                ("TYPO3_DB_PORT", p.to_string()),
            ]
        };
        assert_eq!(
            env(&ctx("postgres:16"), "@primary"),
            e("postgres", "db", "5432")
        );
        let maria = ctx("mariadb:10.11");
        assert_eq!(env(&maria, "@primary"), e("mysqli", "db", "3306"));
        // A site on another server is set up against that server.
        std::fs::create_dir_all(site::dir(&maria, "pg")).unwrap();
        site::write_marker(&maria, "pg", "8.4", &Db::parse("postgres:16").unwrap()).unwrap();
        assert_eq!(
            env(&maria, "pg"),
            e("postgres", "tryout-postgres-16", "5432")
        );
    }
}
