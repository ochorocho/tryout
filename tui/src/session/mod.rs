//! Sessions: a server that owns everything and outlives its clients, as herdr
//! and tmux do. `tryout-tui` attaches to the project's session, starting it
//! when none answers; `q` detaches and leaves it running; only an explicit
//! close (`Q`, or `tryout-tui stop`) ends it.

pub mod client;
pub mod proto;
pub mod server;
pub mod wire;

use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

/// Set in every shell and command a session starts: the session's socket.
pub const SESSION_ENV: &str = "TRYOUT_TUI_SESSION";

/// True when `inside` (the value of SESSION_ENV here) is this project's own
/// session: attaching from within it would draw the session inside itself.
pub fn is_inside(root: &Path, inside: Option<&std::ffi::OsStr>) -> bool {
    inside.is_some_and(|s| Path::new(s) == socket_path(root))
}

/// How long a freshly started server gets to open its socket.
const START_TIMEOUT: Duration = Duration::from_secs(5);

/// FNV-1a: a hash that is the same in every build. The socket's name comes
/// from it, and std's hasher may change between Rust releases — which would
/// leave a running session unreachable after an upgrade.
fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The directory for this user's session sockets, private to them.
fn socket_dir() -> PathBuf {
    // SAFETY: getuid cannot fail and has no preconditions.
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir().join(format!("tryout-tui-{uid}"))
}

/// The project's session socket. Short on purpose: a Unix socket path is
/// limited to about 104 bytes on macOS, and TMPDIR there is already ~50.
pub fn socket_path(root: &Path) -> PathBuf {
    let name: String = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(16)
        .collect();
    let hash = stable_hash(root.as_os_str().as_encoded_bytes());
    socket_dir().join(format!("{name}-{:08x}.sock", hash as u32))
}

/// Where the server writes what it would otherwise have no terminal to say.
pub fn log_path(root: &Path) -> PathBuf {
    socket_path(root).with_extension("log")
}

/// Is a session answering there? A socket file whose server is gone is removed,
/// so a fresh one can take its place.
pub fn connect(socket: &Path) -> io::Result<Option<UnixStream>> {
    match UnixStream::connect(socket) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            let _ = std::fs::remove_file(socket);
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Connect to the project's session, starting it first if none is running.
pub fn connect_or_start(root: &Path) -> Result<UnixStream> {
    let socket = socket_path(root);
    if let Some(s) = connect(&socket)? {
        return Ok(s);
    }
    start_server(root)?;
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(s) = connect(&socket)? {
            return Ok(s);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    bail!(
        "the session server did not start\n  → see {}",
        log_path(root).display()
    )
}

/// Start `tryout-tui server <root>` detached: its own session (so closing this
/// terminal does not hang it up), no terminal, output to the log.
fn start_server(root: &Path) -> Result<()> {
    let dir = socket_dir();
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .with_context(|| format!("cannot create {}", dir.display()))?;
    // An existing directory keeps its mode; insist on ours.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(root))?;
    let exe = std::env::current_exe().context("cannot find this program's path")?;
    let mut cmd = Command::new(exe);
    cmd.arg("server")
        .arg(root)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    // SAFETY: setsid is async-signal-safe, which is all pre_exec requires.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    cmd.spawn().context("cannot start the session server")?;
    Ok(())
}

/// End the project's session. False when there was none.
pub fn stop(root: &Path) -> Result<bool> {
    let Some(mut s) = connect(&socket_path(root))? else {
        return Ok(false);
    };
    proto::write_msg(&mut s, &proto::ClientMsg::Stop)?;
    // Wait for the server to say goodbye, so `stop` returns once it is done.
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = proto::read_msg::<_, proto::ServerMsg>(&mut s);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_socket_is_short_per_user_and_per_project() {
        let long = Path::new("/Users/someone/Development/a-very-long-project-name-indeed-yes");
        let p = socket_path(long);
        assert!(
            p.as_os_str().len() < 100,
            "{} is too long for a socket",
            p.display()
        );
        assert!(p.starts_with(socket_dir()));
        assert_ne!(
            p,
            socket_path(Path::new("/elsewhere/a-very-long-project-name-indeed-yes"))
        );
        assert_eq!(
            p,
            socket_path(long),
            "the same project always gets the same socket"
        );
    }

    #[test]
    fn attaching_from_inside_the_same_session_is_caught() {
        let root = Path::new("/p/demo");
        let own = socket_path(root);
        assert!(is_inside(root, Some(own.as_os_str())));
        assert!(!is_inside(root, None));
        let other = socket_path(Path::new("/p/other"));
        assert!(
            !is_inside(root, Some(other.as_os_str())),
            "another project's session is fine"
        );
    }

    #[test]
    fn the_hash_is_the_same_in_every_build() {
        // Pinned: a change here strands every running session after an upgrade.
        assert_eq!(stable_hash(b"/p/demo"), 0x52b3_1cb2_129c_40d4);
    }

    #[test]
    fn a_socket_nobody_listens_on_is_cleared_away() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("s.sock");
        drop(std::os::unix::net::UnixListener::bind(&sock).unwrap()); // file stays, server gone
        assert!(sock.exists());
        assert!(connect(&sock).unwrap().is_none());
        assert!(!sock.exists(), "a stale socket would block the next server");
    }
}
