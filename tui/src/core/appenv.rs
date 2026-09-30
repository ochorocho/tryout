//! What a project site's app is told about its database and URL, in the names
//! its framework reads. Real environment variables win over `.env` in every
//! dotenv framework (Laravel, Symfony, Craft, Shopware, CakePHP), so the vhost's
//! variables and `exec`'s environment are the whole wiring — no file written.

use super::db::{Db, Engine};

/// A site's database, as the app reaches it from the web container.
pub struct SiteDb<'a> {
    pub db: &'a Db,
    /// Server hostname; empty for SQLite.
    pub host: &'a str,
    /// Database name, or the SQLite file's path.
    pub name: &'a str,
}

/// Every variable for a site of a DDEV project type: tryout's own
/// `TRYOUT_DB_*` and `DATABASE_URL` always, the framework's names for the
/// types that have their own.
pub fn vars(project_type: &str, site: &SiteDb, url: &str) -> Vec<(&'static str, String)> {
    let (db, sqlite) = (site.db, site.db.engine == Engine::Sqlite);
    let port = db.engine.port().to_string();
    let mut v = vec![
        ("TRYOUT_DB_DRIVER", db.engine.name().to_string()),
        ("TRYOUT_DB_NAME", site.name.to_string()),
    ];
    if !sqlite {
        v.extend([
            ("TRYOUT_DB_HOST", site.host.to_string()),
            ("TRYOUT_DB_PORT", port.clone()),
            ("TRYOUT_DB_USER", "db".to_string()),
            ("TRYOUT_DB_PASSWORD", "db".to_string()),
        ]);
    }
    v.push(("DATABASE_URL", database_url(site)));
    v.push(("TRYOUT_URL", url.to_string()));
    let host_port = format!("{}:{port}", site.host);
    match project_type {
        "laravel" => {
            let connection = match db.engine {
                // `mysql` works for MariaDB on every Laravel; `mariadb` only on 11+.
                Engine::Mariadb | Engine::Mysql => "mysql",
                Engine::Postgres => "pgsql",
                Engine::Sqlite => "sqlite",
            };
            v.extend([
                ("DB_CONNECTION", connection.to_string()),
                ("DB_DATABASE", site.name.to_string()),
            ]);
            if !sqlite {
                v.extend([
                    ("DB_HOST", site.host.to_string()),
                    ("DB_PORT", port),
                    ("DB_USERNAME", "db".to_string()),
                    ("DB_PASSWORD", "db".to_string()),
                ]);
            }
            v.push(("APP_URL", url.to_string()));
        }
        "craftcms" if !sqlite => {
            let driver = if db.engine == Engine::Postgres {
                "pgsql"
            } else {
                "mysql"
            };
            v.extend([
                ("CRAFT_DB_DRIVER", driver.to_string()),
                ("CRAFT_DB_SERVER", site.host.to_string()),
                ("CRAFT_DB_PORT", port),
                ("CRAFT_DB_DATABASE", site.name.to_string()),
                ("CRAFT_DB_USER", "db".to_string()),
                ("CRAFT_DB_PASSWORD", "db".to_string()),
                ("PRIMARY_SITE_URL", url.to_string()),
            ]);
        }
        "shopware6" => v.push(("APP_URL", url.to_string())),
        "cakephp" => v.push(("APP_FULL_BASE_URL", url.to_string())),
        // Laravel's names, without a connection type.
        "asterios" if !sqlite => v.extend([
            ("DB_HOST", site.host.to_string()),
            ("DB_PORT", port.clone()),
            ("DB_DATABASE", site.name.to_string()),
            ("DB_USERNAME", "db".to_string()),
            ("DB_PASSWORD", "db".to_string()),
            ("APP_URL", url.to_string()),
        ]),
        // Bedrock's dotenv never overrides a real variable.
        "wp-bedrock" if !sqlite => v.extend([
            ("DB_NAME", site.name.to_string()),
            ("DB_USER", "db".to_string()),
            ("DB_PASSWORD", "db".to_string()),
            ("DB_HOST", host_port),
            ("WP_HOME", url.to_string()),
            ("WP_SITEURL", format!("{url}/wp")),
        ]),
        "silverstripe" => {
            let class = match db.engine {
                Engine::Postgres => "PostgreSQLDatabase",
                Engine::Sqlite => "SQLite3Database",
                _ => "MySQLDatabase",
            };
            v.extend([
                ("SS_DATABASE_CLASS", class.to_string()),
                ("SS_DATABASE_NAME", site.name.to_string()),
                ("SS_BASE_URL", url.to_string()),
            ]);
            if !sqlite {
                v.extend([
                    ("SS_DATABASE_SERVER", site.host.to_string()),
                    ("SS_DATABASE_PORT", port),
                    ("SS_DATABASE_USERNAME", "db".to_string()),
                    ("SS_DATABASE_PASSWORD", "db".to_string()),
                ]);
            }
        }
        // CodeIgniter 4 reads its config keys, dots and all, from the environment.
        "codeigniter" => {
            let driver = match db.engine {
                Engine::Postgres => "Postgre",
                Engine::Sqlite => "SQLite3",
                _ => "MySQLi",
            };
            v.extend([
                ("database.default.DBDriver", driver.to_string()),
                ("database.default.database", site.name.to_string()),
                ("app.baseURL", format!("{url}/")),
            ]);
            if !sqlite {
                v.extend([
                    ("database.default.hostname", site.host.to_string()),
                    ("database.default.port", port),
                    ("database.default.username", "db".to_string()),
                    ("database.default.password", "db".to_string()),
                ]);
            }
        }
        _ => {}
    }
    v
}

