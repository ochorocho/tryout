//! The databases, from inside the web container. The project's own server is
//! DDEV's `db` service (MariaDB or MySQL with root/root, or Postgres with db/db,
//! a superuser there). A served site may run on another type or version: a
//! server tryout adds while a site uses it, with the same credentials — or
//! SQLite, a file in the site's own var/, with no server at all.

use std::path::PathBuf;
use std::process::Output;

use super::ctx::Ctx;
use super::out;
use super::{Failed, Step, proc, site};

/// A database type: what decides the driver, the port and the SQL.
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

    /// The versions a site can pick, newest first; the first is the default.
    /// A curated part of what DDEV supports: versions a TYPO3 of today runs on
    /// and the web image's clients can talk to — MySQL 9 is out, its server no
    /// longer lets the MariaDB client log in. SQLite is what PHP brings.
    pub fn versions(self) -> &'static [&'static str] {
        match self {
            Engine::Mariadb => &["11.8", "11.4", "10.11", "10.6"],
            Engine::Mysql => &["8.4", "8.0"],
            Engine::Postgres => &["18", "17", "16", "15", "14"],
            Engine::Sqlite => &[""],
        }
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
}

/// A database server: a type at a version (SQLite has none), written the way
/// DDEV's own `--database` takes it — `postgres:16`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Db {
    pub engine: Engine,
    pub version: String,
}

impl Db {
    fn new(engine: Engine, version: &str) -> Self {
        Self {
            engine,
            version: if engine == Engine::Sqlite {
                String::new()
            } else {
                version.to_string()
            },
        }
    }

    /// Every type and version a site can pick, in picker order.
    pub fn choices() -> Vec<Db> {
        Engine::ALL
            .into_iter()
            .flat_map(|e| e.versions().iter().map(move |v| Db::new(e, v)))
            .collect()
    }

    /// `postgres:16`, or a bare `postgres` for its default (newest) version.
    /// Only what `choices` offers.
    pub fn parse(s: &str) -> Option<Self> {
        let (name, version) = s.split_once(':').unwrap_or((s, ""));
        let engine = Engine::parse(name)?;
        let version = if version.is_empty() {
            engine.versions()[0]
        } else {
            version
        };
        engine
            .versions()
            .contains(&version)
            .then(|| Db::new(engine, version))
    }

    /// What a marker recorded: any `type:version`, offered or not — a site on
    /// the project's own server carries the project's version, whatever it is;
    /// a bare type (an earlier marker) is its default version.
    pub fn parse_recorded(s: &str) -> Option<Self> {
        let (name, version) = s.split_once(':').unwrap_or((s, ""));
        let engine = Engine::parse(name)?;
        let version = if version.is_empty() {
            engine.versions()[0]
        } else {
            version
        };
        Some(Db::new(engine, version))
    }

    /// DDEV's `db` service, as `DDEV_DATABASE` says (`mariadb:11.8`) — any
    /// version DDEV runs, not only the ones offered for a site.
    pub fn of_project(ctx: &Ctx) -> Self {
        let (name, version) = ctx
            .env
            .database
            .split_once(':')
            .unwrap_or((ctx.env.database.as_str(), ""));
        match Engine::parse(name) {
            Some(e) if e != Engine::Sqlite => Db::new(e, version),
            // Not said (DDEV's default) or not a type tryout knows.
            _ => Db::new(Engine::Mariadb, "11.8"),
        }
    }

    /// As `--db` takes it and the marker records it: `postgres:16`, `sqlite`.
    pub fn name(&self) -> String {
        if self.version.is_empty() {
            self.engine.name().to_string()
        } else {
            format!("{}:{}", self.engine.name(), self.version)
        }
    }

    /// As people call it: `PostgreSQL 16`, `SQLite`.
    pub fn label(&self) -> String {
        if self.version.is_empty() {
            self.engine.label().to_string()
        } else {
            format!("{} {}", self.engine.label(), self.version)
        }
    }

    /// For file names: `postgres-16`, `mariadb-10.11`, `sqlite`.
    pub fn slug(&self) -> String {
        self.name().replace(':', "-")
    }

    /// The image of the extra server that provides it; SQLite needs none.
    pub fn image(&self) -> Option<String> {
        (self.engine != Engine::Sqlite).then(|| self.name())
    }

