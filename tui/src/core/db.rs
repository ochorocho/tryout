//! The database service, from inside the web container: DDEV's root/root
//! (MariaDB, MySQL) and db/db (Postgres, a superuser there), reached as `db`.

use std::process::Output;

use super::ctx::Ctx;
use super::out;
use super::{Failed, Step, proc, site};

/// One statement as the superuser, against the server's default database — right
/// for CREATE DATABASE, wrong for a site's own tables.
pub fn root_sql(ctx: &Ctx, sql: &str) -> Option<Output> {
    if ctx.env.is_postgres() {
        psql(ctx, "postgres", sql)
    } else {
        proc::capture("mysql", &["-h", "db", "-uroot", "-proot", "-e", sql], None)
    }
}

/// One statement against a named database.
pub fn site_sql(ctx: &Ctx, db: &str, sql: &str) -> Option<Output> {
    if ctx.env.is_postgres() {
        psql(ctx, db, sql)
    } else {
        proc::capture(
            "mysql",
            &["-h", "db", "-uroot", "-proot", "-D", db, "-e", sql],
            None,
        )
    }
}

fn psql(_ctx: &Ctx, db: &str, sql: &str) -> Option<Output> {
    std::process::Command::new("psql")
        .env("PGPASSWORD", "db")
        .args(["-h", "db", "-U", "db", "-d", db, "-tAc", sql])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
}

/// Did the statement succeed? Its output is passed through, as the bash did.
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

/// Create a served site's database and grant the DDEV user access to it.
pub fn ensure_site_database(ctx: &Ctx, name: &str) -> Step {
    let db = site::database(name);
    if db == "db" {
        return Ok(());
    }
    out::info(format!("Ensuring database {db}..."));
    let created = if ctx.env.is_postgres() {
        let exists = root_sql(
            ctx,
            &format!("SELECT 1 FROM pg_database WHERE datname='{db}'"),
        )
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains('1'));
        exists || ok(root_sql(ctx, &format!("CREATE DATABASE \"{db}\"")))
    } else {
        shown(root_sql(
            ctx,
            &format!("CREATE DATABASE IF NOT EXISTS `{db}`; GRANT ALL ON `{db}`.* TO 'db'@'%';"),
        ))
    };
    if created {
        Ok(())
    } else {
        out::error(format!("Failed to create database {db}"));
        Err(Failed)
    }
}

/// Does the site's database already hold a TYPO3 install?
pub fn has_tables(ctx: &Ctx, name: &str) -> bool {
    let db = site::database(name);
    let sql = if ctx.env.is_postgres() {
        "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'".to_string()
    } else {
        format!("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='{db}';")
    };
    let count: String = root_sql(ctx, &sql)
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .chars()
                .filter(char::is_ascii_digit)
                .collect()
        })
        .unwrap_or_default();
    count.parse::<u64>().is_ok_and(|n| n > 0)
}

/// Drop a site's database.
pub fn drop(ctx: &Ctx, db: &str) {
    if ctx.env.is_postgres() {
        let _ = root_sql(ctx, &format!("DROP DATABASE IF EXISTS \"{db}\""));
    } else {
        shown(root_sql(ctx, &format!("DROP DATABASE IF EXISTS `{db}`;")));
    }
}

/// Drop and recreate a site's database, re-granting what DROP took with it.
pub fn recreate(ctx: &Ctx, db: &str) -> bool {
    if ctx.env.is_postgres() {
        let _ = root_sql(ctx, &format!("DROP DATABASE IF EXISTS \"{db}\""));
        ok(root_sql(ctx, &format!("CREATE DATABASE \"{db}\"")))
    } else {
        shown(root_sql(
            ctx,
            &format!(
                "DROP DATABASE IF EXISTS `{db}`; CREATE DATABASE `{db}`; GRANT ALL ON `{db}`.* TO 'db'@'%';"
            ),
        ))
    }
}
