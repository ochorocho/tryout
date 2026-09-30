//! Which PHP a Core checkout accepts, and which of the image's PHPs satisfy it.
//! Judged in Rust, the way the add-on always judged it, so the host needs no PHP.

use std::cmp::Ordering;
use std::path::Path;

use super::vsort;

/// `require.php` from a composer.json — not `config.platform.php`, which is a
/// pinned build version. None when there is none or the file cannot be read:
/// the caller's cue to fall back rather than guess.
pub fn core_constraint(composer_json: &Path) -> Option<String> {
    let text = std::fs::read_to_string(composer_json).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let c = v.get("require")?.get("php")?.as_str()?;
    (!c.is_empty()).then(|| c.to_string())
}

/// Does PHP `version` (major.minor) satisfy `constraint`? Every clause must hold;
/// `^8.2` means `>=8.2 <9.0`, and a bare version means `>=`. Clauses of any other
/// shape are ignored, exactly as before.
pub fn satisfies(constraint: &str, version: &str) -> bool {
    let v = format!("{version}.0");
    constraint
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|p| !p.is_empty())
        .all(|part| {
            if let Some((major, minor)) = caret(part) {
                version_compare(&v, &format!("{major}.{minor}.0")) != Ordering::Less
                    && version_compare(&v, &format!("{}.0.0", major + 1)) == Ordering::Less
            } else if let Some((op, want)) = comparison(part) {
                let o = version_compare(&v, want);
                match op {
                    ">=" => o != Ordering::Less,
                    "<=" => o != Ordering::Greater,
                    ">" => o == Ordering::Greater,
                    "<" => o == Ordering::Less,
                    _ => o == Ordering::Equal,
                }
            } else {
                true
            }
        })
}

/// `^(\d+)\.(\d+)` at the start of a clause (anything after it is ignored).
fn caret(part: &str) -> Option<(u64, u64)> {
    let rest = part.strip_prefix('^')?;
    let (major, rest) = leading_number(rest)?;
    let (minor, _) = leading_number(rest.strip_prefix('.')?)?;
    Some((major, minor))
}

/// `^(>=|<=|>|<|=)?\s*(\d+(?:\.\d+){0,2})$`
fn comparison(part: &str) -> Option<(&str, &str)> {
    let op = [">=", "<=", ">", "<", "="]
        .into_iter()
        .find(|op| part.starts_with(op));
    let rest = part[op.map_or(0, str::len)..].trim_start();
    let pieces: Vec<&str> = rest.split('.').collect();
    let numeric = (1..=3).contains(&pieces.len())
        && pieces
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    numeric.then_some((op.unwrap_or(">="), rest))
}

fn leading_number(s: &str) -> Option<(u64, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    Some((s[..end].parse().ok()?, &s[end..]))
}

/// PHP's `version_compare` for plain dotted numbers: part by part, and when one
/// runs out, the longer is greater ("8.2.0" > "8.2").
fn version_compare(a: &str, b: &str) -> Ordering {
    let parts = |s: &str| {
        s.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parts(a).cmp(&parts(b))
}

/// The PHP versions the web image provides, low to high: `/usr/bin/php8.N`.
pub fn available_versions() -> Vec<String> {
    available_in(Path::new("/usr/bin"))
}

pub fn available_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_prefix("php")
                .map(String::from)
        })
        .filter(|v| {
            v.strip_prefix("8.")
                .is_some_and(|m| !m.is_empty() && m.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    vsort::sort(&mut v);
    v
}

/// The versions in `available` that satisfy `constraint`, ascending.
pub fn matching(constraint: &str, available: &[String]) -> Vec<String> {
    available
        .iter()
        .filter(|v| satisfies(constraint, v))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<String> {
        ["8.1", "8.2", "8.3", "8.4", "8.5"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn caret_means_this_major() {
        assert_eq!(matching("^8.2", &all()), ["8.2", "8.3", "8.4", "8.5"]);
        assert!(!satisfies("^8.2", "9.0"));
    }

    #[test]
    fn an_upper_bound_is_honoured() {
        assert_eq!(matching(">=8.2 <8.4", &all()), ["8.2", "8.3"]);
        assert_eq!(matching(">=8.2,<8.4", &all()), ["8.2", "8.3"]);
    }

    #[test]
    fn a_bare_version_is_a_floor_and_php_compare_rules_hold() {
        assert_eq!(matching("8.3", &all()), ["8.3", "8.4", "8.5"]);
        // version_compare: "8.2.0" > "8.2", so <=8.2 excludes 8.2 — as PHP did.
        assert_eq!(matching("<=8.2", &all()), ["8.1"]);
        assert!(satisfies("=8.2.0", "8.2"));
    }

    #[test]
    fn unknown_clauses_are_ignored() {
        assert_eq!(matching("~8.2", &all()), all());
    }

    #[test]
    fn the_constraint_comes_from_require_not_the_platform() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("composer.json");
        std::fs::write(
            &f,
            r#"{"config":{"platform":{"php":"8.2.0"}},"require":{"php":"^8.5"}}"#,
        )
        .unwrap();
        assert_eq!(core_constraint(&f).as_deref(), Some("^8.5"));
        std::fs::write(&f, r#"{"require":{}}"#).unwrap();
        assert_eq!(core_constraint(&f), None);
        assert_eq!(core_constraint(&d.path().join("missing.json")), None);
    }

    #[test]
    fn the_image_versions_are_read_and_version_sorted() {
        let d = tempfile::tempdir().unwrap();
        for f in [
            "php8.10",
            "php8.2",
            "php8.4",
            "php",
            "php7.4",
            "php8.x",
            "php-fpm8.4",
        ] {
            std::fs::write(d.path().join(f), "").unwrap();
        }
        assert_eq!(available_in(d.path()), ["8.2", "8.4", "8.10"]);
    }
}
