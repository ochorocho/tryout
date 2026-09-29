//! `tryout __post-start`: DDEV's post-start hook, in the web container. Clone
//! Core if absent → reapply TRYOUT_PATCHES → sync the overlay and composer
//! install → first-run TYPO3 setup → extension:setup and a cache flush. It also
//! leaves the PHP versions the image provides where the host can read them.

use super::ctx::{Ctx, PRIMARY_SITE};
use super::out::{self, BOLD, NC};
use super::{Failed, Step, patch, php, proc, serve, worktree};

/// Where post-start leaves what only the container can see, for the host.
pub fn state_dir(ctx: &Ctx) -> std::path::PathBuf {
    ctx.tryout_dir().join(".state")
}

pub fn run(ctx: &Ctx) -> Step {
    out::print(&format!(
        "\n{BOLD}TYPO3 tryout — Post-Start Setup{NC}\n═══════════════════════════════════════\n\n"
    ));
    let branch = ctx.branch().to_string();

    if !ctx.has_core() {
        out::info("[1/5] Cloning TYPO3 Core repository into the project root...");
        out::info("This may take a few minutes on first run.");
        if worktree::clone_into_root(ctx, &branch).is_err() {
            out::error("Failed to clone TYPO3 Core");
            out::error("  → Try manually: ddev tryout download");
            return Err(Failed);
        }
        worktree::ensure_relative_paths(ctx);
        out::success("TYPO3 Core cloned");
    } else {
        out::info("[1/5] TYPO3 Core already present");
        // Both repair a checkout an older payload made.
        worktree::ensure_excludes(ctx);
        worktree::ensure_relative_paths(ctx);
    }

    // The host reads worktree state itself, but only the container knows which
    // PHP versions the image has: the TUI's PHP menus come from this.
    let state = state_dir(ctx);
    let _ = std::fs::create_dir_all(&state);
    let _ = std::fs::write(
        state.join("php-versions"),
        php::available_versions().join(" ") + "\n",
    );

    let patches: String = std::env::var("TRYOUT_PATCHES")
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if patches.is_empty() {
        out::info("[2/5] No patches configured");
    } else {
        out::info(format!(
            "[2/5] Resetting core to origin/{branch} and applying patches: {patches}"
        ));
        worktree::reset_to_base(ctx, &ctx.root, &branch, PRIMARY_SITE);
        if patch::apply_all(&ctx.root, &branch, &patches).is_err() {
            out::warn("Some patches failed to apply — check output above");
            out::warn("  → Reset and retry: ddev tryout reset");
        }
    }

    // The overlay's require block follows the sysexts on disk, so sync it every
    // start: it ships empty, and a branch switch changes the set.
    out::info("[3/5] Syncing composer.tryout.json with Core sysexts...");
    if !serve::sync_composer(&ctx.instance_dir(), &ctx.active_core_dir()) {
        out::error("Failed to sync composer.tryout.json");
        out::error("  → Try: ddev tryout download --reset && ddev restart");
        return Err(Failed);
    }
    serve::check_php_for_core(&ctx.active_core_dir(), &ctx.env.php_version, "")?;
    out::info("[3/5] Running composer install...");
    if !proc::run("composer", &["install"], Some(&ctx.instance_dir())) {
        out::error("Composer install failed");
        out::error("  → Try: ddev tryout download --reset && ddev restart");
        return Err(Failed);
    }
    out::success("Composer dependencies installed");

    out::info("[4/5] Checking TYPO3 setup...");
    if serve::setup_typo3(ctx, PRIMARY_SITE).is_err() {
        out::error("TYPO3 setup failed");
        out::error("  → ddev tryout delete   (wipe the database and set up fresh)");
        return Err(Failed);
    }

    out::info("[5/5] Setting up extensions and flushing caches...");
    let instance = ctx.instance_dir();
    if !serve::typo3(&instance, &["extension:setup"]) {
        out::warn("extension:setup had warnings");
    }
    if !serve::typo3(&instance, &["cache:flush"]) {
        out::warn("cache:flush had warnings");
    }
    out::success("Extensions ready, caches flushed");

    out::print("\n═══════════════════════════════════════\n");
    out::success("TYPO3 is ready!");
    out::print(&format!(
        "\n  {BOLD}Backend:{NC}  {}/typo3/\n  {BOLD}Login:{NC}    admin / Password.1\n\n  {BOLD}Commands:{NC}\n\
\x20   ddev tryout status     Show project status\n\
\x20   ddev tryout patch ID   Apply a Gerrit patch\n\
\x20   ddev tryout reset      Reset to clean state\n\n",
        ctx.env.primary_url
    ));
    Ok(())
}
