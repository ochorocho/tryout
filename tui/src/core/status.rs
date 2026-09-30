//! `ddev tryout status`: the project at a glance.

use super::ctx::Ctx;
use super::kind::Mode;
use super::out::{BOLD, CYAN, DIM, GREEN, NC, RED, TEXT, YELLOW};
use super::{contrib, git, worktree};

/// The report's frame title: TYPO3 tryout for a Core checkout, tryout for a
/// project of one's own.
pub fn title(ctx: &Ctx) -> &'static str {
    match ctx.mode() {
        Mode::Core => "TYPO3 tryout — Status",
        Mode::Project => "tryout — Status",
    }
}

/// The report's lines, framed by the caller.
pub fn body(ctx: &Ctx, patches: &str) -> Vec<String> {
    let ok = format!("{GREEN}✓{TEXT}");
    let warn = format!("{YELLOW}!{TEXT}");
    let fail = format!("{RED}✗{TEXT}");
    let mut l = Vec::new();
    let mut line = |s: String| l.push(format!("{TEXT}{s}{NC}"));
    let core = &ctx.root;

    // Which of the two tryout works in.
    match ctx.mode() {
        Mode::Core => line(format!("  Mode:      {}", Mode::Core.label())),
        Mode::Project => {
            let t = project_type(ctx);
            line(format!("  Mode:      project ({t})"));
            let (support, note) = super::types::support(&ctx.env.project_type);
            let icon = if support == super::types::Support::ServeOnly {
                &warn
            } else {
                &ok
            };
            line(format!("  Sites:     {icon} {}", support.label()));
            line(format!("             {DIM}{note}"));
        }
    }

    if ctx.mode() == Mode::Core && !ctx.has_core() {
        line(format!("  Core:      {fail} not cloned"));
        line(format!("             {DIM}→ ddev tryout download"));
        return l;
    }

    let branch = git::out(core, &["branch", "--show-current"]).unwrap_or_else(|| "detached".into());
    let head =
        git::out(core, &["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let date = git::out(core, &["log", "-1", "--format=%cr"]).unwrap_or_default();
    let clean =
        git::ok(core, &["diff", "--quiet"]) && git::ok(core, &["diff", "--cached", "--quiet"]);
    let (state, icon) = if clean {
        ("clean", &ok)
    } else {
        ("dirty", &warn)
    };
    let what = match ctx.mode() {
        Mode::Core => "Core:     ",
        Mode::Project => "Repo:     ",
    };
    line(format!("  {what} {icon} {branch} ({head}) — {state}"));
    if !date.is_empty() {
        line(format!("             {DIM}{date}"));
    }

    if ctx.worktrees_dir().is_dir() {
        let active = ctx.active_worktree_name();
        let rows = worktree::rows(ctx);
        line(format!(
            "  Worktree:  {ok} {active} {DIM}({} total)",
            rows.len()
        ));
        for r in rows.iter().filter(|r| !r.active) {
            line(format!(
                "             {DIM}{} — {} ({})",
                r.name, r.branch, r.head
            ));
        }
        if ctx.mode() == Mode::Core && worktree::vendor_core_mismatch(ctx) {
            line(format!(
                "             {warn} vendor/ was built from a different Core"
            ));
            line(format!(
                "             {DIM}→ ddev tryout worktree use {active}"
            ));
        }
    }

    // What follows is Core's: its patches, packages, TYPO3 and Gerrit.
    if ctx.mode() == Mode::Project {
        let served = super::site::served_names(ctx);
        if served.is_empty() {
            line(format!("  Served:    {DIM}only the project itself"));
            line(format!(
                "             {DIM}→ ddev tryout worktree serve <name>"
            ));
        } else {
            line(format!("  Served:    {ok} {}", served.join(", ")));
        }
        line(format!("  Site:      {BOLD}{}", ctx.env.primary_url));
        line(format!("  Database:  {}", database_line(ctx)));
        // A database from a newer version of the framework breaks the code
        // (the container asks the databases; the host has no client).
        if ctx.in_container {
            let sites =
                std::iter::once(super::ctx::PRIMARY_SITE.to_string()).chain(served.iter().cloned());
            for name in sites {
                if let Some(why) = super::schema::mismatch(ctx, &name) {
                    let label = if super::site::is_primary(&name) {
                        "primary"
                    } else {
                        &name
                    };
                    line(format!("  Schema:    {warn} {label}: {why}"));
                    line(format!(
                        "             {DIM}newer than the code — pages will fail. → ddev tryout delete {label}"
                    ));
                }
            }
        }
        return l;
    }

    let upstream = format!("origin/{}", ctx.branch());
    if git::ok(core, &["rev-parse", &upstream]) {
        let range = format!("{upstream}..HEAD");
        let ahead: u32 = git::out(core, &["rev-list", "--count", &range])
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        if ahead > 0 {
            line(format!("  Patches:   {ok} {ahead} applied"));
            for c in git::lines(core, &["log", "--oneline", &range])
                .into_iter()
                .take(10)
            {
                line(format!("             {DIM}{c}"));
            }
        } else {
            line(format!("  Patches:   {DIM}none applied"));
        }
    }

    if patches.is_empty() {
        line(format!("  Config:    {DIM}no patches configured"));
    } else {
        line(format!("  Config:    {CYAN}TRYOUT_PATCHES={patches}"));
    }

    let packages: Vec<String> = std::fs::read_dir(ctx.root.join("packages"))
        .into_iter()
        .flatten()
        .flatten()
        // find -type d: a symlink to a directory is not one.
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    if packages.is_empty() {
        line(format!("  Packages:  {DIM}none in packages/"));
    } else {
        line(format!(
            "  Packages:  {ok} {} custom extension(s)",
            packages.len()
        ));
        for p in packages {
            line(format!("             {DIM}{p}"));
        }
    }

    if ctx.instance_dir().join("vendor").is_dir() {
        line(format!("  Composer:  {ok} installed"));
    } else {
        line(format!("  Composer:  {fail} not installed"));
        line(format!("             {DIM}→ ddev composer install"));
    }

    if ctx
        .instance_dir()
        .join("config/system/settings.php")
        .is_file()
    {
        line(format!("  TYPO3:     {ok} configured"));
    } else {
        line(format!("  TYPO3:     {fail} not set up"));
    }

    let cs = contrib::inspect(ctx);
    let parts: Vec<&str> = [
        (cs.hook_commit_msg, "commit-msg"),
        (cs.hook_pre_commit, "pre-commit"),
        (cs.template, "template"),
        (cs.gerrit_push(), "push-url"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    if parts.len() == 4 {
        let user = if cs.user.is_empty() { "?" } else { &cs.user };
        line(format!("  Contrib:   {ok} ready ({user})"));
    } else if !parts.is_empty() {
        line(format!("  Contrib:   {warn} partial ({})", parts.join(" ")));
        line(format!("             {DIM}→ ddev tryout cs doctor"));
    } else {
        line(format!("  Contrib:   {DIM}not configured"));
        line(format!("             {DIM}→ ddev tryout cs"));
    }

    line(format!("  Site:      {BOLD}{}", ctx.env.primary_url));
    line(format!("  Database:  {}", database_line(ctx)));
    l
}

/// The project's database server, and the extra ones served sites run on.
fn database_line(ctx: &Ctx) -> String {
    use super::db::{Db, Engine};
    let mut s = format!("{} {DIM}(db){NC}", Db::of_project(ctx).name());
    let on = |pick: &dyn Fn(&Db) -> bool| -> Vec<String> {
        super::site::served_names(ctx)
            .into_iter()
            .filter(|n| pick(&super::site::db(ctx, n)))
            .collect()
    };
    for d in super::site::extra_dbs(ctx) {
        s.push_str(&format!(
            " · {} {DIM}({}: {}){NC}",
            d.name(),
            d.host(ctx),
            on(&|x| *x == d).join(", ")
        ));
    }
    let lite = on(&|x| x.engine == Engine::Sqlite);
    if !lite.is_empty() {
        s.push_str(&format!(" · sqlite {DIM}({}){NC}", lite.join(", ")));
    }
    s
}

/// A titled block: the title in bold, then the lines.
pub fn boxed(title: &str, lines: &[String]) -> String {
    let mut s = format!("{BOLD}{title}{NC}\n");
    for l in lines {
        s.push_str(l);
        s.push('\n');
    }
    s
}

/// True when the installed payload is older than the one this binary belongs
/// to. A missing stamp is not reported: it predates the marker.
pub fn addon_is_stale(ctx: &Ctx, current: &str) -> bool {
    let installed = std::fs::read_to_string(ctx.tryout_dir().join(".version")).unwrap_or_default();
    let installed: String = installed.chars().filter(|c| !c.is_whitespace()).collect();
    !installed.is_empty() && installed != current
}

/// The project's DDEV type, as far as it is known here.
fn project_type(ctx: &Ctx) -> &str {
    if ctx.env.project_type.is_empty() {
        "type unknown"
    } else {
        &ctx.env.project_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ctx::DdevEnv;

    fn git(dir: &std::path::Path, args: &[&str]) {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }

    #[test]
    fn status_says_which_mode_it_works_in() {
        // Nothing cloned yet: core mode, and the way to clone it.
        let empty = tempfile::tempdir().unwrap();
        let lines = body(&Ctx::new(empty.path(), DdevEnv::default()), "").join("\n");
        assert!(lines.contains("Mode:      TYPO3 Core"), "{lines}");
        assert!(lines.contains("not cloned"), "{lines}");

        // A repository of the user's own: project mode, nothing of Core's.
        let own = tempfile::tempdir().unwrap();
        git(own.path(), &["init", "-q"]);
        git(
            own.path(),
            &["remote", "add", "origin", "git@github.com:acme/shop.git"],
        );
        let env = DdevEnv {
            project_type: "laravel".into(),
            ..DdevEnv::default()
        };
        let lines = body(&Ctx::new(own.path(), env), "").join("\n");
        assert!(lines.contains("project (laravel)"), "{lines}");
        assert!(
            lines.contains("database and URL from the environment"),
            "{lines}"
        );
        assert!(lines.contains("Repo:"), "{lines}");
        assert!(lines.contains("only the project itself"), "{lines}");
        for core_only in ["not cloned", "Patches:", "Contrib:", "TYPO3:"] {
            assert!(!lines.contains(core_only), "{core_only}: {lines}");
        }
    }
}
