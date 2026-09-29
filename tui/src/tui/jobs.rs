//! Commands as background jobs: `ddev tryout …` run fully argued, one at a time,
//! their progress read from the add-on's `@@tryout` event lines (TRYOUT_EVENTS=1)
//! and the rest kept as a log. The UI asks its questions before a job exists,
//! so tryout itself never waits on input.
//!
//! One prompt can still come: DDEV runs `sudo` to add a hostname to /etc/hosts,
//! and sudo reads the password from a terminal. So a job runs on a
//! pseudo-terminal, a password prompt on it is reported as `waiting`, and the UI
//! answers it (`answer`) or cancels it (`cancel_prompt`). Echo is off on that
//! terminal: nothing typed into it ever reaches the log.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::tui::actions::Action;

/// How many finished jobs the Activity panel keeps.
const KEEP_DONE: usize = 5;
/// A log longer than this keeps its tail: a composer install can run to
/// thousands of lines, and it is the end that says what went wrong.
const LOG_LINES: usize = 3000;
/// The add-on's event marker, see `tryout_event` in functions.sh.
const MARKER: &str = "@@tryout ";

#[derive(Debug, Clone, PartialEq)]
pub enum JobState {
    Queued,
    Running {
        step: Option<String>,
        since: Instant,
    },
    Done {
        ok: bool,
        code: Option<i32>,
        took: Duration,
    },
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: u64,
    pub label: String,
    pub args: Vec<String>,
    pub state: JobState,
    /// Output lines as the command printed them, colours and all.
    pub log: VecDeque<String>,
    /// The last error the command reported, for the failure notice.
    pub last_error: Option<String>,
    /// The list shows what this changes: reload it when done.
    pub changes_worktrees: bool,
    /// The output is the point (status, exec): show it when done.
    pub reveal: bool,
    /// A password prompt it is stopped at, answered through the UI.
    pub waiting: Option<String>,
}

impl Job {
    pub fn command_line(&self) -> String {
        format!("ddev tryout {}", self.args.join(" "))
    }
}

/// What the runner threads report.
enum Report {
    Line(u64, String),
    /// An unfinished line that asks for a password.
    Prompt(u64, String),
    Exit(u64, Option<i32>),
}

#[derive(Deserialize)]
struct Event {
    level: String,
    msg: String,
}

pub struct Jobs {
    /// Running first, then queued, then finished newest first — the order the
    /// Activity panel shows them in.
    list: Vec<Job>,
    next_id: u64,
    program: PathBuf,
    tx: Sender<Report>,
    rx: Receiver<Report>,
    /// Each running job's terminal, for answering its prompt.
    writers: HashMap<u64, Box<dyn Write + Send>>,
}

