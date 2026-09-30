//! Sessions: a server that owns everything and outlives its clients, as tmux
//! does. `tryout ui` attaches to the project's session, starting it when none
//! answers; `q` detaches and leaves it running; only an explicit close (`Q`, or
//! `tryout ui stop`) ends it.

pub mod client;
pub mod proto;
pub mod server;
pub mod wire;

use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
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

/// Where a server notes the build it runs, beside its socket.
pub fn build_path(root: &Path) -> PathBuf {
    socket_path(root).with_extension("build")
}

/// This binary's build, as its file on disk says: size and modification time.
/// A rebuild or an add-on update changes it; a running server keeps the one it
/// started with, because a process keeps the code it was started from.
pub fn build_stamp() -> String {
    std::env::current_exe()
        .and_then(std::fs::metadata)
        .map(|m| {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .unwrap_or_default();
            format!("{}-{}.{:09}", m.len(), t.as_secs(), t.subsec_nanos())
        })
        .unwrap_or_default()
}

/// Is a session running on another build than this binary? A server too old
/// to note its build counts as another. False when nothing is running.
pub fn runs_another_build(root: &Path) -> bool {
    if !socket_path(root).exists() {
        return false;
    }
    std::fs::read_to_string(build_path(root)).ok().as_deref() != Some(build_stamp().as_str())
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

/// The socket directory, made if missing — and refused unless it is a real
/// directory owned by this user and closed to everyone else. On Linux it sits
/// in the shared /tmp: another user could create it first and put a server of
/// their own there, which would then be sent every key typed, passwords
/// included.
fn private_socket_dir() -> Result<PathBuf> {
    let dir = socket_dir();
    make_private(&dir)?;
    Ok(dir)
}

fn make_private(dir: &Path) -> Result<()> {
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e).with_context(|| format!("cannot create {}", dir.display())),
    }
    let m = std::fs::symlink_metadata(dir)
        .with_context(|| format!("cannot inspect {}", dir.display()))?;
    // SAFETY: getuid cannot fail and has no preconditions.
    let uid = unsafe { libc::getuid() };
    if !m.file_type().is_dir() || m.uid() != uid {
        bail!(
            "{} is not a directory of yours — refusing to use it\n  → remove it, or set TMPDIR to a private directory",
            dir.display()
        );
    }
    if m.mode() & 0o077 != 0 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Connect to the project's session, starting it first if none is running.
pub fn connect_or_start(root: &Path) -> Result<UnixStream> {
    private_socket_dir()?;
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

/// Start `tryout ui server <root>` detached: its own session (so closing this
/// terminal does not hang it up), no terminal, output to the log.
fn start_server(root: &Path) -> Result<()> {
    private_socket_dir()?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(root))?;
    let exe = std::env::current_exe().context("cannot find this program's path")?;
    let mut cmd = Command::new(exe);
    cmd.arg("ui")
        .arg("server")
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
    private_socket_dir()?;
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
    fn the_socket_directory_must_be_a_private_directory_of_ours() {
        let t = tempfile::tempdir().unwrap();
        let fresh = t.path().join("fresh");
        make_private(&fresh).unwrap();
        let mode = std::fs::metadata(&fresh).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o700);

        let open = t.path().join("open");
        std::fs::create_dir(&open).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o777)).unwrap();
        make_private(&open).unwrap();
        assert_eq!(std::fs::metadata(&open).unwrap().mode() & 0o777, 0o700);

        // A symlink could point anywhere, someone else's directory included.
        let link = t.path().join("link");
        std::os::unix::fs::symlink(&fresh, &link).unwrap();
        assert!(make_private(&link).is_err());
        let file = t.path().join("file");
        std::fs::write(&file, "").unwrap();
        assert!(make_private(&file).is_err());
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
    fn a_session_is_on_another_build_when_its_note_differs_or_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proj-build-test");
        std::fs::create_dir_all(socket_dir()).unwrap();
        let (sock, build) = (socket_path(&root), build_path(&root));
        assert!(!runs_another_build(&root), "no session, nothing to compare");
        std::fs::write(&sock, "").unwrap();
        assert!(
            runs_another_build(&root),
            "a server too old to note its build"
        );
        std::fs::write(&build, "1-2.000000003").unwrap();
        assert!(runs_another_build(&root));
        std::fs::write(&build, build_stamp()).unwrap();
        assert!(!runs_another_build(&root));
        let _ = std::fs::remove_file(sock);
        let _ = std::fs::remove_file(build);
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
