//! The add-on's own logic, shared by the command line (host and container side)
//! and the terminal UI, one helper group per module.

pub mod appenv;
pub mod composer;
pub mod contrib;
pub mod ctx;
pub mod db;
pub mod ddev;
pub mod fpm;
pub mod gerrit;
pub mod git;
pub mod kind;
pub mod out;
pub mod patch;
pub mod php;
pub mod phpjson;
pub mod poststart;
pub mod proc;
pub mod prompt;
pub mod review;
pub mod schema;
pub mod serve;
pub mod site;
pub mod status;
pub mod types;
pub mod vsort;
pub mod webserver;
pub mod worktree;

/// A step failed and has said why: its message is already printed. What the
/// caller does with it — stop, fall back, exit 1 — is the caller's decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failed;

/// The outcome of a step that reports its own failure.
pub type Step = Result<(), Failed>;

/// Print each line as an error and fail: the step has said why.
pub fn fail<S: AsRef<str>>(lines: impl IntoIterator<Item = S>) -> Failed {
    for l in lines {
        out::error(l);
    }
    Failed
}

/// The payload version (tryout/VERSION), which the install stamps into
/// .ddev/tryout/.version and `status` compares: one number, read by both.
pub const PAYLOAD_VERSION_FILE: &str = include_str!("../../../tryout/VERSION");

/// The number in tryout/VERSION: its one line that is only digits.
pub fn payload_version() -> &'static str {
    PAYLOAD_VERSION_FILE
        .lines()
        .find(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()))
        .unwrap_or("0")
}

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
    fn the_payload_version_is_the_number_in_tryout_version() {
        assert!(
            super::payload_version()
                .parse::<u32>()
                .is_ok_and(|v| v >= 49)
        );
    }
}