    /// The extra server's service name, which is also its hostname:
    /// `tryout-postgres-16`, `tryout-mariadb-10-11` (no dots in a hostname).
    pub fn service(&self) -> Option<String> {
        (self.engine != Engine::Sqlite).then(|| format!("tryout-{}", self.slug().replace('.', "-")))
    }

    /// Whether a site on it needs a server besides the project's: an extra
    /// service, and the restart that starts it.
    pub fn needs_service(&self, ctx: &Ctx) -> bool {
        self.engine != Engine::Sqlite && *self != Db::of_project(ctx)
    }

    /// Where its server answers: DDEV's `db` for the project's own, the extra
    /// service otherwise ("" for SQLite, which has none).
    pub fn host(&self, ctx: &Ctx) -> String {
        if *self == Db::of_project(ctx) {
            "db".into()
        } else {
            self.service().unwrap_or_default()
        }
    }

    /// Where the image keeps its data: Postgres 18 moved it up a level.
    pub fn data_dir(&self) -> &'static str {
        match self.engine {
            Engine::Postgres if self.version.parse::<u32>().is_ok_and(|v| v >= 18) => {
                "/var/lib/postgresql"
            }
            Engine::Postgres => "/var/lib/postgresql/data",
            _ => "/var/lib/mysql",
        }
    }

    /// What TYPO3 itself says about it, where that is less than "supported".
    /// get.typo3.org lists MariaDB up to 10.x for 12.4 to 14.3 — yet 11.x is
    /// DDEV's default and runs them, so this is a note, never a refusal.
    pub fn typo3_note(&self) -> Option<&'static str> {
        let major: u32 = self.version.split('.').next()?.parse().ok()?;
        (self.engine == Engine::Mariadb && major >= 11).then_some("TYPO3 lists MariaDB up to 10.x")
    }

    /// What choosing it means, in a few words — for completion and the TUI.
    pub fn what(&self, ctx: &Ctx) -> String {
        let what = if *self == Db::of_project(ctx) {
            "the project's own server".to_string()
        } else if self.engine == Engine::Sqlite {
            "a file in the site, no server".to_string()
        } else {
            "its own server, started for it".to_string()
        };
        // What TYPO3 supports matters only where TYPO3 is what runs.
        match self.typo3_note() {
            Some(note) if ctx.mode() == super::kind::Mode::Core => format!("{what} · {note}"),
            _ => what,
        }
    }
}

/// Where a site's SQLite database lives: TYPO3 keeps it in the site's var/;
/// a project site's waits in its state, out of the user's worktree.
pub fn sqlite_dir(ctx: &Ctx, name: &str) -> PathBuf {
    match ctx.mode() {
        super::kind::Mode::Core => site::dir(ctx, name).join("var/sqlite"),
        super::kind::Mode::Project => site::state_dir(ctx, name).join("sqlite"),
    }
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
    server_sql(ctx, &site::db(ctx, site_name), sql)
}

/// One statement as the superuser of a server. None for SQLite.
fn server_sql(ctx: &Ctx, db: &Db, sql: &str) -> Option<Output> {
    let host = db.host(ctx);
    match db.engine {
        Engine::Postgres => psql(&host, "postgres", sql),
        Engine::Mariadb | Engine::Mysql => {
            proc::capture("mysql", &["-h", &host, "-uroot", "-proot", "-e", sql], None)
        }
        Engine::Sqlite => None,
    }
}