impl Jobs {
    /// Jobs that run `program tryout …` — `ddev`, or a stand-in under test.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            list: Vec::new(),
            next_id: 1,
            program: program.into(),
            tx,
            rx,
            writers: HashMap::new(),
        }
    }

    /// The first job stopped at a password prompt: (id, prompt, label).
    pub fn waiting(&self) -> Option<(u64, String, String)> {
        self.list.iter().find_map(|j| {
            j.waiting
                .as_ref()
                .map(|p| (j.id, p.clone(), j.label.clone()))
        })
    }

    /// Send the password (and Enter) to the job's terminal. It goes nowhere
    /// else: not the log, not the events.
    pub fn answer(&mut self, id: u64, secret: &str) {
        self.write(id, format!("{secret}\r").as_bytes());
    }

    /// Decline the prompt: Ctrl-C, so the job fails the way it would at a
    /// terminal.
    pub fn cancel_prompt(&mut self, id: u64) {
        self.write(id, b"\x03");
    }

    fn write(&mut self, id: u64, bytes: &[u8]) {
        if let Some(w) = self.writers.get_mut(&id) {
            let _ = w.write_all(bytes);
            let _ = w.flush();
        }
        if let Some(job) = self.list.iter_mut().find(|j| j.id == id) {
            job.waiting = None;
        }
    }

    pub fn list(&self) -> &[Job] {
        &self.list
    }

    pub fn get(&self, id: u64) -> Option<&Job> {
        self.list.iter().find(|j| j.id == id)
    }

    pub fn running(&self) -> bool {
        self.list
            .iter()
            .any(|j| matches!(j.state, JobState::Running { .. }))
    }

    /// Queue a command. It starts on the next `tick` if nothing else runs.
    pub fn enqueue(&mut self, action: &Action, reveal: bool) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.list.push(Job {
            id,
            label: action.label.trim().to_string(),
            args: action.args.clone(),
            state: JobState::Queued,
            log: VecDeque::new(),
            last_error: None,
            changes_worktrees: action.changes_worktrees(),
            reveal,
            waiting: None,
        });
        self.sort();
        id
    }

    /// The newest job that failed, if the newest finished one did.
    pub fn last_failed(&self) -> Option<u64> {
        self.list
            .iter()
            .filter(|j| matches!(j.state, JobState::Done { .. }))
            .max_by_key(|j| j.id)
            .filter(|j| matches!(j.state, JobState::Done { ok: false, .. }))
            .map(|j| j.id)
    }

    /// Queue a finished job's command again, as a new job. None while the job
    /// is still queued or running.
    pub fn retry(&mut self, id: u64) -> Option<u64> {
        let job = self.get(id)?;
        if !matches!(job.state, JobState::Done { .. }) {
            return None;
        }
        let (label, args, reveal, changes) = (
            job.label.clone(),
            job.args.clone(),
            job.reveal,
            job.changes_worktrees,
        );
        let new = self.next_id;
        self.next_id += 1;
        self.list.push(Job {
            id: new,
            label,
            args,
            state: JobState::Queued,
            log: VecDeque::new(),
            last_error: None,
            changes_worktrees: changes,
            reveal,
            waiting: None,
        });
        self.sort();
        Some(new)
    }

    /// Take in what the runners reported, start the next job when none runs,
    /// and hand back the ones that just finished.
    pub fn tick(&mut self, cwd: &Path) -> Vec<Job> {
        let mut finished = Vec::new();
        while let Ok(report) = self.rx.try_recv() {
            match report {
                Report::Line(id, line) => {
                    if let Some(job) = self.list.iter_mut().find(|j| j.id == id) {
                        // Output after a prompt means it moved on.
                        job.waiting = None;
                        take_line(job, line);
                    }
                }
                Report::Prompt(id, prompt) => {
                    if let Some(job) = self.list.iter_mut().find(|j| j.id == id) {
                        job.waiting = Some(prompt);
                    }
                }
                Report::Exit(id, code) => {
                    self.writers.remove(&id);
                    if let Some(job) = self.list.iter_mut().find(|j| j.id == id) {
                        let took = match job.state {
                            JobState::Running { since, .. } => since.elapsed(),
                            _ => Duration::ZERO,
                        };
                        job.waiting = None;
                        job.state = JobState::Done {
                            ok: code == Some(0),
                            code,
                            took,
                        };
                        finished.push(job.clone());
                    }
                }
            }
        }
        if !self.running()
            && let Some(job) = self.list.iter_mut().find(|j| j.state == JobState::Queued)
        {
            job.state = JobState::Running {
                step: None,
                since: Instant::now(),
            };
            if let Some(w) = spawn(&self.program, cwd, job.id, &job.args, &self.tx) {
                self.writers.insert(job.id, w);
            }
        }
        if !finished.is_empty() {
            self.sort();
            self.trim();
        }
        finished
    }

    fn sort(&mut self) {
        let rank = |j: &Job| match j.state {
            JobState::Running { .. } => 0,
            JobState::Queued => 1,
            JobState::Done { .. } => 2,
        };
        // Stable: queued keep their order; finished newest first by id.
        self.list.sort_by(|a, b| {
            rank(a).cmp(&rank(b)).then_with(|| match rank(a) {
                2 => b.id.cmp(&a.id),
                _ => a.id.cmp(&b.id),
            })
        });
    }

    fn trim(&mut self) {
        let mut done = 0;
        self.list.retain(|j| {
            if matches!(j.state, JobState::Done { .. }) {
                done += 1;
                done <= KEEP_DONE
            } else {
                true
            }
        });
    }
}