/// The Doctrine-style URL (Symfony, Shopware, CakePHP read it as is), with
/// `serverVersion` so Doctrine need not ask the server. DBAL 4 takes a MariaDB
/// or MySQL version only as major.minor.patch ("mariadb-11.8.0"): a bare
/// "11.8-MariaDB" fails every query with InvalidPlatformVersion.
pub fn database_url(site: &SiteDb) -> String {
    let (db, host, name) = (site.db, site.host, site.name);
    let port = db.engine.port();
    let version = |v: String| {
        if db.version.is_empty() {
            String::new()
        } else {
            format!("serverVersion={v}&")
        }
    };
    let full = || {
        let dots = db.version.matches('.').count();
        format!("{}{}", db.version, ".0".repeat(2usize.saturating_sub(dots)))
    };
    match db.engine {
        Engine::Mariadb => format!(
            "mysql://db:db@{host}:{port}/{name}?{}charset=utf8mb4",
            version(format!("mariadb-{}", full()))
        ),
        Engine::Mysql => format!(
            "mysql://db:db@{host}:{port}/{name}?{}charset=utf8mb4",
            version(full())
        ),
        Engine::Postgres => format!(
            "postgresql://db:db@{host}:{port}/{name}?{}charset=utf8",
            version(db.version.clone())
        ),
        // An absolute path after `sqlite:///`.
        Engine::Sqlite => format!("sqlite:///{name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(v: &'a [(&'static str, String)], k: &str) -> Option<&'a str> {
        v.iter().find(|(n, _)| *n == k).map(|(_, x)| x.as_str())
    }

    #[test]
    fn every_framework_reads_its_own_names_for_the_sites_database() {
        let maria = Db::parse("mariadb:11.8").unwrap();
        let site = SiteDb {
            db: &maria,
            host: "db",
            name: "db_one",
        };
        let url = "https://one.shop.ddev.site";

        let laravel = vars("laravel", &site, url);
        assert_eq!(get(&laravel, "DB_CONNECTION"), Some("mysql"));
        assert_eq!(get(&laravel, "DB_DATABASE"), Some("db_one"));
        assert_eq!(get(&laravel, "DB_HOST"), Some("db"));
        assert_eq!(get(&laravel, "DB_USERNAME"), Some("db"));
        assert_eq!(get(&laravel, "APP_URL"), Some(url));

        let symfony = vars("symfony", &site, url);
        assert_eq!(
            get(&symfony, "DATABASE_URL"),
            Some("mysql://db:db@db:3306/db_one?serverVersion=mariadb-11.8.0&charset=utf8mb4")
        );
        assert_eq!(get(&symfony, "DB_DATABASE"), None);

        let craft = vars("craftcms", &site, url);
        assert_eq!(get(&craft, "CRAFT_DB_DATABASE"), Some("db_one"));
        assert_eq!(get(&craft, "PRIMARY_SITE_URL"), Some(url));

        // Every type gets tryout's own names.
        let php = vars("php", &site, url);
        assert_eq!(get(&php, "TRYOUT_DB_NAME"), Some("db_one"));
        assert_eq!(get(&php, "TRYOUT_URL"), Some(url));
        assert_eq!(get(&php, "APP_URL"), None);

        let bedrock = vars("wp-bedrock", &site, url);
        assert_eq!(get(&bedrock, "DB_HOST"), Some("db:3306"));
        assert_eq!(
            get(&bedrock, "WP_SITEURL"),
            Some("https://one.shop.ddev.site/wp")
        );

        let ss = vars("silverstripe", &site, url);
        assert_eq!(get(&ss, "SS_DATABASE_CLASS"), Some("MySQLDatabase"));
        assert_eq!(get(&ss, "SS_DATABASE_NAME"), Some("db_one"));

        let ci = vars("codeigniter", &site, url);
        assert_eq!(get(&ci, "database.default.database"), Some("db_one"));
        assert_eq!(get(&ci, "app.baseURL"), Some("https://one.shop.ddev.site/"));
    }

    #[test]
    fn the_database_url_follows_the_server() {
        let url = |db: &str, host: &str, name: &str| {
            let db = Db::parse(db).unwrap();
            database_url(&SiteDb {
                db: &db,
                host,
                name,
            })
        };
        assert_eq!(
            url("postgres:16", "tryout-postgres-16", "db_pg"),
            "postgresql://db:db@tryout-postgres-16:5432/db_pg?serverVersion=16&charset=utf8"
        );
        assert_eq!(
            url("mysql:8.4", "tryout-mysql-8-4", "db_my"),
            "mysql://db:db@tryout-mysql-8-4:3306/db_my?serverVersion=8.4.0&charset=utf8mb4"
        );
        assert_eq!(
            url(
                "sqlite",
                "",
                "/var/www/html/.ddev/tryout-sites/l/sqlite/db_l.sqlite"
            ),
            "sqlite:////var/www/html/.ddev/tryout-sites/l/sqlite/db_l.sqlite"
        );
    }

    #[test]
    fn laravel_on_sqlite_names_the_file_and_no_server() {
        let lite = Db::parse("sqlite").unwrap();
        let v = vars(
            "laravel",
            &SiteDb {
                db: &lite,
                host: "",
                name: "/x/db.sqlite",
            },
            "https://x",
        );
        assert_eq!(get(&v, "DB_CONNECTION"), Some("sqlite"));
        assert_eq!(get(&v, "DB_DATABASE"), Some("/x/db.sqlite"));
        assert_eq!(get(&v, "DB_HOST"), None);
    }
}
