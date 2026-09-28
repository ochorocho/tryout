//! A live terminal pane: a program on a real PTY, its screen emulated by vt100.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Lines of scrollback each pane keeps.
const SCROLLBACK: usize = 5000;

/// What a program says about itself besides drawing: the title it gives its
/// terminal (how an agent names its task) and the bell (how it asks for you).
#[derive(Default)]
struct Signals {
    title: Option<String>,
    bell: bool,
}

impl vt100::Callbacks for Signals {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        let title = String::from_utf8_lossy(title).trim().to_string();
        self.title = (!title.is_empty()).then_some(title);
    }

    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.bell = true;
    }
}

/// When output last arrived, and whether any has since the pane was last looked at.
struct Activity {
    last_output: Instant,
    unseen: bool,
}

pub struct Pane {
    parser: Arc<Mutex<vt100::Parser<Signals>>>,
    activity: Arc<Mutex<Activity>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    alive: Arc<AtomicBool>,
    /// Set by the reader whenever output arrived, so the UI redraws only then.
    dirty: Arc<AtomicBool>,
    size: (u16, u16),
}

impl Pane {
    /// The user's shell in `cwd`.
    pub fn shell(cwd: &Path, rows: u16, cols: u16) -> Result<Self> {
        let mut cmd = CommandBuilder::new_default_prog();
        cmd.cwd(cwd);
        Self::spawn(cmd, rows, cols)
    }

    pub fn spawn(mut cmd: CommandBuilder, rows: u16, cols: u16) -> Result<Self> {
        // The pane IS a terminal; say which kind, whatever launched us.
        cmd.env("TERM", "xterm-256color");
        let pair = native_pty_system()
            .openpty(pty_size(rows, cols))
            .context("could not open a pseudo-terminal")?;
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .context("could not start the program")?;
        drop(pair.slave);

        let parser = Arc::new(Mutex::new(vt100::Parser::new_with_callbacks(
            rows,
            cols,
            SCROLLBACK,
            Signals::default(),
        )));
        let activity = Arc::new(Mutex::new(Activity {
            last_output: Instant::now(),
            unseen: false,
        }));
        let alive = Arc::new(AtomicBool::new(true));
        let dirty = Arc::new(AtomicBool::new(true));
        let mut reader = pair.master.try_clone_reader()?;
        {
            let (parser, activity, alive, dirty) = (
                parser.clone(),
                activity.clone(),
                alive.clone(),
                dirty.clone(),
            );
            thread::spawn(move || {
                let mut buf = [0u8; 8192];
                // EOF or an error both mean the program is gone.
                while let Ok(n @ 1..) = reader.read(&mut buf) {
                    parser.lock().unwrap().process(&buf[..n]);
                    {
                        let mut a = activity.lock().unwrap();
                        a.last_output = Instant::now();
                        a.unseen = true;
                    }
                    dirty.store(true, Ordering::Release);
                }
                let _ = child.wait();
                alive.store(false, Ordering::Release);
                dirty.store(true, Ordering::Release);
            });
        }
        let writer = pair.master.take_writer()?;
        Ok(Self {
            parser,
            activity,
            writer,
            master: pair.master,
            alive,
            dirty,
            size: (rows, cols),
        })
    }

    pub fn write(&mut self, bytes: &[u8]) {
        if self.is_alive() {
            let _ = self
                .writer
                .write_all(bytes)
                .and_then(|_| self.writer.flush());
        }
    }

