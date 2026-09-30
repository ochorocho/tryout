//! Served sites. Every instance lives at TYPO3-Instances/<name>; the primary is
//! the one called `primary`, and `@primary` (or nothing) names it in arguments.

use std::path::PathBuf;

use super::ctx::{Ctx, PRIMARY_INSTANCE, PRIMARY_SITE};
use super::db::Db;
use super::kind::Mode;

pub fn is_primary(name: &str) -> bool {
    name.is_empty() || name == PRIMARY_SITE
}

/// A worktree's name works wherever a site is asked for: the active worktree IS
/// the primary and has no site of its own name.
pub fn for_name(ctx: &Ctx, name: &str) -> String {
    if !name.is_empty()
        && name != PRIMARY_SITE
        && !is_served(ctx, name)
        && name == ctx.active_worktree_name()
    {
        PRIMARY_SITE.into()
    } else {
        name.into()
    }
}

/// The site's app: where its composer.json, vendor/ and docroot live. In core
/// mode an instance under TYPO3-Instances/; in project mode the worktree
/// itself (the project root for the primary).
pub fn dir(ctx: &Ctx, name: &str) -> PathBuf {
    match (ctx.mode(), is_primary(name)) {
        (_, true) => ctx.instance_dir(),
        (Mode::Core, false) => ctx.instances_dir().join(name),
        (Mode::Project, false) => ctx.core_worktree_dir(name),
    }
}

/// Where tryout keeps what it knows about a site: its marker, and what unserve
/// saves. The instance itself in core mode; beside the others in
/// .ddev/tryout-sites/ in project mode, out of the user's worktree.
pub fn state_dir(ctx: &Ctx, name: &str) -> PathBuf {
    match ctx.mode() {
        Mode::Core => dir(ctx, name),
        Mode::Project if is_primary(name) => ctx.instances_dir().join(PRIMARY_INSTANCE),
        Mode::Project => ctx.instances_dir().join(name),
    }
}

/// The directory the webserver serves: `public/` of an instance in core mode;
/// in project mode the worktree's copy of the project's own docroot.
pub fn docroot(ctx: &Ctx, name: &str) -> PathBuf {
    match ctx.mode() {
        Mode::Core => dir(ctx, name).join("public"),
        Mode::Project => {
            let d = dir(ctx, name);
            let rel = ctx.env.docroot.trim_matches('/');
            if rel.is_empty() { d } else { d.join(rel) }
        }
    }
}

pub fn vendor(ctx: &Ctx, name: &str) -> PathBuf {
    dir(ctx, name).join("vendor")
}

/// The Core checkout a site serves: the primary follows `worktree use`, a served
/// site is nailed to its own worktree.
pub fn core_dir(ctx: &Ctx, name: &str) -> PathBuf {
    if is_primary(name) {
        ctx.active_core_dir()
    } else {
        ctx.core_worktree_dir(name)
    }
}

/// The Core a site runs on and the branch that Core is based on. The primary's
/// is the root checkout — or, after `worktree use`, that worktree.
pub fn core_and_base(ctx: &Ctx, name: &str) -> (PathBuf, String) {
    let core = core_dir(ctx, name);
    if core == ctx.root {
        return (core, ctx.branch().to_string());
    }
    // A legacy worktree created attached still tracks its base; a detached one
    // has no upstream and is placed by the remote branches instead.
    let branch = super::git::out(&core, &["rev-parse", "--abbrev-ref", "@{upstream}"])
        .map(|u| u.strip_prefix("origin/").unwrap_or(&u).to_string())
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| super::worktree::detect_detached_base_branch(&core, ctx.kind()));
    (core, branch)
}

/// `<name>.<project>` for extras, the bare project for the primary — DDEV
/// appends .ddev.site to additional_hostnames itself.
pub fn hostname_short(ctx: &Ctx, name: &str) -> String {
    if is_primary(name) {
        ctx.env.sitename.clone()
    } else {
        format!("{name}.{}", ctx.env.sitename)
    }
}

pub fn hostname(ctx: &Ctx, name: &str) -> String {
    format!("{}.ddev.site", hostname_short(ctx, name))
}

/// `db` for the primary, `db_<name>` with every byte MySQL would reject in a
/// bare identifier replaced by `_`.
pub fn database(name: &str) -> String {
    if is_primary(name) {
        return "db".into();
    }
    let safe: String = name
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'_' {
                b as char
            } else {
                '_'
            }
        })
        .collect();
    format!("db_{safe}")
}

