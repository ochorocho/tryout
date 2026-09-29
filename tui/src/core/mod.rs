//! The add-on's own logic, shared by the command line (host and container side)
//! and the terminal UI. Ported from tryout/functions.sh one helper group per
//! module.

pub mod composer;
pub mod contrib;
pub mod ctx;
pub mod db;
pub mod ddev;
pub mod fpm;
pub mod gerrit;
pub mod git;
pub mod out;
pub mod patch;
pub mod php;
pub mod poststart;
pub mod phpjson;
pub mod proc;
pub mod prompt;
pub mod serve;
pub mod site;
pub mod status;
pub mod vsort;
pub mod webserver;
pub mod worktree;

/// A step failed and has said why: its message is already printed. What the
/// caller does with it — stop, fall back, exit 1 — is the caller's decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failed;

/// The outcome of a step that reports its own failure.
pub type Step = Result<(), Failed>;

/// The payload version the add-on stamps into .ddev/tryout/.version, which
/// `status` compares. Bump it whenever what a user sees changes.
pub const PAYLOAD_VERSION: &str = "48";

#[cfg(test)]
mod tests {
    /// `ddev` is only a stub inside the web image: the only code allowed to run
    /// it is the host's side of the split, core/ddev.rs.
    #[test]
    fn nothing_but_the_host_side_runs_ddev() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![src.join("cli/container.rs")];
        files.extend(
            std::fs::read_dir(src.join("core"))
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.file_name().is_some_and(|n| n != "ddev.rs")),
        );
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            assert!(
                !text.contains("Command::new(\"ddev\")"),
                "{} runs ddev",
                f.display()
            );
        }
    }

    #[test]
    fn the_payload_version_matches_the_bash_payload() {
        let f = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tryout/functions.sh"
        ))
        .unwrap();
        let v = f
            .lines()
            .find_map(|l| l.strip_prefix("TRYOUT_VERSION="))
            .unwrap();
        assert_eq!(v, super::PAYLOAD_VERSION);
    }
}
