//! Does a site's database belong to its code? A database of a newer framework
//! version breaks older code — Drupal 10.6 on an 11.x schema fails every page —
//! and nothing downgrades one. Serving guards against making such a site
//! (`serve::primary_fits`); `status` finds one that exists anyway: a worktree
//! switched to an older branch, a database copied by hand.

use std::collections::BTreeSet;
use std::path::Path;

use super::ctx::Ctx;
use super::{db, site};

/// What the database has that the code does not know, in words; None when it
/// fits, or when either side cannot be read (no such table, no such file).
pub fn mismatch(ctx: &Ctx, name: &str) -> Option<String> {
    let dir = site::dir(ctx, name);
    let docroot = ctx.env.docroot.trim_matches('/');
    let rows = |sql: &str| -> Option<Vec<String>> {
        let o = db::site_sql(ctx, name, sql)?;
        o.status.success().then(|| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
    };
    match ctx.env.project_type.as_str() {
        t if t.starts_with("drupal") => {
            let code = drupal_code_schema(&dir, docroot)?;
            let value = rows(
                "SELECT value FROM key_value WHERE collection='system.schema' AND name='system'",
            )?;
            let data = serialized_int(value.first()?)?;
            (data > code).then(|| {
                format!("its database is at Drupal schema {data}, its code knows up to {code}")
            })
        }
        "wordpress" => {
            let code = wordpress_code_db_version(&dir, docroot)?;
            let value = rows("SELECT option_value FROM wp_options WHERE option_name='db_version'")?;
            let data: u64 = value.first()?.parse().ok()?;
            (data > code).then(|| {
                format!("its database is at WordPress db_version {data}, its code knows {code}")
            })
        }
        "laravel" => {
            let code = file_stems(&dir.join("database/migrations"));
            let data = rows("SELECT migration FROM migrations")?;
            unknown(&data, &code, "migrations")
        }
        "symfony" => {
            let code = file_stems(&dir.join("migrations"));
            // `DoctrineMigrations\Version20240101…` → `Version20240101…`
            let data: Vec<String> = rows("SELECT version FROM doctrine_migration_versions")?
                .iter()
                .map(|v| v.rsplit('\\').next().unwrap_or(v).to_string())
                .collect();
            unknown(&data, &code, "Doctrine migrations")
        }
        _ => None,
    }
}

/// The highest `system_update_N` Drupal's code has.
pub fn drupal_code_schema(checkout: &Path, docroot: &str) -> Option<u64> {
    [
        format!("{docroot}/core/modules/system/system.install"),
        "core/modules/system/system.install".to_string(),
    ]
    .iter()
    .find_map(|f| std::fs::read_to_string(checkout.join(f.trim_start_matches('/'))).ok())
    .and_then(|text| max_update(&text, "function system_update_"))
}

pub fn max_update(text: &str, prefix: &str) -> Option<u64> {
    text.match_indices(prefix)
        .filter_map(|(i, _)| {
            let n: String = text[i + prefix.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            n.parse().ok()
        })
        .max()
}

/// `$wp_db_version = 60421;` from wp-includes/version.php.
pub fn wordpress_code_db_version(checkout: &Path, docroot: &str) -> Option<u64> {
    let text = [
        format!("{docroot}/wp-includes/version.php"),
        "wp-includes/version.php".to_string(),
    ]
    .iter()
    .find_map(|f| std::fs::read_to_string(checkout.join(f.trim_start_matches('/'))).ok())?;
    let rest = &text[text.find("$wp_db_version")?..];
    rest.split(|c: char| !c.is_ascii_digit())
        .find(|p| !p.is_empty())?
        .parse()
        .ok()
}

/// Drupal stores the schema serialized: `i:11501;`.
pub fn serialized_int(v: &str) -> Option<u64> {
    v.trim()
        .trim_start_matches("i:")
        .trim_end_matches(';')
        .parse()
        .ok()
}

fn file_stems(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()? != "php" {
                return None;
            }
            Some(p.file_stem()?.to_string_lossy().into_owned())
        })
        .collect()
}

/// Migrations the database ran that the code does not have.
pub fn unknown(data: &[String], code: &BTreeSet<String>, what: &str) -> Option<String> {
    if code.is_empty() {
        return None;
    }
    let missing: Vec<&String> = data.iter().filter(|d| !code.contains(*d)).collect();
    let first = missing.first()?;
    Some(format!(
        "its database ran {} {what} its code does not have (e.g. {first})",
        missing.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drupal_and_wordpress_versions_are_read_from_their_files() {
        let d = tempfile::tempdir().unwrap();
        let w = |rel: &str, text: &str| {
            let p = d.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        w(
            "core/modules/system/system.install",
            "function system_update_10100() {}\nfunction system_update_10600(&$sandbox) {}\n",
        );
        assert_eq!(drupal_code_schema(d.path(), ""), Some(10600));
        assert_eq!(serialized_int("i:11501;"), Some(11501));
        // The report: an 11.x database (11501) on 10.6 code (10600).
        assert!(serialized_int("i:11501;").unwrap() > 10600);

        w(
            "web/wp-includes/version.php",
            "<?php\n$wp_version = '6.8';\n$wp_db_version = 60421;\n",
        );
        assert_eq!(wordpress_code_db_version(d.path(), "web"), Some(60421));
    }

    #[test]
    fn migrations_the_code_does_not_have_are_named() {
        let code: BTreeSet<String> = ["2024_01_01_000000_create_users_table".to_string()].into();
        let data = vec![
            "2024_01_01_000000_create_users_table".to_string(),
            "2026_09_01_000000_add_wishlist".to_string(),
        ];
        let msg = unknown(&data, &code, "migrations").unwrap();
        assert!(
            msg.contains("ran 1 migrations") && msg.contains("add_wishlist"),
            "{msg}"
        );
        assert_eq!(unknown(&data[..1], &code, "migrations"), None);
        // No migration files at all: nothing to judge by.
        assert_eq!(unknown(&data, &BTreeSet::new(), "migrations"), None);
    }
}