/// Wait for a server to answer: an extra service starts with DDEV but needs a
/// moment (a first start initialises its data directory).
pub fn wait_until_ready(ctx: &Ctx, db: &Db) -> Step {
    if db.engine == Engine::Sqlite {
        return Ok(());
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        if ok(server_sql(ctx, db, "SELECT 1")) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            out::error(format!(
                "The {} server ({}) does not answer",
                db.label(),
                db.host(ctx)
            ));
            out::error(format!(
                "  → ddev tryout worktree serve <name> --db {}   adds it and restarts DDEV",
                db.name()
            ));
            return Err(Failed);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// One statement against the site's own database. For SQLite through the
/// `sqlite3` CLI, None where the image has none.
pub fn site_sql(ctx: &Ctx, site_name: &str, sql: &str) -> Option<Output> {
    let db = site::db(ctx, site_name);
    let (host, name) = (db.host(ctx), site::database(site_name));
    match db.engine {
        Engine::Postgres => psql(&host, &name, sql),
        Engine::Mariadb | Engine::Mysql => proc::capture(
            "mysql",
            // Rows only: no header, tab-separated.
            &[
                "-h", &host, "-uroot", "-proot", "-N", "-B", "-D", &name, "-e", sql,
            ],
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
/// SQLite one is created by whoever opens it first (TYPO3's setup, the app);
/// only its directory is made here.
pub fn ensure_site_database(ctx: &Ctx, name: &str) -> Step {
    let dbname = site::database(name);
    let db = site::db(ctx, name);
    if db.engine == Engine::Sqlite {
        // A project's app opens the file where it is told; TYPO3's setup makes
        // its own directory — and one made here would block restoring a kept one.
        if ctx.mode() == super::kind::Mode::Project {
            let _ = std::fs::create_dir_all(sqlite_dir(ctx, name));
        }
        return Ok(());
    }
    if dbname == "db" {
        return Ok(());
    }
    out::info(format!("Ensuring database {dbname} ({})...", db.label()));
    let created = if db.engine == Engine::Postgres {
        let exists = root_sql(
            ctx,
            name,
            &format!("SELECT 1 FROM pg_database WHERE datname='{dbname}'"),
        )
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains('1'));
        exists
            || ok(root_sql(
                ctx,
                name,
                &format!("CREATE DATABASE \"{dbname}\""),
            ))
    } else {
        shown(root_sql(
            ctx,
            name,
            &format!(
                "CREATE DATABASE IF NOT EXISTS `{dbname}`; GRANT ALL ON `{dbname}`.* TO 'db'@'%';"
            ),
        ))
    };
    if created {
        Ok(())
    } else {
        out::error(format!("Failed to create database {dbname}"));
        if db.needs_service(ctx) {
            out::error(format!(
                "  → is the {} server running? ddev restart",
                db.host(ctx)
            ));
        }
        Err(Failed)
    }
}

/// Does the site's database already hold a TYPO3 install?
pub fn has_tables(ctx: &Ctx, name: &str) -> bool {
    let dbname = site::database(name);
    // Postgres answers per database, so ask the site's own; MySQL answers for
    // every schema from anywhere; SQLite is a file that is there or is not.
    let answer = match site::db(ctx, name).engine {
        Engine::Sqlite => return sqlite_file(ctx, name).is_some(),
        Engine::Postgres => site_sql(
            ctx,
            name,
            "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'",
        ),
        Engine::Mariadb | Engine::Mysql => root_sql(
            ctx,
            name,
            &format!(
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='{dbname}';"
            ),
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
    let dbname = site::database(name);
    match site::db(ctx, name).engine {
        Engine::Sqlite => {
            let dir = sqlite_dir(ctx, name);
            !dir.exists() || std::fs::remove_dir_all(dir).is_ok()
        }
        Engine::Postgres => shown(root_sql(
            ctx,
            name,
            &format!("DROP DATABASE IF EXISTS \"{dbname}\""),
        )),
        Engine::Mariadb | Engine::Mysql => shown(root_sql(
            ctx,
            name,
            &format!("DROP DATABASE IF EXISTS `{dbname}`;"),
        )),
    }
}

/// Where a new site's database starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seed {
    /// A project site: a copy of the primary's. A Core site: TYPO3's setup.
    Default,
    /// Nothing: the app sets itself up.
    Empty,
    /// A copy of this database.
    Copy { db: Db, name: String },
}

impl Seed {
    /// A copy of a site's database (the primary's: the project's own).
    pub fn of_site(ctx: &Ctx, name: &str) -> Seed {
        Seed::Copy {
            db: site::db(ctx, name),
            name: site::database(name),
        }
    }
}

/// Server engines whose dumps load into each other.
fn family(e: Engine) -> Option<u8> {
    match e {
        Engine::Mariadb | Engine::Mysql => Some(1),
        Engine::Postgres => Some(2),
        Engine::Sqlite => None,
    }
}

/// The shell pipeline that copies database `from` into `to`, run in the web
/// container: the source server's dump loaded by the target's client. None
/// across engine families (and for SQLite), where no dump loads as it is.
pub fn copy_pipeline(from: (&Db, &str, &str), to: (&Db, &str, &str)) -> Option<String> {
    let ((from_db, from_host, from_name), (to_db, to_host, to_name)) = (from, to);
    if family(from_db.engine)? != family(to_db.engine)? {
        return None;
    }
    // Everything here goes into a shell line: names and hosts are identifiers.
    let safe = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    };
    if ![from_host, from_name, to_host, to_name]
        .iter()
        .all(|s| safe(s))
    {
        return None;
    }
    Some(match to_db.engine {
        Engine::Postgres => format!(
            "PGPASSWORD=db pg_dump -h {from_host} -U db --no-owner --no-acl {from_name} \
             | PGPASSWORD=db psql -q -v ON_ERROR_STOP=1 -h {to_host} -U db -d {to_name}"
        ),
        _ => format!(
            "mysqldump -h {from_host} -uroot -proot --single-transaction --routines --triggers --no-tablespaces {from_name} \
             | mysql -h {to_host} -uroot -proot {to_name}"
        ),
    })
}

/// Fill a site's fresh database from `seed`. False when it could not be
/// copied: the site then starts empty, and says so.
pub fn copy_into(ctx: &Ctx, from: &Db, from_name: &str, site_name: &str) -> bool {
    let to = site::db(ctx, site_name);
    let (from_host, to_host, to_name) = (from.host(ctx), to.host(ctx), site::database(site_name));
    let Some(line) = copy_pipeline((from, &from_host, from_name), (&to, &to_host, &to_name)) else {
        out::warn(format!(
            "No copy from {} into {} — '{site_name}' starts with an empty database",
            from.label(),
            to.label()
        ));
        return false;
    };
    out::info(format!("Copying database {from_name} into {to_name}..."));
    let copied = proc::run("bash", &["-o", "pipefail", "-c", &line], None);
    if !copied {
        out::warn(format!(
            "Copying {from_name} failed — '{site_name}' starts with what arrived"
        ));
    }
    copied
}

/// Drop and recreate a site's database, re-granting what DROP took with it.
pub fn recreate(ctx: &Ctx, name: &str) -> bool {
    let dbname = site::database(name);
    match site::db(ctx, name).engine {
        // The setup that follows creates a new file.
        Engine::Sqlite => drop(ctx, name),
        Engine::Postgres => {
            let _ = root_sql(ctx, name, &format!("DROP DATABASE IF EXISTS \"{dbname}\""));
            ok(root_sql(
                ctx,
                name,
                &format!("CREATE DATABASE \"{dbname}\""),
            ))
        }
        Engine::Mariadb | Engine::Mysql => shown(root_sql(
            ctx,
            name,
            &format!(
                "DROP DATABASE IF EXISTS `{dbname}`; CREATE DATABASE `{dbname}`; GRANT ALL ON `{dbname}`.* TO 'db'@'%';"
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typo3_sqlite_site_is_left_for_its_setup_to_make_and_a_kept_file_can_return() {
        use crate::core::ctx::{DdevEnv, tests::core_repo};
        let d = core_repo();
        let c = Ctx::new(d.path(), DdevEnv::default());
        site::write_marker(&c, "lite", "8.4", &Db::parse("sqlite").unwrap()).unwrap();
        ensure_site_database(&c, "lite").unwrap();
        // An empty directory here would stand where the kept database goes back.
        assert!(!sqlite_dir(&c, "lite").exists());

        let p = crate::core::ctx::tests::project_repo();
        let c = Ctx::new(p.path(), DdevEnv::default());
        site::write_marker(&c, "lite", "8.4", &Db::parse("sqlite").unwrap()).unwrap();
        ensure_site_database(&c, "lite").unwrap();
        // A project's app opens the file where it is told: the directory is there.
        assert!(sqlite_dir(&c, "lite").is_dir());
    }

    #[test]
    fn a_database_is_copied_by_its_own_servers_tools_and_only_within_a_family() {
        let db = |s: &str| Db::parse_recorded(s).unwrap();
        let (maria, pg, lite) = (db("mariadb:11.8"), db("postgres:16"), db("sqlite"));
        assert_eq!(
            copy_pipeline(
                (&maria, "db", "db"),
                (&db("mysql:8.4"), "tryout-mysql-8-4", "db_my")
            )
            .as_deref(),
            Some(
                "mysqldump -h db -uroot -proot --single-transaction --routines --triggers \
                 --no-tablespaces db | mysql -h tryout-mysql-8-4 -uroot -proot db_my"
            )
        );
        assert_eq!(
            copy_pipeline(
                (&pg, "db", "db"),
                (&db("postgres:17"), "tryout-postgres-17", "db_pg")
            )
            .as_deref(),
            Some(
                "PGPASSWORD=db pg_dump -h db -U db --no-owner --no-acl db \
                 | PGPASSWORD=db psql -q -v ON_ERROR_STOP=1 -h tryout-postgres-17 -U db -d db_pg"
            )
        );
        // No dump of one family loads into the other; SQLite has no server.
        assert_eq!(
            copy_pipeline((&maria, "db", "db"), (&pg, "h", "db_x")),
            None
        );
        assert_eq!(
            copy_pipeline((&maria, "db", "db"), (&lite, "", "db_x")),
            None
        );
        // Nothing that is not an identifier reaches the shell line.
        assert_eq!(
            copy_pipeline((&maria, "db", "db; rm -rf /"), (&maria, "db", "db_x")),
            None
        );
    }
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

    fn db(s: &str) -> Db {
        Db::parse(s).unwrap_or_else(|| panic!("{s} does not parse"))
    }

    #[test]
    fn the_projects_own_server_is_db_and_every_other_its_own_service() {
        let maria = ctx("mariadb:11.8");
        assert_eq!(Db::of_project(&maria), db("mariadb:11.8"));
        assert_eq!(db("mariadb:11.8").host(&maria), "db");
        // The same type at another version is another server.
        assert_eq!(db("mariadb:10.11").host(&maria), "tryout-mariadb-10-11");
        assert_eq!(db("mysql:8.0").host(&maria), "tryout-mysql-8-0");
        assert_eq!(db("postgres:16").host(&maria), "tryout-postgres-16");
        let pg = ctx("postgres:16");
        assert_eq!(db("postgres:16").host(&pg), "db");
        assert_eq!(db("postgres:17").host(&pg), "tryout-postgres-17");
        // DDEV's own version need not be one offered for a site.
        let old = ctx("mariadb:10.4");
        assert_eq!(Db::of_project(&old).name(), "mariadb:10.4");
        assert!(db("mariadb:10.11").needs_service(&old));
        // Not said at all: DDEV's default.
        assert_eq!(Db::of_project(&ctx("")).name(), "mariadb:11.8");
    }

    #[test]
    fn a_type_alone_means_its_newest_version_and_only_offered_ones_parse() {
        assert_eq!(db("postgres").name(), "postgres:18");
        assert_eq!(db("mariadb").name(), "mariadb:11.8");
        assert_eq!(db("sqlite").name(), "sqlite");
        for bad in [
            "postgres:9",
            "mysql:9.7",
            "mariadb:12.3",
            "oracle",
            "",
            "sqlite:3",
        ] {
            assert_eq!(Db::parse(bad), None, "{bad}");
        }
        for c in Db::choices() {
            assert_eq!(Db::parse(&c.name()), Some(c.clone()), "{}", c.name());
        }
    }

    #[test]
    fn names_labels_and_data_dirs() {
        let d = db("mariadb:10.11");
        assert_eq!(
            (d.label(), d.slug(), d.service(), d.image()),
            (
                "MariaDB 10.11".to_string(),
                "mariadb-10.11".to_string(),
                Some("tryout-mariadb-10-11".to_string()),
                Some("mariadb:10.11".to_string())
            )
        );
        assert_eq!(db("postgres:18").data_dir(), "/var/lib/postgresql");
        assert_eq!(db("postgres:17").data_dir(), "/var/lib/postgresql/data");
        assert_eq!(db("mysql:8.4").data_dir(), "/var/lib/mysql");
        let lite = db("sqlite");
        assert_eq!(
            (lite.service(), lite.image(), lite.label()),
            (None, None, "SQLite".into())
        );
        assert!(!lite.needs_service(&ctx("mariadb:11.8")));
    }

    #[test]
    fn typo3s_own_word_is_a_note_not_a_refusal() {
        assert!(db("mariadb:11.8").typo3_note().is_some());
        assert!(db("mariadb:10.11").typo3_note().is_none());
        assert!(db("postgres:18").typo3_note().is_none());
        let what = db("mariadb:11.4").what(&ctx("postgres:16"));
        assert!(
            what.contains("its own server") && what.contains("up to 10.x"),
            "{what}"
        );
    }
}
