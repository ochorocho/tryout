//! The databases, from inside the web container. The project's own server is
//! DDEV's `db` service (MariaDB or MySQL with root/root, or Postgres with db/db,
//! a superuser there). A served site may run on another type instead: a server
//! tryout adds while a site uses it, with the same credentials — or SQLite, a
//! file in the site's own var/, with no server at all.

use std::path::PathBuf;
use std::process::Output;

use super::ctx::Ctx;
use super::out;
use super::{Failed, Step, proc, site};

/// A database type a site can run on, each at one version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Engine {
    Mariadb,
    Mysql,
    Postgres,
    Sqlite,
}

impl Engine {
    pub const ALL: [Engine; 4] = [
        Engine::Mariadb,
        Engine::Mysql,
        Engine::Postgres,
        Engine::Sqlite,
    ];

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.name() == s)
    }

    /// As `--db` takes it and the marker records it.
    pub fn name(self) -> &'static str {
        match self {
            Engine::Mariadb => "mariadb",
            Engine::Mysql => "mysql",
            Engine::Postgres => "postgres",
            Engine::Sqlite => "sqlite",
        }
    }

    /// As people call it.
    pub fn label(self) -> &'static str {
        match self {
            Engine::Mariadb => "MariaDB",
            Engine::Mysql => "MySQL",
            Engine::Postgres => "PostgreSQL",
            Engine::Sqlite => "SQLite",
        }
    }

    /// The type of DDEV's `db` service.
    pub fn of_project(ctx: &Ctx) -> Self {
        if ctx.env.is_postgres() {
            Engine::Postgres
        } else if ctx.env.database.starts_with("mysql") {
            Engine::Mysql
        } else {
            Engine::Mariadb
        }
    }

    /// Whether it talks MySQL's protocol and SQL — the same client and
    /// statements serve both.
    fn is_mysql_family(self) -> bool {
        matches!(self, Engine::Mariadb | Engine::Mysql)
    }

    /// The image of the extra server that provides it; SQLite needs none.
    pub fn image(self) -> Option<&'static str> {
        match self {
            Engine::Mariadb => Some("mariadb:11.8"),
            Engine::Mysql => Some("mysql:8.0"),
            Engine::Postgres => Some("postgres:17"),
            Engine::Sqlite => None,
        }
    }

    /// The extra server's service name, which is also its hostname.
    pub fn service(self) -> Option<&'static str> {
        match self {
            Engine::Mariadb => Some("tryout-mariadb"),
            Engine::Mysql => Some("tryout-mysql"),
            Engine::Postgres => Some("tryout-postgres"),
            Engine::Sqlite => None,
        }
    }

    /// Whether a site on it needs a server besides the project's: an extra
    /// service, and the restart that starts it.
    pub fn needs_service(self, ctx: &Ctx) -> bool {
        self.service().is_some() && self != Engine::of_project(ctx)
    }

    pub fn port(self) -> &'static str {
        match self {
            Engine::Postgres => "5432",
            _ => "3306",
        }
    }

    /// The driver name `typo3 setup` takes.
    pub fn setup_driver(self) -> &'static str {
        match self {
            Engine::Mariadb | Engine::Mysql => "mysqli",
            Engine::Postgres => "postgres",
            Engine::Sqlite => "sqlite",
        }
    }

    /// What choosing it means, in a few words — for completion and the TUI.
    pub fn what(self, ctx: &Ctx) -> String {
        match self.image() {
            _ if self == Engine::of_project(ctx) => "the project's own server".into(),
            None => "a file in the site, no server".into(),
            Some(image) => format!("its own {image} server, started for it"),
        }
    }

    /// Where its server answers: DDEV's `db` for the project's type, the extra
    /// service otherwise ("" for SQLite, which has none).
    pub fn host(self, ctx: &Ctx) -> &'static str {
        if self == Engine::of_project(ctx) {
            "db"
        } else {
            self.service().unwrap_or("")
        }
    }
}

/// Where a site's SQLite database lives: TYPO3 keeps it in the site's var/.
pub fn sqlite_dir(ctx: &Ctx, name: &str) -> PathBuf {
    site::dir(ctx, name).join("var/sqlite")
}

/// The site's SQLite file, when there is one with something in it.
fn sqlite_file(ctx: &Ctx, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(sqlite_dir(ctx, name))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.extension().is_some_and(|x| x == "sqlite")
                && std::fs::metadata(p).is_ok_and(|m| m.len() > 0)
        })
}

