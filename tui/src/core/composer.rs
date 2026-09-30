//! The Composer overlays the add-on owns — never the project's composer.json.
//! `composer.tryout.json` in each instance is selected via COMPOSER= and pulls
//! the user's own composer.json in through composer-merge-plugin. Written as
//! PHP's json_encode writes (`core::phpjson`), so existing files do not churn.

use std::path::Path;

use serde_json::{Map, Value};

use super::ctx::Ctx;
use super::phpjson;

pub const OVERLAY: &str = "composer.tryout.json";
const MANAGED: [&str; 2] = ["typo3/cms-", "typo3/theme-"];

fn read(file: &Path) -> Result<Value, String> {
    let text =
        std::fs::read_to_string(file).map_err(|_| format!("{} not found", file.display()))?;
    serde_json::from_str(&text).map_err(|_| format!("cannot parse {}", file.display()))
}

fn write(file: &Path, v: &Value) -> Result<(), String> {
    // A short write leaves a truncated overlay, and the next composer install
    // fails far from the cause: say so here instead.
    std::fs::write(file, phpjson::to_string(v)).map_err(|e| {
        format!(
            "Failed to write {} (permissions? disk full?): {e}",
            file.display()
        )
    })
}

/// `data[k1][k2]…` as an object, created (or turned from an empty list, which
/// is how PHP read `{}`) on the way.
fn object_at<'a>(v: &'a mut Value, path: &[&str]) -> &'a mut Map<String, Value> {
    let mut cur = v;
    for k in path {
        let map = as_object(cur);
        cur = map
            .entry(k.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    as_object(cur)
}

fn as_object(v: &mut Value) -> &mut Map<String, Value> {
    if !v.is_object() {
        *v = Value::Object(Map::new());
    }
    v.as_object_mut().expect("just made an object")
}

/// The sysext package names under `<core>/typo3/sysext/*/composer.json`, sorted.
pub fn sysext_packages(core: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(core.join("typo3/sysext"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let v: Value = serde_json::from_str(
                &std::fs::read_to_string(e.path().join("composer.json")).ok()?,
            )
            .ok()?;
            v.get("name")?
                .as_str()
                .filter(|n| !n.is_empty())
                .map(String::from)
        })
        .collect();
    names.sort();
    names
}

/// Rewrite an instance overlay's `require` to exactly the sysexts on disk at
/// `@dev`, keeping every entry that is not a managed typo3/* package, and drop
/// the lock so a sysext Core removed really goes. Nothing is added by branch
/// name: a package the checkout lacks is a path repository to nowhere.
/// Returns the line to report.
pub fn sync(instance: &Path, core: &Path) -> Result<String, String> {
    let sysext = core.join("typo3/sysext");
    if !sysext.is_dir() {
        return Err(format!(
            "Error: {} not found. Clone TYPO3 Core first.",
            sysext.display()
        ));
    }
    let file = instance.join(OVERLAY);
    if !file.exists() {
        return Err(format!("Error: {} not found.", file.display()));
    }
    let mut data = read(&file).map_err(|_| format!("Error: Failed to parse {}", file.display()))?;
    let names = sysext_packages(core);
    if names.is_empty() {
        return Err(format!(
            "Error: No system extensions found in {}",
            sysext.display()
        ));
    }
    let old = data
        .get("require")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut require: Vec<(String, Value)> = old
        .into_iter()
        .filter(|(k, _)| !MANAGED.iter().any(|p| k.starts_with(p)))
        .collect();
    require.extend(
        names
            .iter()
            .map(|n| (n.clone(), Value::String("@dev".into()))),
    );
    require.sort_by(|a, b| a.0.cmp(&b.0));
    require.dedup_by(|a, b| a.0 == b.0);
    as_object(&mut data).insert(
        "require".into(),
        Value::Object(require.into_iter().collect()),
    );
    write(&file, &data).map_err(|e| format!("Error: {e}"))?;
    let _ = std::fs::remove_file(instance.join(OVERLAY.replace(".json", ".lock")));
    Ok(format!(
        "{} system extensions written to {OVERLAY}",
        names.len()
    ))
}

/// TYPO3-Instances/<name>/composer.tryout.json for a served worktree: the
/// primary's overlay with Core repointed at worktrees/<name>, packages/ shared,
/// the merge-plugin includes reaching ../primary/, and the platform PHP pinned
/// to what the site serves.
pub fn site_overlay(ctx: &Ctx, name: &str, php: &str) -> Result<(), String> {
    if !valid_name(name, false) {
        return Err("site-composer: invalid or missing site name".into());
    }
    if !php.is_empty() && !is_major_minor(php) {
        return Err(format!("site-composer: invalid PHP version '{php}'"));
    }
    let primary = ctx.instance_dir().join(OVERLAY);
    if !primary.exists() {
        return Err(format!("site-composer: {} not found", primary.display()));
    }
    let mut data =
        read(&primary).map_err(|_| format!("site-composer: cannot parse {}", primary.display()))?;

    if let Some(repos) = data.get_mut("repositories").and_then(Value::as_array_mut) {
        for repo in repos
            .iter_mut()
            .filter(|r| r.get("type").and_then(Value::as_str) == Some("path"))
        {
            let url = repo
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let new = if url.contains("typo3/sysext") {
                format!("../../worktrees/{name}/typo3/sysext/*")
            } else if url.contains("packages") {
                "../../packages/*".into()
            } else {
                continue;
            };
            as_object(repo).insert("url".into(), Value::String(new));
        }
    }
    if let Some(include) = data
        .pointer_mut("/extra/merge-plugin/include")
        .and_then(Value::as_array_mut)
    {
        for p in include.iter_mut() {
            if let Some(s) = p.as_str().filter(|s| !s.starts_with("..")) {
                *p = Value::String(format!("../primary/{s}"));
            }
        }
    }
    object_at(&mut data, &["config"]).insert("vendor-dir".into(), Value::String("vendor".into()));
    if !php.is_empty() {
        object_at(&mut data, &["config", "platform"])
            .insert("php".into(), Value::String(php.into()));
    }

    let dir = ctx.instances_dir().join(name);
    std::fs::create_dir_all(&dir)
        .map_err(|_| format!("site-composer: cannot create {}", dir.display()))?;
    write(&dir.join(OVERLAY), &data)
        .map_err(|_| format!("site-composer: cannot write {}/{OVERLAY}", dir.display()))
}

/// Point the PRIMARY overlay's sysext repository at worktrees/<name>, or at the
/// root checkout for an empty name — what `worktree use` moves. Returns the URL.
pub fn use_core(ctx: &Ctx, name: &str) -> Result<String, String> {
    if !name.is_empty() && !valid_name(name, true) {
        return Err(format!("use-core: invalid worktree name '{name}'"));
    }
    let file = ctx.instance_dir().join(OVERLAY);
    if !file.exists() {
        return Err(format!("use-core: {} not found", file.display()));
    }
    let mut data = read(&file).map_err(|_| format!("use-core: cannot parse {}", file.display()))?;
    let url = if name.is_empty() {
        "../../typo3/sysext/*".to_string()
    } else {
        format!("../../worktrees/{name}/typo3/sysext/*")
    };
    let mut found = false;
    if let Some(repos) = data.get_mut("repositories").and_then(Value::as_array_mut) {
        for repo in repos
            .iter_mut()
            .filter(|r| r.get("type").and_then(Value::as_str) == Some("path"))
        {
            if repo
                .get("url")
                .and_then(Value::as_str)
                .is_some_and(|u| u.contains("typo3/sysext"))
            {
                as_object(repo).insert("url".into(), Value::String(url.clone()));
                found = true;
            }
        }
    }
    if !found {
        return Err(format!(
            "use-core: no sysext path repository in {}",
            file.display()
        ));
    }
    write(&file, &data).map_err(|_| format!("use-core: cannot write {}", file.display()))?;
    Ok(url)
}

/// `[A-Za-z0-9._-]+`, and with `alnum_first` starting with a letter or digit.
fn valid_name(name: &str, alnum_first: bool) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && (!alnum_first || name.as_bytes()[0].is_ascii_alphanumeric())
}

fn is_major_minor(v: &str) -> bool {
    v.split_once('.').is_some_and(|(a, b)| {
        [a, b]
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    })
}