/// A line from a job: an event updates its step, anything else is log.
fn take_line(job: &mut Job, line: String) {
    if let Some(ev) = line
        .strip_prefix(MARKER)
        .and_then(|j| serde_json::from_str::<Event>(j).ok())
    {
        let msg = ev.msg.trim().to_string();
        match ev.level.as_str() {
            "error" => {
                // "  → …" is the next step an error names, not the error itself.
                if !msg.starts_with('→') {
                    job.last_error = Some(msg);
                }
            }
            _ => {
                if let JobState::Running { step, .. } = &mut job.state {
                    *step = Some(msg);
                }
            }
        }
        return;
    }
    job.log.push_back(line);
    while job.log.len() > LOG_LINES {
        job.log.pop_front();
    }
}

/// Does this unfinished line ask for a password? sudo's "Password:",
/// "[sudo] password for jo:" and macOS's "Password for jo:" all do.
pub fn is_password_prompt(line: &str) -> bool {
    let t = crate::tui::worktrees::strip_ansi(line)
        .trim()
        .to_lowercase();
    t.ends_with(':') && t.contains("password")
}

/// Run `program tryout <args>` on a pseudo-terminal (echo off), its output read
/// line by line — an unfinished line that asks for a password reported as a
/// prompt — and the exit code reported last. Hands back the terminal's writer.
fn spawn(
    program: &Path,
    cwd: &Path,
    id: u64,
    args: &[String],
    tx: &Sender<Report>,
) -> Option<Box<dyn Write + Send>> {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    let fail = |why: String| {
        let _ = tx.send(Report::Line(id, why));
        let _ = tx.send(Report::Exit(id, None));
        None
    };
    let pty = match native_pty_system().openpty(PtySize {
        rows: 40,
        cols: 160,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(p) => p,
        Err(e) => return fail(format!("could not open a terminal for the job: {e}")),
    };
    no_echo(pty.master.as_raw_fd());
    let mut cmd = CommandBuilder::new(program);
    cmd.arg("tryout");
    cmd.args(args);
    cmd.cwd(cwd);
    cmd.env("TRYOUT_EVENTS", "1");
    let mut child = match pty.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(e) => return fail(format!("could not start {}: {e}", program.display())),
    };
    // Only the child holds the terminal's other end now, so its exit ends the
    // reader.
    drop(pty.slave);
    let reader = pty.master.try_clone_reader().ok()?;
    let writer = pty.master.take_writer().ok()?;

    let lines = {
        let tx = tx.clone();
        thread::spawn(move || read_lines(reader, id, &tx))
    };
    let tx = tx.clone();
    let master = pty.master;
    thread::spawn(move || {
        let code = child.wait().ok().map(|s| s.exit_code() as i32);
        // Every line is in before the exit is reported — unless something the
        // job left running still holds the terminal; then do not wait on it.
        let deadline = Instant::now() + Duration::from_millis(1500);
        while !lines.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        drop(master);
        let _ = tx.send(Report::Exit(id, code));
    });
    Some(writer)
}

/// Echo off on a job's terminal: a password typed into it must not come back
/// as output. sudo turns it off itself; this covers anything that does not.
fn no_echo(fd: Option<std::os::fd::RawFd>) {
    let Some(fd) = fd else { return };
    // SAFETY: tcgetattr/tcsetattr on a terminal descriptor this process owns,
    // with a termios struct it fills first.
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) == 0 {
            t.c_lflag &= !(libc::ECHO | libc::ECHONL);
            libc::tcsetattr(fd, libc::TCSANOW, &t);
        }
    }
}

