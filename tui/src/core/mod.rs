//! The add-on's own logic, shared by the command line (host and container side)
//! and the terminal UI. Ported from tryout/functions.sh one helper group per
//! module.

pub mod ctx;
pub mod git;
pub mod out;
pub mod php;
pub mod phpjson;
pub mod site;
pub mod vsort;
pub mod worktree;
