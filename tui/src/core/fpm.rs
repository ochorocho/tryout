//! One PHP-FPM master per extra PHP version, for served sites on a PHP other
//! than the project's. DDEV's own master serves the project's version.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::ctx::Ctx;
use super::out;
use super::{Failed, Step};

fn run_dir() -> PathBuf {
    PathBuf::from(std::env::var("TRYOUT_FPM_RUN_DIR").unwrap_or_else(|_| "/run/php".into()))
}

/// `tryout __fpm <version>`: write the pool config and become php-fpm, in the
/// foreground, so the supervisor (DDEV's web_extra_daemons) owns the process.
/// Exit 64 without a version, 69 without that PHP, 73 when /run/php is not
/// writable. Returns only on failure.
pub fn serve(version: &str) -> i32 {
    if version.is_empty() {
        eprintln!("tryout-php-fpm: missing PHP version argument");
        return 64;
    }
    let bin = format!("/usr/sbin/php-fpm{version}");
    if !is_executable(&bin) {
        eprintln!("tryout-php-fpm: {bin} not found — PHP {version} is not installed in this image");
        return 69;
    }
    let run = run_dir();
    let socket = run.join(format!("php-fpm-{version}.sock"));
    let conf_dir = PathBuf::from("/tmp/tryout-fpm");
    let conf = conf_dir.join(format!("php-fpm-{version}.conf"));
    let pool = format!("tryout{}", version.replace('.', ""));
    let _ = std::fs::create_dir_all(&conf_dir);
    let body = format!(
        "[global]\npid = {}/php-fpm-{version}.pid\nerror_log = /proc/self/fd/2\ndaemonize = no\n\n\
[{pool}]\nlisten = {}\nlisten.mode = 0666\npm = dynamic\npm.max_children = 10\npm.start_servers = 2\n\
pm.min_spare_servers = 1\npm.max_spare_servers = 3\nclear_env = no\n\
php_admin_value[error_log] = /proc/self/fd/2\nphp_admin_flag[log_errors] = on\n",
        run.display(),
        socket.display()
    );
    if std::fs::write(&conf, body).is_err() {
        eprintln!("tryout-php-fpm: cannot write {}", conf.display());
        return 73;
    }
    let probe = run.join(".tryout-write-probe");
    if std::fs::write(&probe, "").is_err() {
        eprintln!(
            "tryout-php-fpm: {} is not writable — cannot create {}",
            run.display(),
            socket.display()
        );
        return 73;
    }
    let _ = std::fs::remove_file(probe);
    // The master binds a fresh socket; a stale one from a dead master is ours.
    let _ = std::fs::remove_file(&socket);
    println!(
        "tryout-php-fpm: starting PHP {version} on {} (pool {pool})",
        socket.display()
    );
    let err = Command::new(&bin)
        .arg("--nodaemonize")
        .arg("--fpm-config")
        .arg(&conf)
        .exec();
    eprintln!("tryout-php-fpm: cannot start {bin}: {err}");
    1
}

fn is_executable(p: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Is the master for `version` up? A live pid in its pid file — which FPM
/// writes only once the socket is bound, so it doubles as readiness.
fn alive(version: &str) -> bool {
    let pid: i32 = std::fs::read_to_string(run_dir().join(format!("php-fpm-{version}.pid")))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    // SAFETY: kill with signal 0 only checks that the process exists.
    pid > 0 && unsafe { libc::kill(pid, 0) } == 0
}

/// Start the master for a non-default PHP unless one runs. DDEV bakes
/// web_extra_daemons into the image, so a version first served after the
/// container started has none until `ddev restart` — and a vhost reloaded in
/// place would route to a socket nobody listens on. Never a second master
/// beside a live one: it would steal the first one's socket.
pub fn ensure_running(ctx: &Ctx, version: &str) -> Step {
    if version == ctx.env.php_version || alive(version) {
        return Ok(());
    }
    let _ = std::fs::remove_file(run_dir().join(format!("php-fpm-{version}.pid")));
    let log = std::env::temp_dir().join(format!("tryout-php-fpm-{version}.log"));
    out::info(format!("Starting PHP {version} FPM..."));
    let started = std::fs::File::create(&log).ok().and_then(|f| {
        let err = f.try_clone().ok()?;
        let exe = std::env::current_exe().ok()?;
        let mut c = Command::new(exe);
        c.arg("__fpm")
            .arg(version)
            .stdin(Stdio::null())
            .stdout(f)
            .stderr(err);
        // Its own session: it outlives the `ddev exec` that started it.
        // SAFETY: setsid in the child, before exec, touches no shared state.
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        c.spawn().ok()
    });
    if started.is_some() {
        for _ in 0..10 {
            if alive(version) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    out::error(format!("PHP {version} FPM did not start:"));
    let tail: Vec<String> = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect();
    for l in tail.iter().skip(tail.len().saturating_sub(5)) {
        eprintln!("{l}");
    }
    out::error("  → ddev restart");
    Err(Failed)
}
