//! What tryout can do for each DDEV project type: how far a served worktree
//! of it is wired (the support matrix), and for the types that keep their
//! database in a settings file, the per-site copy of DDEV's own.
//!
//! DDEV writes one settings file for the project's docroot, with database
//! `db`. A worktree has none of it — the file is gitignored — so its site gets
//! a copy of the primary's with a snippet that takes the site's own database
//! (and URL) from the environment its vhost and `exec` set (`appenv`).

use std::path::Path;

use super::ctx::Ctx;
use super::out;

/// How far tryout wires a served worktree of a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Its own database and URL, from the environment (dotenv frameworks).
    Env,
    /// Its own database (and URL), through a per-site copy of DDEV's settings
    /// file.
    Settings,
    /// Served from its worktree; its database settings are the app's own
    /// business (tryout cannot rewrite them safely).
    ServeOnly,
}

impl Support {
    pub fn label(self) -> &'static str {
        match self {
            Support::Env => "database and URL from the environment",
            Support::Settings => "database and URL through a per-site settings file",
            Support::ServeOnly => "served only — point the app at its database yourself",
        }
    }
}

/// Every type DDEV knows (`ddev config --project-type`), with its support and
/// what a user should know. An unknown type is served like `php`.
pub const MATRIX: &[(&str, Support, &str)] = &[
    ("asterios", Support::Env, "DB_*, APP_URL"),
    ("backdrop", Support::Settings, "settings.ddev.php"),
    ("cakephp", Support::Env, "DATABASE_URL, APP_FULL_BASE_URL"),
    (
        "codeigniter",
        Support::Env,
        "database.default.*, app.baseURL",
    ),
    ("craftcms", Support::Env, "CRAFT_DB_*, PRIMARY_SITE_URL"),
    ("drupal", Support::Settings, "settings.ddev.php"),
    ("drupal6", Support::Settings, "settings.ddev.php ($db_url)"),
    ("drupal7", Support::Settings, "settings.ddev.php"),
    ("drupal8", Support::Settings, "settings.ddev.php"),
    ("drupal9", Support::Settings, "settings.ddev.php"),
    ("drupal10", Support::Settings, "settings.ddev.php"),
    ("drupal11", Support::Settings, "settings.ddev.php"),
    ("drupal12", Support::Settings, "settings.ddev.php"),
    ("generic", Support::Env, "TRYOUT_DB_*, DATABASE_URL"),
    (
        "joomla",
        Support::ServeOnly,
        "configuration.php is the app's own",
    ),
    ("laravel", Support::Env, "DB_*, APP_URL"),
    ("magento", Support::ServeOnly, "local.xml is the app's own"),
    (
        "magento2",
        Support::ServeOnly,
        "app/etc/env.php and the base URL in the database",
    ),
    (
        "maho",
        Support::ServeOnly,
        "app/etc/local.xml is the app's own",
    ),
    (
        "modx",
        Support::ServeOnly,
        "config.inc.php is the app's own",
    ),
    ("php", Support::Env, "TRYOUT_DB_*, DATABASE_URL"),
    (
        "shopware6",
        Support::Env,
        "DATABASE_URL, APP_URL; storefront domain updated",
    ),
    ("silverstripe", Support::Env, "SS_DATABASE_*, SS_BASE_URL"),
    ("symfony", Support::Env, "DATABASE_URL"),
    ("typo3", Support::Settings, "config/system/additional.php"),
    (
        "wordpress",
        Support::Settings,
        "wp-config-ddev.php (MariaDB/MySQL only)",
    ),
    ("wp-bedrock", Support::Env, "DB_*, WP_HOME"),
];

/// A type's support and note; an unknown one is served like `php`.
pub fn support(project_type: &str) -> (Support, &'static str) {
    MATRIX
        .iter()
        .find(|(t, ..)| *t == project_type)
        .map(|(_, s, n)| (*s, *n))
        .unwrap_or((
            Support::Env,
            "unknown type: TRYOUT_DB_*, DATABASE_URL — the app may need its own configuration",
        ))
}

/// Where tryout's snippet goes in the copy of DDEV's file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// After everything: a later assignment wins (Drupal, TYPO3, Backdrop).
    Append,
    /// Right after `<?php`: a constant defined first wins (WordPress).
    Prepend,
}

