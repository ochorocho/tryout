//! The `tryout` command line. `tryout <verb>` runs on the host (what
//! `ddev tryout` execs), `tryout ctr <verb>` inside the web container, where the
//! host delegates the work of every verb that is not host-only.

pub mod complete;
pub mod container;
pub mod help;
pub mod host;
pub mod verbs;

use std::path::PathBuf;

use crate::core::ctx::{CONTAINER_ROOT, Ctx};

/// A verb's failure, its message already printed: the exit code to leave with.
#[derive(Debug, PartialEq, Eq)]
pub struct Exit(pub i32);

pub type Res = Result<(), Exit>;

/// Everything but `ui`, which keeps its own entry.
pub fn main(args: Vec<String>) -> i32 {
    match args.first().map(String::as_str) {
        Some("ctr") => {
            let ctx = Ctx::from_env(approot().unwrap_or_else(|| PathBuf::from(CONTAINER_ROOT)));
            code(container::run(&ctx, &args[1..]))
        }
        Some("__complete") => {
            complete::run(&args[1..]);
            0
        }
        _ => match approot() {
            Some(root) => code(host::run(&Ctx::from_env(root), &args)),
            None => {
                crate::core::out::error("Not inside a DDEV project (DDEV_APPROOT is not set)");
                crate::core::out::error("  → run it as: ddev tryout …");
                1
            }
        },
    }
}

fn code(r: Res) -> i32 {
    match r {
        Ok(()) => 0,
        Err(Exit(c)) => c,
    }
}

fn approot() -> Option<PathBuf> {
    std::env::var_os("DDEV_APPROOT")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// `require_core`: refuse before anything else when the root is no checkout.
pub fn require_core(ctx: &Ctx) -> Res {
    if ctx.has_core() {
        return Ok(());
    }
    crate::core::out::error("TYPO3 Core not found — the project root is not a git checkout");
    crate::core::out::error("  → Run: ddev tryout download");
    Err(Exit(1))
}

/// Some verbs take no arguments; a silently ignored word makes a typo look
/// like it worked.
pub fn reject_args(cmd: &str, args: &[String]) -> Res {
    if args.is_empty() {
        return Ok(());
    }
    crate::core::out::error(format!(
        "'{cmd}' takes no arguments, but got: {}",
        args.join(" ")
    ));
    Err(Exit(1))
}