/// Another worktree whose site would share `name`'s database: `feat-x`,
/// `feat.x` and `feat_x` all map to `db_feat_x`.
pub fn database_taken_by(ctx: &Ctx, name: &str) -> Option<String> {
    let db = database(name);
    super::worktree::worktree_names(ctx)
        .into_iter()
        .find(|other| other != name && database(other) == db)
}

/// A site is served when its marker exists; the primary always is.
pub fn is_served(ctx: &Ctx, name: &str) -> bool {
    is_primary(name) || marker(ctx, name).is_file()
}

pub fn marker(ctx: &Ctx, name: &str) -> PathBuf {
    state_dir(ctx, name).join(".tryout-site")
}

/// The PHP a site runs, from its marker's `php=` line; the project's otherwise.
pub fn php_version(ctx: &Ctx, name: &str) -> String {
    if !is_primary(name)
        && let Ok(m) = std::fs::read_to_string(marker(ctx, name))
        && let Some(v) = m.lines().find_map(|l| l.strip_prefix("php="))
    {
        return v.trim().to_string();
    }
    ctx.env.php_version.clone()
}

/// The database server a site runs on, from its marker's `db=` line; the
/// project's otherwise (every site served before there was a choice).
pub fn db(ctx: &Ctx, name: &str) -> Db {
    if !is_primary(name)
        && let Ok(m) = std::fs::read_to_string(marker(ctx, name))
        && let Some(d) = m
            .lines()
            .find_map(|l| l.strip_prefix("db="))
            .and_then(|v| Db::parse_recorded(v.trim()))
    {
        return d;
    }
    Db::of_project(ctx)
}

/// Mark a site served, on this PHP and database server.
pub fn write_marker(ctx: &Ctx, name: &str, php: &str, db: &Db) -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir(ctx, name))?;
    std::fs::write(marker(ctx, name), format!("php={php}\ndb={}\n", db.name()))
}

/// A project site's database and URL, as its vhost and `exec` hand them over
/// (`appenv`: tryout's names and its framework's; DDEV's own settings name
/// `db` for every worktree). SQLite's name is the file, kept in the site's
/// state beside its marker.
pub fn db_env(ctx: &Ctx, name: &str) -> Vec<(&'static str, String)> {
    let db = db(ctx, name);
    let sqlite = db.engine == super::db::Engine::Sqlite;
    let database = if sqlite {
        ctx.in_container(
            &super::db::sqlite_dir(ctx, name).join(format!("{}.sqlite", database(name))),
        )
        .display()
        .to_string()
    } else {
        database(name)
    };
    let host = if sqlite { String::new() } else { db.host(ctx) };
    super::appenv::vars(
        &ctx.env.project_type,
        &super::appenv::SiteDb {
            db: &db,
            host: &host,
            name: &database,
        },
        &format!("https://{}", hostname(ctx, name)),
    )
}