/// One statement as the superuser of the site's server, against its default
/// database — right for CREATE DATABASE, wrong for a site's own tables.
pub fn root_sql(ctx: &Ctx, site_name: &str, sql: &str) -> Option<Output> {
    server_sql(ctx, site::db_engine(ctx, site_name), sql)
}

/// One statement as the superuser of a type's server. None for SQLite.
fn server_sql(ctx: &Ctx, engine: Engine, sql: &str) -> Option<Output> {
    let host = engine.host(ctx);
    match engine {
        Engine::Postgres => psql(host, "postgres", sql),
        Engine::Mariadb | Engine::Mysql => {
            proc::capture("mysql", &["-h", host, "-uroot", "-proot", "-e", sql], None)
        }
        Engine::Sqlite => None,
    }
}

/// Wait for a type's server to answer: an extra service starts with DDEV but
/// needs a moment (a first start initialises its data directory).
pub fn wait_until_ready(ctx: &Ctx, engine: Engine) -> Step {
    if engine == Engine::Sqlite {
        return Ok(());
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        if ok(server_sql(ctx, engine, "SELECT 1")) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            out::error(format!(
                "The {} server ({}) does not answer",
                engine.label(),
                engine.host(ctx)
            ));
            out::error(format!(
                "  → ddev tryout worktree serve <name> --db {}   adds it and restarts DDEV",
                engine.name()
            ));
            return Err(Failed);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// One statement against the site's own database. For SQLite through the
/// `sqlite3` CLI, None where the image has none.
pub fn site_sql(ctx: &Ctx, site_name: &str, sql: &str) -> Option<Output> {
    let engine = site::db_engine(ctx, site_name);
    let (host, db) = (engine.host(ctx), site::database(site_name));
    match engine {
        Engine::Postgres => psql(host, &db, sql),
        Engine::Mariadb | Engine::Mysql => proc::capture(
            "mysql",
            &["-h", host, "-uroot", "-proot", "-D", &db, "-e", sql],
            None,
        ),
        Engine::Sqlite => {
            let file = sqlite_file(ctx, site_name)?;
            proc::capture("sqlite3", &[&file.to_string_lossy(), sql], None)
        }
    }
}

fn psql(host: &str, db: &str, sql: &str) -> Option<Output> {
    std::process::Command::new("psql")
        .env("PGPASSWORD", "db")
        .args(["-h", host, "-U", "db", "-d", db, "-tAc", sql])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
}

/// Did the statement succeed? Its output is passed through.
fn shown(o: Option<Output>) -> bool {
    match o {
        Some(o) => {
            let _ = std::io::Write::write_all(&mut std::io::stdout(), &o.stdout);
            let _ = std::io::Write::write_all(&mut std::io::stderr(), &o.stderr);
            o.status.success()
        }
        None => false,
    }
}

fn ok(o: Option<Output>) -> bool {
    o.is_some_and(|o| o.status.success())
}

/// Create a served site's database and grant the DDEV user access to it. A
/// SQLite one is created by TYPO3's setup itself.
pub fn ensure_site_database(ctx: &Ctx, name: &str) -> Step {
    let db = site::database(name);
    let engine = site::db_engine(ctx, name);
    if db == "db" || engine == Engine::Sqlite {
        return Ok(());
    }
    out::info(format!("Ensuring database {db} ({})...", engine.label()));
    let created = if engine == Engine::Postgres {
        let exists = root_sql(
            ctx,
            name,
            &format!("SELECT 1 FROM pg_database WHERE datname='{db}'"),
        )
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains('1'));
        exists || ok(root_sql(ctx, name, &format!("CREATE DATABASE \"{db}\"")))
    } else {
        shown(root_sql(
            ctx,
            name,
            &format!("CREATE DATABASE IF NOT EXISTS `{db}`; GRANT ALL ON `{db}`.* TO 'db'@'%';"),
        ))
    };
    if created {
        Ok(())
    } else {
        out::error(format!("Failed to create database {db}"));
        if engine.needs_service(ctx) {
            out::error(format!(
                "  → is the {} server running? ddev restart",
                engine.host(ctx)
            ));
        }
        Err(Failed)
    }
}

/// Does the site's database already hold a TYPO3 install?
pub fn has_tables(ctx: &Ctx, name: &str) -> bool {
    let db = site::database(name);
    let engine = site::db_engine(ctx, name);
    // Postgres answers per database, so ask the site's own; MySQL answers for
    // every schema from anywhere; SQLite is a file that is there or is not.
    let answer = match engine {
        Engine::Sqlite => return sqlite_file(ctx, name).is_some(),
        Engine::Postgres => site_sql(
            ctx,
            name,
            "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'",
        ),
        Engine::Mariadb | Engine::Mysql => root_sql(
            ctx,
            name,
            &format!("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='{db}';"),
        ),
    };
    let count: String = answer
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .chars()
                .filter(char::is_ascii_digit)
                .collect()
        })
        .unwrap_or_default();
    count.parse::<u64>().is_ok_and(|n| n > 0)
}