    /// Resize both sides — the PTY, so the program reflows, and the emulator.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        if (rows, cols) == self.size || rows == 0 || cols == 0 {
            return;
        }
        self.size = (rows, cols);
        let _ = self.master.resize(pty_size(rows, cols));
        self.parser
            .lock()
            .unwrap()
            .screen_mut()
            .set_size(rows, cols);
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// True once per batch of output: the caller redraws, and the flag clears.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    /// The title the program gave its terminal, if any.
    pub fn title(&self) -> Option<String> {
        self.parser.lock().unwrap().callbacks().title.clone()
    }

    /// The program rang the bell since the pane was last looked at.
    pub fn bell_pending(&self) -> bool {
        self.parser.lock().unwrap().callbacks().bell
    }

    /// Output arrived since the pane was last looked at.
    pub fn unseen_output(&self) -> bool {
        self.activity.lock().unwrap().unseen
    }

    pub fn since_output(&self) -> Duration {
        self.activity.lock().unwrap().last_output.elapsed()
    }

    /// Someone is looking: clear what was waiting for their attention.
    pub fn mark_seen(&self) {
        self.parser.lock().unwrap().callbacks_mut().bell = false;
        self.activity.lock().unwrap().unseen = false;
    }

    /// The process in the foreground on this terminal — the job a shell is
    /// running, or the shell itself at its prompt.
    pub fn foreground_pid(&self) -> Option<u32> {
        self.master.process_group_leader().map(|p| p as u32)
    }

    /// Run `f` against the current screen, under the parser's lock.
    pub fn with_screen<R>(&self, f: impl FnOnce(&vt100::Screen) -> R) -> R {
        f(self.parser.lock().unwrap().screen())
    }
}

fn pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for(pane: &Pane, what: impl Fn(&Pane) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if what(pane) {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn output_reaches_the_screen_and_exit_is_noticed() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "printf 'hello from the pty'"]);
        let pane = Pane::spawn(cmd, 10, 40).unwrap();
        assert!(wait_for(&pane, |p| p
            .with_screen(|s| s.contents().contains("hello from the pty"))));
        assert!(wait_for(&pane, |p| !p.is_alive()), "exit was never noticed");
    }

    fn sh(script: &str) -> Pane {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", script]);
        Pane::spawn(cmd, 10, 40).unwrap()
    }

    #[test]
    fn a_program_names_its_tab_through_the_terminal_title() {
        let pane = sh("printf '\\033]2;claude: fix tests\\007'; sleep 2");
        assert!(wait_for(&pane, |p| p.title().as_deref() == Some("claude: fix tests")));
    }

    #[test]
    fn a_bell_is_pending_until_seen() {
        let pane = sh("printf 'done\\007'; sleep 2");
        assert!(wait_for(&pane, |p| p.bell_pending()));
        assert!(pane.unseen_output());
        pane.mark_seen();
        assert!(!pane.bell_pending() && !pane.unseen_output());
    }

    #[test]
    fn output_is_timestamped() {
        let pane = sh("printf x; sleep 2");
        assert!(wait_for(&pane, |p| p.unseen_output()));
        assert!(pane.since_output() < Duration::from_secs(2));
    }

    #[test]
    fn the_foreground_process_is_the_job_not_the_shell() {
        // An interactive shell puts a job in its own process group; that group's
        // leader is what the agents pane must look at.
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-i");
        let mut pane = Pane::spawn(cmd, 10, 40).unwrap();
        thread::sleep(Duration::from_millis(300));
        pane.write(b"sleep 5\r");
        let is_sleep = |p: &Pane| {
            p.foreground_pid().is_some_and(|pid| {
                std::process::Command::new("ps")
                    .args(["-o", "comm=", "-p", &pid.to_string()])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().ends_with("sleep"))
                    .unwrap_or(false)
            })
        };
        assert!(
            wait_for(&pane, is_sleep),
            "the foreground job was never `sleep`"
        );
    }

    #[test]
    fn input_is_delivered_and_the_program_sees_the_size() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "read line; echo \"got:$line\"; stty size"]);
        let mut pane = Pane::spawn(cmd, 12, 50).unwrap();
        pane.write(b"ping\r");
        assert!(wait_for(&pane, |p| p
            .with_screen(|s| s.contents().contains("got:ping"))));
        assert!(wait_for(&pane, |p| p.with_screen(|s| s.contents().contains("12 50"))));
    }
}
