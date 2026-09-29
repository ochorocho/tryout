//! `ddev tryout status`: the project at a glance.

use super::ctx::Ctx;
use super::out::{BOLD, CYAN, DIM, GREEN, NC, RED, TEXT, YELLOW};
use super::{contrib, git, worktree};

/// The report's lines, framed by the caller.
pub fn body(ctx: &Ctx, patches: &str) -> Vec<String> {
    let ok = format!("{GREEN}✓{TEXT}");
    let warn = format!("{YELLOW}!{TEXT}");
    let fail = format!("{RED}✗{TEXT}");
    let mut l = Vec::new();
    let mut line = |s: String| l.push(format!("{TEXT}{s}{NC}"));
    let core = &ctx.root;

    if !ctx.has_core() {
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
    line(format!("  Core:      {icon} {branch} ({head}) — {state}"));
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
        if worktree::vendor_core_mismatch(ctx) {
            line(format!(
                "             {warn} vendor/ was built from a different Core"
            ));
            line(format!(
                "             {DIM}→ ddev tryout worktree use {active}"
            ));
        }
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
    l
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
