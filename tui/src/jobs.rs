//! Commands as background jobs: `ddev tryout …` run fully argued, one at a time,
//! their progress read from the add-on's `@@tryout` event lines (TRYOUT_EVENTS=1)
//! and the rest kept as a log. The UI asks its questions before a job exists, so
//! a job's stdin is closed: a prompt nobody expected fails fast with its usage
//! line instead of hanging the queue.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::actions::Action;

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
}

impl Job {
    pub fn command_line(&self) -> String {
        format!("ddev tryout {}", self.args.join(" "))
    }
}

/// What the runner threads report.
enum Report {
    Line(u64, String),
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
        });
        self.sort();
        id
    }

    /// Take in what the runners reported, start the next job when none runs,
    /// and hand back the ones that just finished.
    pub fn tick(&mut self, cwd: &Path) -> Vec<Job> {
        let mut finished = Vec::new();
        while let Ok(report) = self.rx.try_recv() {
            match report {
                Report::Line(id, line) => {
                    if let Some(job) = self.list.iter_mut().find(|j| j.id == id) {
                        take_line(job, line);
                    }
                }
                Report::Exit(id, code) => {
                    if let Some(job) = self.list.iter_mut().find(|j| j.id == id) {
                        let took = match job.state {
                            JobState::Running { since, .. } => since.elapsed(),
                            _ => Duration::ZERO,
                        };
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
            spawn(&self.program, cwd, job.id, &job.args, &self.tx);
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

/// Run `program tryout <args>` with stdin closed, both output streams read
/// line by line, and the exit code reported last.
fn spawn(program: &Path, cwd: &Path, id: u64, args: &[String], tx: &Sender<Report>) {
    let child = Command::new(program)
        .arg("tryout")
        .args(args)
        .current_dir(cwd)
        .env("TRYOUT_EVENTS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(Report::Line(
                id,
                format!("could not start {}: {e}", program.display()),
            ));
            let _ = tx.send(Report::Exit(id, None));
            return;
        }
    };
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|stream| {
        let tx = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let _ = tx.send(Report::Line(id, line));
            }
        })
    })
    .collect();
    let tx = tx.clone();
    thread::spawn(move || {
        // Every line is in before the exit is reported, so a finished job's
        // log is whole.
        for r in readers {
            let _ = r.join();
        }
        let code = child.wait().ok().and_then(|s| s.code());
        let _ = tx.send(Report::Exit(id, code));
    });
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::actions::Run;
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
        };
        for i in 0..LOG_LINES + 10 {
            take_line(&mut job, format!("line {i}"));
        }
        assert_eq!(job.log.len(), LOG_LINES);
        assert_eq!(job.log.back().unwrap(), &format!("line {}", LOG_LINES + 9));
    }
}
