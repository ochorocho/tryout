//! The completion as DDEV runs it: a separate process, whatever the project.

use std::process::Command;

#[test]
fn completion_never_writes_to_stderr_and_never_fails() {
    for root in ["/nonexistent", "/", env!("CARGO_MANIFEST_DIR")] {
        for line in [
            &["tryout", "''"][..],
            &["tryout", "worktree", "serve", "''"],
            &["tryout"],
            &[],
        ] {
            let out = Command::new(env!("CARGO_BIN_EXE_tryout"))
                .arg("__complete")
                .args(line)
                .env("DDEV_APPROOT", root)
                .current_dir("/")
                .output()
                .unwrap();
            assert!(out.status.success(), "{root} {line:?}");
            assert!(
                out.stderr.is_empty(),
                "{root} {line:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}
