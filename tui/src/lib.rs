//! tryout: the DDEV add-on's command line, container side and terminal UI in one
//! binary. `ddev tryout …` on the host, `tryout ctr …` inside the web container.

pub mod cli;
pub mod core;
pub mod tui;

/// DDEV decides whether it owns a file by grepping it for this marker, so it has
/// to survive into the binary: `#[used]` keeps it past dead-code elimination.
#[used]
#[unsafe(no_mangle)]
pub static DDEV_MARKER: [u8; 16] = *b"#ddev-generated\n";
