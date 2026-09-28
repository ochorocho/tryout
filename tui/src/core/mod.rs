//! The add-on's own logic, shared by the command line (host and container side)
//! and the terminal UI. Ported from tryout/functions.sh one helper group per
//! module.

pub mod contrib;
pub mod ctx;
pub mod ddev;
pub mod gerrit;
pub mod git;
pub mod out;
pub mod php;
pub mod phpjson;
pub mod prompt;
pub mod site;
pub mod status;
pub mod vsort;
pub mod worktree;

/// The payload version the add-on stamps into .ddev/tryout/.version, which
/// `status` compares. Bump it whenever what a user sees changes.
pub const PAYLOAD_VERSION: &str = "48";

#[cfg(test)]
mod tests {
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