/// DDEV's settings file for a type, and what else a worktree needs of the
/// project's local (gitignored) configuration. `{docroot}` is the project's.
pub struct Settings {
    /// DDEV's file, the first that exists in the project.
    pub files: &'static [&'static str],
    /// Copied into the worktree as they are, where it has none.
    pub also: &'static [&'static str],
    pub place: Place,
    pub snippet: &'static str,
}

/// Marks tryout's snippet, so a re-serve rewrites the file it wrote before.
pub const MARKER: &str = "tryout: this site's own database";

pub fn settings(project_type: &str) -> Option<Settings> {
    let drupal = Settings {
        files: &["{docroot}/sites/default/settings.ddev.php"],
        also: &["{docroot}/sites/default/settings.php"],
        place: Place::Append,
        snippet: DRUPAL,
    };
    match project_type {
        "drupal" | "drupal7" | "drupal8" | "drupal9" | "drupal10" | "drupal11" | "drupal12" => {
            Some(drupal)
        }
        "drupal6" => Some(Settings {
            snippet: DRUPAL6,
            ..drupal
        }),
        "backdrop" => Some(Settings {
            files: &["{docroot}/settings.ddev.php"],
            also: &["{docroot}/settings.php"],
            place: Place::Append,
            snippet: BACKDROP,
        }),
        "wordpress" => Some(Settings {
            files: &["{docroot}/wp-config-ddev.php"],
            also: &["{docroot}/wp-config.php"],
            place: Place::Prepend,
            snippet: WORDPRESS,
        }),
        "typo3" => Some(Settings {
            // v12+ Composer, v12+ legacy, v11 and older — as DDEV picks them.
            files: &[
                "config/system/additional.php",
                "{docroot}/typo3conf/system/additional.php",
                "{docroot}/typo3conf/AdditionalConfiguration.php",
            ],
            also: &[
                "config/system/settings.php",
                "{docroot}/typo3conf/system/settings.php",
                "{docroot}/typo3conf/LocalConfiguration.php",
            ],
            place: Place::Append,
            snippet: TYPO3,
        }),
        _ => None,
    }
}

const DRUPAL: &str = r#"
// tryout: this site's own database, from the environment its vhost and
// `ddev tryout exec` set. Absent for the primary: DDEV's settings stand.
if (getenv('TRYOUT_DB_NAME')) {
  $tryout_driver = getenv('TRYOUT_DB_DRIVER');
  $databases['default']['default'] = $tryout_driver === 'sqlite'
    ? ['driver' => 'sqlite', 'database' => getenv('TRYOUT_DB_NAME')]
    : [
      'driver' => $tryout_driver === 'postgres' ? 'pgsql' : 'mysql',
      'database' => getenv('TRYOUT_DB_NAME'),
      'host' => getenv('TRYOUT_DB_HOST'),
      'port' => getenv('TRYOUT_DB_PORT'),
      'username' => getenv('TRYOUT_DB_USER'),
      'password' => getenv('TRYOUT_DB_PASSWORD'),
      'prefix' => '',
    ];
}
"#;

const DRUPAL6: &str = r#"
// tryout: this site's own database, from the environment its vhost and
// `ddev tryout exec` set. Absent for the primary: DDEV's settings stand.
if (getenv('TRYOUT_DB_NAME')) {
  $db_url = sprintf('%s://%s:%s@%s:%s/%s',
    getenv('TRYOUT_DB_DRIVER') === 'postgres' ? 'pgsql' : 'mysqli',
    getenv('TRYOUT_DB_USER'), getenv('TRYOUT_DB_PASSWORD'),
    getenv('TRYOUT_DB_HOST'), getenv('TRYOUT_DB_PORT'), getenv('TRYOUT_DB_NAME'));
}
"#;

const BACKDROP: &str = r#"
// tryout: this site's own database, from the environment its vhost and
// `ddev tryout exec` set. Absent for the primary: DDEV's settings stand.
if (getenv('TRYOUT_DB_NAME')) {
  $database = sprintf('mysql://%s:%s@%s:%s/%s', getenv('TRYOUT_DB_USER'),
    getenv('TRYOUT_DB_PASSWORD'), getenv('TRYOUT_DB_HOST'),
    getenv('TRYOUT_DB_PORT'), getenv('TRYOUT_DB_NAME'));
}
"#;

