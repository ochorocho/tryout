//! The host's side of the host/container split: the only module that runs
//! `ddev`. Container code never reaches it — `ddev` is only a stub in the image.

use std::process::{Command, Stdio};

use super::ctx::{CONTAINER_ROOT, Ctx};
use super::out;

/// The environment the container needs from the host; DDEV forwards nothing.
const FORWARDED: [&str; 4] = [
    "TRYOUT_BRANCH",
    "TRYOUT_GERRIT_USER",
    "TRYOUT_PATCHES",
    "TRYOUT_EVENTS",
];

/// Run one verb in the web container: `tryout ctr <args>` through
/// `ddev exec --raw`, which passes each argument as is — no shell in between to
/// re-split or interpret it. Then bring the host up to date, unless the verb
/// changes nothing. Returns the container's exit code.
pub fn delegate(ctx: &Ctx, args: &[String], flush: bool) -> i32 {
    let mut cmd = Command::new("ddev");
    cmd.args(["exec", "--raw", "--", "env"]);
    for k in FORWARDED {
        if let Ok(v) = std::env::var(k)
            && !v.is_empty()
        {
            cmd.arg(format!("{k}={v}"));
        }
    }
    // A local build for the container side (a /var/www/html/... path).
    if let Ok(bin) = std::env::var("TRYOUT_CONTAINER_BIN")
        && !bin.is_empty()
    {
        cmd.arg(format!("TRYOUT_BIN={bin}"));
    }
    cmd.arg(format!("{CONTAINER_ROOT}/.ddev/tryout/tryout"))
        .arg("ctr")
        .args(args);
    let rc = match cmd.status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(_) => {
            out::error("Could not run ddev");
            out::error("  → is DDEV installed and on PATH?");
            127
        }
    };
    if flush {
        flush_mutagen(ctx);
    }
    rc
}

/// Wait for Mutagen to bring what the container wrote over to the host, so a
/// worktree it just created is there for the next command. Quiet; a no-op
/// without Mutagen.
pub fn flush_mutagen(ctx: &Ctx) {
    if ctx.env.mutagen_enabled {
        let _ = Command::new("ddev")
            .args(["mutagen", "sync"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Hand a URL to the desktop's browser. The URL is printed either way, so a
/// headless box still leaves something to click.
pub fn open_url(ctx: &Ctx, url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if !ctx.in_container
        && on_path(opener)
        && Command::new(opener)
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    {
        out::success(format!("Opened {url}"));
        return;
    }
    out::info(format!("Open: {url}"));
}

/// `command -v`: is there an executable of that name on PATH?
pub fn on_path(name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|d| {
            std::fs::metadata(d.join(name))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}

/// `ddev restart`, its output shown.
pub fn restart() -> bool {
    Command::new("ddev")
        .arg("restart")
        .status()
        .is_ok_and(|s| s.success())
}