fn read_lines(mut reader: Box<dyn Read + Send>, id: u64, tx: &Sender<Report>) {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut asked = false;
    loop {
        let n = match reader.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        while let Some(i) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=i).collect();
            let text = String::from_utf8_lossy(&line[..line.len() - 1])
                .trim_end_matches('\r')
                .to_string();
            let _ = tx.send(Report::Line(id, text));
            asked = false;
        }
        let partial = String::from_utf8_lossy(&buf).into_owned();
        if !asked && is_password_prompt(&partial) {
            asked = true;
            let _ = tx.send(Report::Prompt(
                id,
                crate::tui::worktrees::strip_ansi(&partial)
                    .trim()
                    .to_string(),
            ));
        }
    }
    if !buf.is_empty() {
        let _ = tx.send(Report::Line(
            id,
            String::from_utf8_lossy(&buf)
                .trim_end_matches('\r')
                .to_string(),
        ));
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::tui::actions::Run;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in for ddev: `$1` is "tryout", `$2` names what to do.
    pub fn fake(dir: &Path) -> PathBuf {
        let p = dir.join("fake-ddev");
        std::fs::write(
            &p,
            r#"#!/bin/sh
echo "@@tryout {\"level\":\"info\",\"msg\":\"Starting $2\"}"
echo "plain output of $2"
sleep 0.3
echo "@@tryout {\"level\":\"info\",\"msg\":\"Finishing $2\"}"
if [ "$2" = fail ]; then
  echo "@@tryout {\"level\":\"error\",\"msg\":\"it broke\"}" >&2
  echo "@@tryout {\"level\":\"error\",\"msg\":\"  → try again\"}" >&2
  exit 3
fi
"#,
        )
        .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn action(what: &str) -> Action {
        Action {
            label: what.into(),
            hint: String::new(),
            args: vec![what.into()],
            run: Run::Background,
        }
    }

    fn until_done(jobs: &mut Jobs, dir: &Path, n: usize) -> Vec<Job> {
        let mut done = Vec::new();
        let t = Instant::now();
        while done.len() < n {
            assert!(t.elapsed() < Duration::from_secs(10), "jobs never finished");
            done.extend(jobs.tick(dir));
            thread::sleep(Duration::from_millis(20));
        }
        done
    }

    #[test]
    fn jobs_run_one_at_a_time_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Jobs::new(fake(dir.path()));
        let a = jobs.enqueue(&action("first"), false);
        let b = jobs.enqueue(&action("second"), false);
        jobs.tick(dir.path());
        assert!(matches!(
            jobs.get(a).unwrap().state,
            JobState::Running { .. }
        ));
        assert_eq!(
            jobs.get(b).unwrap().state,
            JobState::Queued,
            "the second waits"
        );
        let done = until_done(&mut jobs, dir.path(), 2);
        assert_eq!(done.iter().map(|j| j.id).collect::<Vec<_>>(), [a, b]);
    }

    #[test]
    fn events_become_the_step_and_the_rest_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Jobs::new(fake(dir.path()));
        let id = jobs.enqueue(&action("serve"), false);
        jobs.tick(dir.path());
        let t = Instant::now();
        loop {
            jobs.tick(dir.path());
            if let JobState::Running { step: Some(s), .. } = &jobs.get(id).unwrap().state {
                assert_eq!(s, "Starting serve");
                break;
            }
            assert!(t.elapsed() < Duration::from_secs(5), "no step arrived");
            thread::sleep(Duration::from_millis(10));
        }
        let job = &until_done(&mut jobs, dir.path(), 1)[0];
        assert_eq!(job.log, ["plain output of serve"], "markers are not log");
        assert!(matches!(
            job.state,
            JobState::Done {
                ok: true,
                code: Some(0),
                ..
            }
        ));
    }

    #[test]
    fn a_failure_keeps_its_error_and_does_not_stall_the_queue() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Jobs::new(fake(dir.path()));
        jobs.enqueue(&action("fail"), false);
        jobs.enqueue(&action("after"), false);
        let done = until_done(&mut jobs, dir.path(), 2);
        assert!(matches!(
            done[0].state,
            JobState::Done {
                ok: false,
                code: Some(3),
                ..
            }
        ));
        assert_eq!(
            done[0].last_error.as_deref(),
            Some("it broke"),
            "not the '→' hint"
        );
        assert!(matches!(done[1].state, JobState::Done { ok: true, .. }));
    }

    #[test]
    fn a_program_that_cannot_start_is_a_failed_job_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Jobs::new(dir.path().join("no-such-program"));
        jobs.enqueue(&action("x"), false);
        let done = until_done(&mut jobs, dir.path(), 1);
        assert!(matches!(
            done[0].state,
            JobState::Done {
                ok: false,
                code: None,
                ..
            }
        ));
        assert!(done[0].log[0].contains("could not start"));
    }

    #[test]
    fn only_the_last_few_finished_jobs_are_kept_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Jobs::new(fake(dir.path()));
        for i in 0..7 {
            jobs.enqueue(&action(&format!("j{i}")), false);
        }
        until_done(&mut jobs, dir.path(), 7);
        let labels: Vec<_> = jobs.list().iter().map(|j| j.label.as_str()).collect();
        assert_eq!(labels, ["j6", "j5", "j4", "j3", "j2"]);
    }

    /// A stand-in that asks for a password the way sudo does: echo off, a
    /// prompt with no newline, the answer read from the terminal.
    pub fn asking(dir: &Path) -> PathBuf {
        let p = dir.join("fake-sudo");
        std::fs::write(
            &p,
            "#!/bin/sh\necho before\nprintf 'Password:'\nread -r pw\necho\n[ \"$pw\" = secret ] && echo accepted && exit 0\necho refused; exit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn until_waiting(jobs: &mut Jobs, dir: &Path) -> (u64, String, String) {
        let t = Instant::now();
        loop {
            jobs.tick(dir);
            if let Some(w) = jobs.waiting() {
                return w;
            }
            assert!(t.elapsed() < Duration::from_secs(10), "no prompt arrived");
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_password_prompt_is_answered_and_never_logged() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Jobs::new(asking(dir.path()));
        let id = jobs.enqueue(&action("serve"), false);
        let (wid, prompt, label) = until_waiting(&mut jobs, dir.path());
        assert_eq!(
            (wid, prompt.as_str(), label.as_str()),
            (id, "Password:", "serve")
        );
        jobs.answer(id, "secret");
        assert!(jobs.waiting().is_none());
        let job = &until_done(&mut jobs, dir.path(), 1)[0];
        assert!(
            matches!(job.state, JobState::Done { ok: true, .. }),
            "{:?}",
            job.log
        );
        assert!(job.log.iter().any(|l| l.contains("accepted")));
        assert!(
            !job.log.iter().any(|l| l.contains("secret")),
            "the password was echoed: {:?}",
            job.log
        );
    }

    #[test]
    fn a_cancelled_prompt_fails_the_job() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Jobs::new(asking(dir.path()));
        let id = jobs.enqueue(&action("serve"), false);
        until_waiting(&mut jobs, dir.path());
        jobs.cancel_prompt(id);
        let job = &until_done(&mut jobs, dir.path(), 1)[0];
        assert!(matches!(job.state, JobState::Done { ok: false, .. }));
    }

    #[test]
    fn only_a_password_question_is_a_prompt() {
        assert!(is_password_prompt("Password:"));
        assert!(is_password_prompt("[sudo] password for jochen: "));
        assert!(is_password_prompt("\x1b[1mPassword for jochen:\x1b[0m"));
        assert!(!is_password_prompt("Composer: installing"));
        assert!(!is_password_prompt("the password was reset"));
    }

    #[test]
    fn a_finished_job_can_be_run_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Jobs::new(fake(dir.path()));
        let first = jobs.enqueue(&action("fail"), true);
        jobs.tick(dir.path());
        assert_eq!(jobs.retry(first), None, "not while it runs");
        until_done(&mut jobs, dir.path(), 1);
        let again = jobs.retry(first).unwrap();
        let job = jobs.get(again).unwrap();
        assert_ne!(again, first);
        assert_eq!(
            (job.label.as_str(), job.args.as_slice(), job.reveal),
            ("fail", &["fail".to_string()][..], true)
        );
        assert_eq!(job.state, JobState::Queued);
        let done = until_done(&mut jobs, dir.path(), 1);
        assert_eq!(done[0].id, again);
    }

    #[test]
    fn a_long_log_keeps_its_tail() {
        let mut job = Job {
            id: 1,
            label: "x".into(),
            args: vec![],
            state: JobState::Queued,
            log: VecDeque::new(),
            last_error: None,
            changes_worktrees: false,
            reveal: false,
            waiting: None,
        };
        for i in 0..LOG_LINES + 10 {
            take_line(&mut job, format!("line {i}"));
        }
        assert_eq!(job.log.len(), LOG_LINES);
        assert_eq!(job.log.back().unwrap(), &format!("line {}", LOG_LINES + 9));
    }
}