const WORDPRESS: &str = r#"
// tryout: this site's own database, from the environment its vhost and
// `ddev tryout exec` set; defined before DDEV's own, so these win. Absent for
// the primary: DDEV's settings stand.
if (getenv('TRYOUT_DB_NAME')) {
	define( 'DB_NAME', getenv( 'TRYOUT_DB_NAME' ) );
	define( 'DB_USER', getenv( 'TRYOUT_DB_USER' ) );
	define( 'DB_PASSWORD', getenv( 'TRYOUT_DB_PASSWORD' ) );
	define( 'DB_HOST', getenv( 'TRYOUT_DB_HOST' ) . ':' . getenv( 'TRYOUT_DB_PORT' ) );
	define( 'WP_HOME', getenv( 'TRYOUT_URL' ) );
	define( 'WP_SITEURL', getenv( 'TRYOUT_URL' ) );
}
"#;

const TYPO3: &str = r#"
// tryout: this site's own database, from the environment its vhost and
// `ddev tryout exec` set. Absent for the primary: DDEV's settings stand.
if (getenv('TRYOUT_DB_NAME')) {
    $tryoutDriver = getenv('TRYOUT_DB_DRIVER');
    $GLOBALS['TYPO3_CONF_VARS']['DB']['Connections']['Default'] = $tryoutDriver === 'sqlite'
        ? ['driver' => 'pdo_sqlite', 'path' => getenv('TRYOUT_DB_NAME')]
        : array_merge($GLOBALS['TYPO3_CONF_VARS']['DB']['Connections']['Default'] ?? [], [
            'driver' => $tryoutDriver === 'postgres' ? 'pdo_pgsql' : 'mysqli',
            'dbname' => getenv('TRYOUT_DB_NAME'),
            'host' => getenv('TRYOUT_DB_HOST'),
            'port' => (int)getenv('TRYOUT_DB_PORT'),
            'user' => getenv('TRYOUT_DB_USER'),
            'password' => getenv('TRYOUT_DB_PASSWORD'),
        ]);
}
"#;

/// What runs in a site after its database was copied from another: the
/// types that keep their own URL in the database move it to the site's host.
pub fn after_copy(project_type: &str, host: &str) -> Option<Vec<String>> {
    match project_type {
        "shopware6" => Some(
            ["bin/console", "sales-channel:update:domain", host]
                .map(String::from)
                .to_vec(),
        ),
        _ => None,
    }
}

/// DDEV's file with tryout's snippet in its place.
pub fn with_snippet(ddev_file: &str, s: &Settings) -> String {
    match s.place {
        // After a closing `?>` it would be output, not code.
        Place::Append => {
            let body = ddev_file.trim_end();
            let body = body.strip_suffix("?>").unwrap_or(body);
            format!("{}\n{}", body.trim_end(), s.snippet)
        }
        Place::Prepend => match ddev_file.split_once("<?php") {
            Some((before, after)) => format!("{before}<?php\n{}{after}", s.snippet),
            None => format!("<?php\n{}?>\n{ddev_file}", s.snippet),
        },
    }
}

fn resolve(ctx: &Ctx, rel: &str) -> String {
    let docroot = ctx.env.docroot.trim_matches('/');
    let r = rel.replace("{docroot}", docroot);
    r.trim_start_matches('/').replace("//", "/")
}