/// Drop a site's database; false when the server refused.
#[must_use]
pub fn drop(ctx: &Ctx, name: &str) -> bool {
    let db = site::database(name);
    match site::db_engine(ctx, name) {
        Engine::Sqlite => {
            let dir = sqlite_dir(ctx, name);
            !dir.exists() || std::fs::remove_dir_all(dir).is_ok()
        }
        Engine::Postgres => shown(root_sql(
            ctx,
            name,
            &format!("DROP DATABASE IF EXISTS \"{db}\""),
        )),
        Engine::Mariadb | Engine::Mysql => shown(root_sql(
            ctx,
            name,
            &format!("DROP DATABASE IF EXISTS `{db}`;"),
        )),
    }
}

/// Drop and recreate a site's database, re-granting what DROP took with it.
pub fn recreate(ctx: &Ctx, name: &str) -> bool {
    let db = site::database(name);
    let engine = site::db_engine(ctx, name);
    match engine {
        // The setup that follows creates a new file.
        Engine::Sqlite => drop(ctx, name),
        Engine::Postgres => {
            let _ = root_sql(ctx, name, &format!("DROP DATABASE IF EXISTS \"{db}\""));
            ok(root_sql(ctx, name, &format!("CREATE DATABASE \"{db}\"")))
        }
        _ => {
            debug_assert!(engine.is_mysql_family());
            shown(root_sql(
                ctx,
                name,
                &format!(
                    "DROP DATABASE IF EXISTS `{db}`; CREATE DATABASE `{db}`; GRANT ALL ON `{db}`.* TO 'db'@'%';"
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ctx::DdevEnv;

    fn ctx(db: &str) -> Ctx {
        Ctx::new(
            std::path::Path::new("/p"),
            DdevEnv {
                database: db.into(),
                ..DdevEnv::default()
            },
        )
    }

    #[test]
    fn the_projects_type_is_on_db_and_every_other_server_on_its_own_service() {
        let maria = ctx("mariadb:11.8");
        assert_eq!(Engine::of_project(&maria), Engine::Mariadb);
        assert_eq!(Engine::Mariadb.host(&maria), "db");
        assert_eq!(Engine::Mysql.host(&maria), "tryout-mysql");
        assert_eq!(Engine::Postgres.host(&maria), "tryout-postgres");
        let pg = ctx("postgres:16");
        assert_eq!(Engine::Postgres.host(&pg), "db");
        assert_eq!(Engine::Mariadb.host(&pg), "tryout-mariadb");
        let my = ctx("mysql:8.0");
        assert_eq!(Engine::of_project(&my), Engine::Mysql);
        assert_eq!(Engine::Mysql.host(&my), "db");
        assert!(Engine::Mariadb.needs_service(&my));
        assert!(!Engine::Mysql.needs_service(&my));
    }

    #[test]
    fn sqlite_is_a_file_with_no_server() {
        let maria = ctx("mariadb:11.8");
        assert_eq!(Engine::Sqlite.service(), None);
        assert_eq!(Engine::Sqlite.image(), None);
        assert!(!Engine::Sqlite.needs_service(&maria));
        assert_eq!(Engine::Sqlite.setup_driver(), "sqlite");
        assert_eq!(
            sqlite_dir(&maria, "lite"),
            PathBuf::from("/p/TYPO3-Instances/lite/var/sqlite")
        );
    }

    #[test]
    fn every_type_parses_by_its_name_and_nothing_else_does() {
        for e in Engine::ALL {
            assert_eq!(Engine::parse(e.name()), Some(e));
        }
        assert_eq!(Engine::parse("MySQL"), None);
        assert_eq!(Engine::parse("oracle"), None);
        assert_eq!(Engine::parse(""), None);
    }
}