/// The database servers needed besides the project's own: one extra service
/// each for the servers served sites run on, and for those still holding a
/// database `unserve` kept (its saved settings name the server:
/// `.<name>.<type>-<version>.settings.php`). SQLite needs none. Sorted, once
/// each.
pub fn extra_dbs(ctx: &Ctx) -> Vec<Db> {
    let files: Vec<String> = std::fs::read_dir(ctx.instances_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // Matched against the servers there are, not parsed: a worktree name may
    // hold dots and dashes itself.
    let kept = Db::choices().into_iter().filter(|d| {
        let [settings, kept] = [".settings.php", ".kept"].map(|e| format!(".{}{e}", d.slug()));
        files
            .iter()
            .any(|f| f.starts_with('.') && (f.ends_with(&settings) || f.ends_with(&kept)))
    });
    let mut v: Vec<Db> = served_names(ctx)
        .iter()
        .map(|n| db(ctx, n))
        .chain(kept)
        .filter(|d| d.needs_service(ctx))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// What `delete --all` wipes: every site, and in core mode the primary too —
/// a project's own database is DDEV's.
pub fn wipeable(ctx: &Ctx) -> Vec<String> {
    let primary = (ctx.mode() == Mode::Core).then(|| PRIMARY_SITE.to_string());
    primary.into_iter().chain(served_names(ctx)).collect()
}

/// The EXTRA served sites, sorted; never the primary, which the glob also sees.
pub fn served_names(ctx: &Ctx) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(ctx.instances_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n != PRIMARY_INSTANCE && is_served(ctx, n))
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ctx::{DdevEnv, tests::core_repo};

    fn ctx(root: &std::path::Path) -> Ctx {
        Ctx::new(
            root,
            DdevEnv {
                sitename: "unitproj".into(),
                php_version: "8.5".into(),
                ..Default::default()
            },
        )
    }

    #[test]
    fn sites_resolve_under_typo3_instances() {
        let c = ctx(std::path::Path::new("/p"));
        assert_eq!(
            dir(&c, "@primary"),
            PathBuf::from("/p/TYPO3-Instances/primary")
        );
        assert_eq!(dir(&c, ""), PathBuf::from("/p/TYPO3-Instances/primary"));
        assert_eq!(
            docroot(&c, "x"),
            PathBuf::from("/p/TYPO3-Instances/x/public")
        );
        assert_eq!(
            vendor(&c, "x"),
            PathBuf::from("/p/TYPO3-Instances/x/vendor")
        );
    }

    #[test]
    fn a_project_site_is_handed_its_own_database_in_the_environment() {
        let d = crate::core::ctx::tests::project_repo();
        let c = Ctx::new(
            d.path(),
            DdevEnv {
                database: "mariadb:11.8".into(),
                ..Default::default()
            },
        );
        let env = |name: &str| -> Vec<(String, String)> {
            db_env(&c, name)
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect()
        };
        let pairs = |p: &[(&str, &str)]| -> Vec<(String, String)> {
            p.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        // Served on the project's server: its own database there.
        write_marker(&c, "feat-x", "8.4", &Db::of_project(&c)).unwrap();
        assert_eq!(
            env("feat-x"),
            pairs(&[
                ("TRYOUT_DB_DRIVER", "mariadb"),
                ("TRYOUT_DB_NAME", "db_feat_x"),
                ("TRYOUT_DB_HOST", "db"),
                ("TRYOUT_DB_PORT", "3306"),
                ("TRYOUT_DB_USER", "db"),
                ("TRYOUT_DB_PASSWORD", "db"),
                (
                    "DATABASE_URL",
                    "mysql://db:db@db:3306/db_feat_x?serverVersion=mariadb-11.8.0&charset=utf8mb4"
                ),
                ("TRYOUT_URL", "https://feat-x..ddev.site"),
            ])
        );
        // On another server: that server's host and port.
        write_marker(&c, "pg", "8.4", &Db::parse("postgres:16").unwrap()).unwrap();
        let pg = env("pg");
        assert!(pg.contains(&("TRYOUT_DB_HOST".into(), "tryout-postgres-16".into())));
        assert!(pg.contains(&("TRYOUT_DB_PORT".into(), "5432".into())));
        // SQLite: a file in the site's state, out of the worktree; no server.
        write_marker(&c, "lite", "8.4", &Db::parse("sqlite").unwrap()).unwrap();
        assert_eq!(
            env("lite"),
            pairs(&[
                ("TRYOUT_DB_DRIVER", "sqlite"),
                (
                    "TRYOUT_DB_NAME",
                    "/var/www/html/.ddev/tryout-sites/lite/sqlite/db_lite.sqlite"
                ),
                (
                    "DATABASE_URL",
                    "sqlite:////var/www/html/.ddev/tryout-sites/lite/sqlite/db_lite.sqlite"
                ),
                ("TRYOUT_URL", "https://lite..ddev.site"),
            ])
        );
        // `delete --all` never takes the project's own database.
        assert_eq!(wipeable(&c), ["feat-x", "lite", "pg"]);
    }

    #[test]
    fn a_projects_sites_are_its_worktrees_and_their_state_waits_in_ddev() {
        // A repository of the user's own: project mode.
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        assert!(
            std::process::Command::new("git")
                .args(["init", "-q"])
                .current_dir(root)
                .status()
                .unwrap()
                .success()
        );
        let c = Ctx::new(
            root,
            DdevEnv {
                docroot: "web".into(),
                ..Default::default()
            },
        );
        assert_eq!(c.mode(), Mode::Project);
        // The primary is the project itself, served from its own docroot.
        assert_eq!(dir(&c, "@primary"), root);
        assert_eq!(docroot(&c, "@primary"), root.join("web"));
        // A site is its worktree, with the same docroot inside it.
        assert_eq!(dir(&c, "feature"), root.join("worktrees/feature"));
        assert_eq!(docroot(&c, "feature"), root.join("worktrees/feature/web"));
        // What tryout knows about it stays out of the worktree.
        assert_eq!(
            marker(&c, "feature"),
            root.join(".ddev/tryout-sites/feature/.tryout-site")
        );
        assert_eq!(c.instances_dir(), root.join(".ddev/tryout-sites"));
        // The container sees it under /var/www/html.
        assert_eq!(
            c.in_container(&docroot(&c, "feature")),
            PathBuf::from("/var/www/html/worktrees/feature/web")
        );
        // A docroot at the project root: the worktree itself.
        let flat = Ctx::new(root, DdevEnv::default());
        assert_eq!(docroot(&flat, "feature"), root.join("worktrees/feature"));
    }

    #[test]
    fn databases_keep_db_for_the_primary_and_are_sanitised() {
        assert_eq!(database("@primary"), "db");
        assert_eq!(database("v13"), "db_v13");
        assert_eq!(database("a.b-c"), "db_a_b_c");
        assert_eq!(database("é"), "db___");
    }

    #[test]
    fn hostnames_derive_from_the_project_name() {
        let c = ctx(std::path::Path::new("/p"));
        assert_eq!(hostname(&c, "@primary"), "unitproj.ddev.site");
        assert_eq!(hostname_short(&c, "v13"), "v13.unitproj");
        assert_eq!(hostname(&c, "v13"), "v13.unitproj.ddev.site");
    }

    #[test]
    fn a_site_keeps_its_server_and_every_other_server_is_listed_as_extra() {
        let d = tempfile::tempdir().unwrap();
        let c = ctx(d.path()); // no DDEV_DATABASE: a MariaDB 11.8 project
        let db = |s: &str| Db::parse(s).unwrap();
        for (n, s) in [
            ("pg", "postgres:16"),
            ("maria", "mariadb:11.8"),      // the project's own
            ("old-maria", "mariadb:10.11"), // same type, another version
            ("pg2", "postgres:16"),
            ("lite", "sqlite"),
        ] {
            std::fs::create_dir_all(dir(&c, n)).unwrap();
            write_marker(&c, n, "8.4", &db(s)).unwrap();
        }
        // A site served before there was a choice has no db= line; one from
        // before versions says only the type.
        std::fs::create_dir_all(dir(&c, "old")).unwrap();
        std::fs::write(marker(&c, "old"), "php=8.4\n").unwrap();
        std::fs::create_dir_all(dir(&c, "typed")).unwrap();
        std::fs::write(marker(&c, "typed"), "php=8.4\ndb=postgres\n").unwrap();

        assert_eq!(super::db(&c, "pg"), db("postgres:16"));
        assert_eq!(super::db(&c, "old"), db("mariadb:11.8"));
        assert_eq!(super::db(&c, "typed"), db("postgres:18"));
        assert_eq!(super::db(&c, "@primary"), db("mariadb:11.8"));
        assert_eq!(php_version(&c, "pg"), "8.4");
        let extra: Vec<String> = extra_dbs(&c).iter().map(Db::name).collect();
        assert_eq!(extra, ["mariadb:10.11", "postgres:16", "postgres:18"]);

        // A database `unserve` kept keeps its server, found by the saved file.
        std::fs::remove_file(marker(&c, "pg")).unwrap();
        std::fs::remove_file(marker(&c, "pg2")).unwrap();
        std::fs::write(
            c.instances_dir().join(".pg.postgres-16.settings.php"),
            "<?php return [];",
        )
        .unwrap();
        let extra: Vec<String> = extra_dbs(&c).iter().map(Db::name).collect();
        assert_eq!(extra, ["mariadb:10.11", "postgres:16", "postgres:18"]);
    }

    #[test]
    fn served_state_and_php_come_from_the_marker() {
        let d = tempfile::tempdir().unwrap();
        let c = ctx(d.path());
        assert!(is_served(&c, "@primary"));
        assert!(!is_served(&c, "missing"));
        for (n, m) in [("b", "php=8.2\n"), ("a", "db=x\n")] {
            std::fs::create_dir_all(dir(&c, n)).unwrap();
            std::fs::write(marker(&c, n), m).unwrap();
        }
        std::fs::create_dir_all(dir(&c, "unmarked")).unwrap();
        std::fs::create_dir_all(c.instance_dir()).unwrap();
        std::fs::write(c.instance_dir().join(".tryout-site"), "").unwrap();
        assert_eq!(served_names(&c), ["a", "b"]);
        assert_eq!(php_version(&c, "b"), "8.2");
        assert_eq!(php_version(&c, "a"), "8.5");
        assert_eq!(php_version(&c, "missing"), "8.5");
    }

    #[test]
    fn the_active_worktree_name_means_the_primary() {
        let d = core_repo();
        let c = ctx(d.path());
        assert_eq!(for_name(&c, "main"), "@primary");
        assert_eq!(for_name(&c, "other"), "other");
        assert_eq!(for_name(&c, ""), "");
    }
}