/// Give a project site of a settings-file type its copy of DDEV's settings,
/// pointed at its own database, and the local files it lacks. A file the
/// worktree has of its own (committed) is left as it is, and said so.
pub fn write_settings(ctx: &Ctx, name: &str, dir: &Path) {
    let Some(s) = settings(&ctx.env.project_type) else {
        return;
    };
    for rel in s.also.iter().map(|r| resolve(ctx, r)) {
        let (from, to) = (ctx.root.join(&rel), dir.join(&rel));
        if from.is_file() && !to.exists() {
            let _ = to.parent().map(std::fs::create_dir_all);
            if std::fs::copy(&from, &to).is_ok() {
                out::info(format!("Copied the project's {rel} into '{name}'"));
            }
        }
    }
    let Some(rel) = s
        .files
        .iter()
        .map(|r| resolve(ctx, r))
        .find(|r| ctx.root.join(r).is_file())
    else {
        out::warn(format!(
            "No DDEV settings file for {} in the project — '{name}' uses the primary's database",
            ctx.env.project_type
        ));
        out::warn("  → ddev restart   (DDEV writes it), then serve again");
        return;
    };
    let target = dir.join(&rel);
    let own = std::fs::read_to_string(&target).unwrap_or_default();
    if target.exists() && !own.contains(MARKER) {
        out::warn(format!(
            "'{name}' has its own {rel} (committed?) — left as it is; its database is db"
        ));
        return;
    }
    let Ok(ddev) = std::fs::read_to_string(ctx.root.join(&rel)) else {
        return;
    };
    let _ = target.parent().map(std::fs::create_dir_all);
    match std::fs::write(&target, with_snippet(&ddev, &s)) {
        Ok(()) => out::info(format!("Wrote {rel} for '{name}' — its own database")),
        Err(e) => out::warn(format!("Could not write {}: {e}", target.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ddev config --project-type` of DDEV v1.25.4.
    const DDEV_TYPES: &str = "asterios, backdrop, cakephp, codeigniter, craftcms, drupal, \
        drupal6, drupal7, drupal8, drupal9, drupal10, drupal11, drupal12, generic, joomla, \
        laravel, magento, magento2, maho, modx, php, shopware6, silverstripe, symfony, typo3, \
        wordpress, wp-bedrock";

    #[test]
    fn every_type_ddev_lists_has_a_stated_support_level() {
        let listed: Vec<&str> = DDEV_TYPES.split(", ").map(str::trim).collect();
        let matrix: Vec<&str> = MATRIX.iter().map(|(t, ..)| *t).collect();
        assert_eq!(listed, matrix);
        // Every settings-file type has its adapter.
        for (t, s, _) in MATRIX {
            assert_eq!(*s == Support::Settings, settings(t).is_some(), "{t}");
        }
        // An unknown one is served generically, never refused.
        assert_eq!(support("next-big-thing").0, Support::Env);
    }

    #[test]
    fn the_docs_state_every_types_support_and_link_a_page_for_it() {
        let page = include_str!("../../../docs/frameworks/index.md");
        let pages = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs");
        for (t, s, _) in MATRIX {
            let label = match s {
                Support::Env => "environment",
                Support::Settings => "settings file",
                Support::ServeOnly => "served only",
            };
            let row = page
                .lines()
                .find(|l| l.starts_with(&format!("| `{t}` | {label} |")))
                .unwrap_or_else(|| panic!("docs/frameworks/index.md lacks {t} as {label}"));
            // Its row links the page that explains it, and that page exists.
            let link = row
                .split("](/")
                .nth(1)
                .and_then(|r| r.split(')').next())
                .unwrap_or_else(|| panic!("{t}: no page linked"));
            let file = pages.join(format!("{}.md", link.split('#').next().unwrap()));
            assert!(file.is_file(), "{t}: {} is missing", file.display());
        }
    }

    #[test]
    fn the_snippet_goes_where_it_wins() {
        let drupal = settings("drupal11").unwrap();
        let out = with_snippet("<?php\n$databases['default']['default'] = [];\n", &drupal);
        assert!(out.starts_with("<?php\n$databases"));
        assert!(out.trim_end().ends_with('}'));
        assert!(out.contains(MARKER));
        let closed = with_snippet("<?php\n$x = 1;\n?>\n", &drupal);
        assert!(!closed.contains("?>"), "{closed}");

        // WordPress: before DDEV's defines, which a first define wins over.
        let wp = settings("wordpress").unwrap();
        let out = with_snippet(
            "<?php\n/** #ddev-generated */\ndefine('DB_NAME', 'db');\n",
            &wp,
        );
        let (ours, ddevs) = (
            out.find("TRYOUT_DB_NAME").unwrap(),
            out.find("'db')").unwrap(),
        );
        assert!(out.starts_with("<?php\n") && ours < ddevs, "{out}");
    }
}
