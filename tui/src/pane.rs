//! A live terminal pane: a program on a real PTY, its screen emulated by vt100.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Lines of scrollback each pane keeps.
const SCROLLBACK: usize = 5000;

pub struct Pane {
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    alive: Arc<AtomicBool>,
    /// Set by the reader whenever output arrived, so the UI redraws only then.
    dirty: Arc<AtomicBool>,
    /// The program's exit code, once it has one.
    exit_code: Arc<Mutex<Option<u32>>>,
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

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK)));
        let alive = Arc::new(AtomicBool::new(true));
        let dirty = Arc::new(AtomicBool::new(true));
        let exit_code = Arc::new(Mutex::new(None));
        let mut reader = pair.master.try_clone_reader()?;
        {
            let (parser, alive, dirty, code) = (
                parser.clone(),
                alive.clone(),
                dirty.clone(),
                exit_code.clone(),
            );
            thread::spawn(move || {
                let mut buf = [0u8; 8192];
                // EOF or an error both mean the program is gone.
                while let Ok(n @ 1..) = reader.read(&mut buf) {
                    parser.lock().unwrap().process(&buf[..n]);
                    dirty.store(true, Ordering::Release);
                }
                // The code is stored BEFORE alive flips, so whoever sees the
                // program gone also sees how it ended.
                let status = child.wait().map(|s| s.exit_code()).unwrap_or(1);
                *code.lock().unwrap() = Some(status);
                alive.store(false, Ordering::Release);
                dirty.store(true, Ordering::Release);
            });
        }
        let writer = pair.master.take_writer()?;
        Ok(Self {
            parser,
            writer,
            master: pair.master,
            alive,
            dirty,
            exit_code,
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

    /// How the program ended; None while it runs.
    pub fn exit_code(&self) -> Option<u32> {
        if self.is_alive() {
            None
        } else {
            *self.exit_code.lock().unwrap()
        }
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// True once per batch of output: the caller redraws, and the flag clears.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
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
    use std::time::{Duration, Instant};

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

    #[test]
    fn the_exit_code_is_kept() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "exit 3"]);
        let pane = Pane::spawn(cmd, 5, 20).unwrap();
        assert!(wait_for(&pane, |p| p.exit_code().is_some()));
        assert_eq!(pane.exit_code(), Some(3));
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
