//! The session server: it owns the app — every tab, agent, popup and the UI
//! state itself — and runs whether or not a terminal is attached. It renders
//! into a `WireBackend` and sends the attached client what changed.

use std::collections::HashMap;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::Terminal;
use ratatui::layout::Rect;

use super::proto::{self, ClientMsg, ServerMsg, VERSION};
use super::wire::WireBackend;
use crate::tui::actions::{Action, Run};
use crate::tui::agents;
use crate::tui::app::{App, Effect, Focus};
use crate::tui::pane::Pane;
use crate::tui::ui;
use crate::tui::worktrees::{self, Worktree};

pub type Loaded = Result<Vec<Worktree>>;
/// How the worktree list is fetched; a test passes its own.
pub type Loader = fn(&Path) -> Loaded;

/// How long to wait for a client message before looking for pane output
/// again. Short enough that a busy pane scrolls smoothly, long enough to idle.
const TICK: Duration = Duration::from_millis(16);
/// The same with nobody attached: jobs still start and finish, and tabs are
/// still reaped, but nothing is drawn, so there is no need to hurry.
const IDLE_TICK: Duration = Duration::from_millis(250);

/// How often the agents pane asks `ps` what each tab is running.
const AGENT_POLL: Duration = Duration::from_secs(1);

/// The branch list, as the loader delivers it.
type Branches = Result<Vec<String>>;
/// A page of open changes, with the search and page it answers.
type Patches = (String, u32, Result<worktrees::PatchPage>);

/// Which tabs run an agent: tab id → agent name.
type AgentKinds = HashMap<u64, &'static str>;

/// What reaches the loop from the socket side.
enum Incoming {
    /// A new connection and its first message. Read by the accept thread, which
    /// is the only reader until then: the loop reading it too raced the
    /// per-connection reader, and the loser's attach never happened.
    Connected(u64, UnixStream, ClientMsg),
    Msg(u64, ClientMsg),
    Gone(u64),
}

/// The attached terminal, if any. One at a time.
struct Client {
    id: u64,
    stream: UnixStream,
}

/// Run a session for `root` on `socket` until it is closed. Blocks.
pub fn serve(root: PathBuf, socket: &Path, loader: Loader) -> Result<()> {
    let listener = UnixListener::bind(socket)
        .with_context(|| format!("cannot listen on {}", socket.display()))?;
    // Every shell and command the session starts inherits this, so a
    // `ddev tryout ui` typed inside one knows it is inside — attaching there
    // would draw the session within itself.
    // SAFETY: set before this function starts any thread.
    unsafe { std::env::set_var(super::SESSION_ENV, socket) };
    // Which socket file is ours: a successor may bind the same path the moment
    // this one says goodbye — `stop` then `ddev tryout ui` does exactly that —
    // and removing the path blindly would take the NEW session's socket with it.
    let ours = socket_inode(socket);
    // Which build this session runs, so a client from a newer one can say so.
    let build = socket.with_extension("build");
    let stamp = super::build_stamp();
    let _ = std::fs::write(&build, &stamp);
    let (in_tx, in_rx) = mpsc::channel();
    accept(listener, in_tx);
    let result = Server::new(root, loader).run(in_rx);
    if ours.is_some() && socket_inode(socket) == ours {
        let _ = std::fs::remove_file(socket);
        if std::fs::read_to_string(&build).ok() == Some(stamp) {
            let _ = std::fs::remove_file(&build);
        }
    }
    result
}

fn socket_inode(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path).ok().map(|m| m.ino())
}

/// Every connection gets an id and a reader thread feeding the loop.
fn accept(listener: UnixListener, tx: Sender<Incoming>) {
    thread::spawn(move || {
        for (stream, id) in listener.incoming().flatten().zip(1u64..) {
            let Ok(mut reader) = stream.try_clone() else {
                continue;
            };
            // Briefly, so a connection that never speaks cannot stall accepting.
            let _ = reader.set_read_timeout(Some(Duration::from_secs(2)));
            let Ok(first) = proto::read_msg::<_, ClientMsg>(&mut reader) else {
                continue;
            };
            let _ = reader.set_read_timeout(None);
            if tx.send(Incoming::Connected(id, stream, first)).is_err() {
                return;
            }
            let tx = tx.clone();
            thread::spawn(move || {
                while let Ok(msg) = proto::read_msg::<_, ClientMsg>(&mut reader) {
                    if tx.send(Incoming::Msg(id, msg)).is_err() {
                        return;
                    }
                }
                let _ = tx.send(Incoming::Gone(id));
            });
        }
    });
}

struct Server {
    app: App,
    terminal: Terminal<WireBackend>,
    client: Option<Client>,
    loader: Loader,
    loads: (Sender<Loaded>, Receiver<Loaded>),
    notices: (Sender<String>, Receiver<String>),
    kinds: (Sender<AgentKinds>, Receiver<AgentKinds>),
    branches: (Sender<Branches>, Receiver<Branches>),
    patches: (Sender<Patches>, Receiver<Patches>),
    last_poll: Instant,
    polling: bool,
    /// A list load is out, and whether another was asked for meanwhile: loads
    /// never overlap, so an older answer cannot land after a newer one.
    loading: bool,
    load_again: bool,
    /// The tab a mouse press started on, until the button comes up.
    dragging: Option<u64>,
    redraw: bool,
    /// When the Activity spinner last moved.
    last_spin: Instant,
}

/// What the loop does after a message.
enum Next {
    Go,
    Close,
}

impl Server {
    fn new(root: PathBuf, loader: Loader) -> Self {
        Self {
            app: App::new(root),
            terminal: Terminal::new(WireBackend::new(80, 24)).expect("an in-memory backend"),
            client: None,
            loader,
            loads: mpsc::channel(),
            notices: mpsc::channel(),
            kinds: mpsc::channel(),
            branches: mpsc::channel(),
            patches: mpsc::channel(),
            last_poll: Instant::now() - AGENT_POLL,
            polling: false,
            loading: false,
            load_again: false,
            dragging: None,
            redraw: true,
            last_spin: Instant::now(),
        }
    }

    fn run(mut self, incoming: Receiver<Incoming>) -> Result<()> {
        self.load();
        loop {
            self.tick();
            if self.redraw && self.client.is_some() {
                self.draw();
            }
            let wait = if self.client.is_some() {
                TICK
            } else {
                IDLE_TICK
            };
            let next = match incoming.recv_timeout(wait) {
                Ok(Incoming::Connected(id, stream, first)) => self.attach(id, stream, first),
                Ok(Incoming::Msg(id, msg)) if self.client.as_ref().is_some_and(|c| c.id == id) => {
                    self.message(msg)
                }
                Ok(Incoming::Msg(..)) => Next::Go,
                Ok(Incoming::Gone(id)) => {
                    // A terminal closed or crashed: the session carries on.
                    if self.client.as_ref().is_some_and(|c| c.id == id) {
                        self.client = None;
                    }
                    Next::Go
                }
                Err(RecvTimeoutError::Timeout) => Next::Go,
                Err(RecvTimeoutError::Disconnected) => Next::Close,
            };
            if let Next::Close = next {
                return Ok(());
            }
        }
    }

    /// A connection's first message says what it wants: to attach, or to stop
    /// the session.
    fn attach(&mut self, id: u64, mut stream: UnixStream, first: ClientMsg) -> Next {
        match first {
            ClientMsg::Hello {
                version,
                cols,
                rows,
            } if version == VERSION => {
                if let Some(mut old) = self.client.take() {
                    bye(&mut old.stream, "attached from another terminal");
                }
                self.terminal.backend_mut().resize(cols, rows);
                let _ = self.terminal.resize(Rect::new(0, 0, cols, rows));
                // A newcomer has nothing on screen: send it everything.
                let _ = self.terminal.clear();
                self.client = Some(Client { id, stream });
                self.redraw = true;
                Next::Go
            }
            ClientMsg::Hello { version, .. } => {
                bye(
                    &mut stream,
                    &format!(
                        "this session runs protocol {VERSION}, the terminal speaks {version}\n  → ddev tryout ui stop, then ddev tryout ui"
                    ),
                );
                Next::Go
            }
            ClientMsg::Stop => {
                bye(&mut stream, "session closed");
                self.goodbye("session closed");
                Next::Close
            }
            ClientMsg::Event(_) => Next::Go,
        }
    }

    fn message(&mut self, msg: ClientMsg) -> Next {
        match msg {
            ClientMsg::Event(ev) => self.event(ev),
            ClientMsg::Stop => {
                self.goodbye("session closed");
                Next::Close
            }
            ClientMsg::Hello { .. } => Next::Go,
        }
    }

    /// Tell the attached client, if any, that it is done.
    fn goodbye(&mut self, reason: &str) {
        if let Some(mut c) = self.client.take() {
            bye(&mut c.stream, reason);
        }
    }

    fn draw(&mut self) {
        let app = &self.app;
        if self.terminal.draw(|f| ui::draw(f, app)).is_err() {
            return;
        }
        self.redraw = false;
        let frame = self.terminal.backend_mut().take_frame();
        if let Some(c) = self.client.as_mut()
            && proto::write_msg(&mut c.stream, &ServerMsg::Frame(frame)).is_err()
        {
            self.client = None;
        }
    }

    fn screen(&self) -> Rect {
        let size = self.terminal.size().unwrap_or_default();
        Rect::new(0, 0, size.width, size.height)
    }

    /// Everything that happens with or without a client: sizes, finished
    /// loads, agents, reaped tabs, popups that closed themselves.
    fn tick(&mut self) {
        let screen = self.screen();
        self.app.sidebar_max = ui::sidebar_max(screen.width);
        self.app.screen = screen;
        let inner = ui::areas(screen, self.app.sidebar_width).pane_inner;
        for ws in self.app.workspaces.values_mut() {
            for tab in &mut ws.tabs {
                tab.pane.resize(inner.height, inner.width);
            }
        }
        if let Ok(result) = self.loads.1.try_recv() {
            self.app.set_worktrees(result);
            self.redraw = true;
            self.loading = false;
            if std::mem::take(&mut self.load_again) {
                self.load();
            }
        }
        while let Ok((search, page, result)) = self.patches.1.try_recv() {
            self.app.set_patches(&search, page, result);
            self.redraw = true;
        }
        if let Ok(result) = self.branches.1.try_recv() {
            self.app.set_branches(result);
            self.redraw = true;
        }
        if let Ok(notice) = self.notices.1.try_recv() {
            self.app.notice = Some(notice);
            self.redraw = true;
        }
        // Once a second, and never two at a time: which tabs run an agent. The
        // answer also redraws, which is what moves a status on as time passes.
        if let Ok(kinds) = self.kinds.1.try_recv() {
            self.app.set_agent_kinds(kinds);
            self.polling = false;
            self.redraw = true;
        }
        if !self.polling && self.last_poll.elapsed() >= AGENT_POLL {
            poll_agents(self.app.foreground_pids(), &self.kinds.0);
            self.last_poll = Instant::now();
            self.polling = true;
        }
        // Every pane's flag is cleared, not just the first dirty one found.
        self.redraw |= self
            .app
            .workspaces
            .values()
            .flat_map(|ws| ws.tabs.iter().map(|t| &t.pane))
            .fold(false, |any, p| p.take_dirty() | any);
        // A running job's spinner and step move without any output of ours.
        if self.app.jobs.running() && self.last_spin.elapsed() >= Duration::from_millis(100) {
            self.last_spin = Instant::now();
            self.redraw = true;
        }
        // A success leaving the Activity list is a change nothing else draws.
        let jobs_before = self.app.jobs.list().len();
        let effect = self.app.tick();
        if self.app.jobs.list().len() != jobs_before {
            self.redraw = true;
        }
        match effect {
            Effect::Reload => {
                self.load();
                self.redraw = true;
            }
            Effect::None => {}
            other => {
                self.apply(other);
            }
        }
    }

    fn load(&mut self) {
        if self.loading {
            self.load_again = true;
            return;
        }
        self.loading = true;
        let (root, tx, loader) = (self.app.root.clone(), self.loads.0.clone(), self.loader);
        thread::spawn(move || {
            let _ = tx.send(loader(&root));
        });
    }

    /// Carry out an effect, from a key or a click alike.
    fn apply(&mut self, effect: Effect) -> Next {
        let screen = self.screen();
        match effect {
            Effect::None => {}
            Effect::Detach => {
                self.goodbye(
                    "detached — the session keeps running\n  → ddev tryout ui to attach again",
                );
            }
            Effect::CloseSession => {
                self.goodbye("session closed");
                return Next::Close;
            }
            Effect::Reload => self.load(),
            Effect::NewTab(name) => {
                let inner = ui::areas(screen, self.app.sidebar_width).pane_inner;
                open_tab(&mut self.app, &name, inner)
            }
            Effect::LoadPatches(site, search, page) => {
                let (root, tx) = (self.app.root.clone(), self.patches.0.clone());
                self.redraw = true;
                thread::spawn(move || {
                    let result = worktrees::load_patches(&root, &site, &search, page);
                    let _ = tx.send((search, page, result));
                });
            }
            Effect::LoadPullRequests(search, page) => {
                let (root, tx) = (self.app.root.clone(), self.patches.0.clone());
                self.redraw = true;
                thread::spawn(move || {
                    let result = worktrees::load_pull_requests(&root, &search, page);
                    let _ = tx.send((search, page, result));
                });
            }
            Effect::LoadBranches => {
                let (root, tx) = (self.app.root.clone(), self.branches.0.clone());
                thread::spawn(move || {
                    let _ = tx.send(worktrees::load_branches(&root));
                });
            }
            Effect::Run(action) => match action.run {
                Run::Job { reveal } => {
                    self.app.jobs.enqueue(&action, reveal);
                }
                Run::Form(kind) => {
                    let effect = self.app.open_form(kind);
                    return self.apply(effect);
                }
                Run::Background => run_in_background(&self.app.root, action, &self.notices.0),
            },
        }
        Next::Go
    }

    fn event(&mut self, ev: Event) -> Next {
        let screen = self.screen();
        let app = &mut self.app;
        match ev {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                self.redraw = true;
                let effect = self.app.handle_key(key);
                return self.apply(effect);
            }
            Event::Paste(text) => {
                let target = match app.focus {
                    Focus::Pane => {
                        let name = app.selected().map(|w| w.name.clone());
                        name.and_then(|n| app.workspaces.get_mut(&n))
                            .and_then(|ws| ws.tabs.get_mut(ws.active))
                            .map(|t| &mut t.pane)
                    }
                    _ => None,
                };
                if let Some(pane) = target {
                    pane.write(text.as_bytes());
                }
            }
            // On press: selecting is instant and harmless, so there is no drag to
            // guard against the way a click that RUNS something would need.
            Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                self.redraw = true;
                let (col, row) = (m.column, m.row);
                if app.popup_open() {
                    return Next::Go;
                }
                // An open form takes the click: a change ticks, the page bar pages;
                // nothing behind it is clicked through.
                if app.form.is_some() {
                    if let Some(hit) = ui::form_hit(screen, app, col, row) {
                        app.form_click(hit);
                    }
                    return Next::Go;
                }
                // The divider first: grabbing it must not select what is beside it.
                if app.menu.is_none() && app.form.is_none() && ui::divider_at(screen, app, col, row)
                {
                    app.press_divider(col, Instant::now());
                    return Next::Go;
                }
                // An open menu takes the click whatever it lands on: an item runs,
                // anything else just closes it — and selects nothing behind it.
                let effect = if app.menu.is_some() {
                    Some(app.click_menu(ui::menu_at(screen, app, col, row)))
                } else if app.log_view.is_some() && ui::log_retry_at(screen, app, col, row) {
                    app.retry_log();
                    return Next::Go;
                } else if ui::new_button_at(screen, app, col, row) {
                    Some(app.new_worktree())
                } else if ui::url_at(screen, app, col, row) {
                    Some(app.click_url())
                } else {
                    None
                };
                if let Some(effect) = effect {
                    return self.apply(effect);
                } else if let Some(id) = ui::activity_retry_at(screen, app, col, row) {
                    app.retry(id);
                } else if let Some(id) = ui::activity_at(screen, app, col, row) {
                    app.open_log(id);
                } else if let Some(i) = ui::agent_at(screen, app, col, row) {
                    app.click_agent(i);
                } else if let Some(i) = ui::worktree_at(screen, app, col, row) {
                    app.click_worktree(i);
                } else if let Some(hit) = ui::tab_at(screen, app, col, row) {
                    match hit {
                        ui::TabHit::Tab(id) => {
                            app.click_tab_at(id, Instant::now());
                            self.dragging = Some(id);
                        }
                        ui::TabHit::New => {
                            let effect = app.click_new_tab();
                            return self.apply(effect);
                        }
                    }
                } else if ui::in_pane_body(screen, app, col, row) {
                    app.click_pane();
                }
            }
            // The wheel over an open form moves its selection.
            Event::Mouse(m)
                if matches!(
                    m.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) && app.form.is_some() =>
            {
                if let Some(form) = app.form.as_mut() {
                    form.move_selection(if m.kind == MouseEventKind::ScrollUp {
                        -1
                    } else {
                        1
                    });
                }
                self.redraw = true;
            }
            // The wheel over an open log scrolls it, three rows a notch.
            Event::Mouse(m)
                if matches!(
                    m.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) && app.log_view.is_some()
                    && ui::areas(screen, app.sidebar_width)
                        .pane
                        .contains(ratatui::layout::Position::new(m.column, m.row)) =>
            {
                app.scroll_log(if m.kind == MouseEventKind::ScrollUp {
                    3
                } else {
                    -3
                });
                self.redraw = true;
            }
            // A right-click on a worktree opens its `worktree` commands there.
            Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Right) => {
                if let Some(i) = ui::worktree_at(screen, app, m.column, m.row) {
                    app.context_menu(i, (m.column, m.row));
                    self.redraw = true;
                }
            }
            // The divider follows the pointer while held.
            Event::Mouse(m) if m.kind == MouseEventKind::Drag(MouseButton::Left) => {
                if app.resizing() {
                    app.drag_divider(m.column);
                    self.redraw = true;
                }
            }
            Event::Mouse(m)
                if m.kind == MouseEventKind::Up(MouseButton::Left) && app.resizing() =>
            {
                app.release_divider();
                self.redraw = true;
            }
            // A tab pressed and released over another tab was dragged there.
            Event::Mouse(m) if m.kind == MouseEventKind::Up(MouseButton::Left) => {
                if let (Some(from), Some(ui::TabHit::Tab(onto))) = (
                    self.dragging.take(),
                    ui::tab_at(screen, app, m.column, m.row),
                ) {
                    app.drop_tab(from, onto);
                    self.redraw = true;
                }
            }
            Event::Resize(cols, rows) => {
                self.terminal.backend_mut().resize(cols, rows);
                let _ = self.terminal.resize(Rect::new(0, 0, cols, rows));
                let _ = self.terminal.clear();
                self.redraw = true;
            }
            _ => {}
        }
        Next::Go
    }
}

fn bye(stream: &mut UnixStream, reason: &str) {
    let _ = proto::write_msg(
        stream,
        &ServerMsg::Bye {
            reason: reason.to_string(),
        },
    );
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// Ask `ps` about every tab's foreground process, off the loop's thread.
fn poll_agents(pids: Vec<(u64, u32)>, tx: &Sender<AgentKinds>) {
    let tx = tx.clone();
    thread::spawn(move || {
        let args = agents::lookup(&pids.iter().map(|(_, pid)| *pid).collect::<Vec<_>>());
        let kinds = pids
            .into_iter()
            .filter_map(|(tab, pid)| {
                args.get(&pid)
                    .and_then(|a| agents::classify(a))
                    .map(|k| (tab, k))
            })
            .collect();
        let _ = tx.send(kinds);
    });
}

fn open_tab(app: &mut App, name: &str, inner: Rect) {
    let dir = app.checkout_dir(name);
    match Pane::shell(&dir, inner.height.max(1), inner.width.max(1)) {
        Ok(pane) => app.add_tab(name, pane),
        Err(e) => app.notice = Some(format!("could not start a shell: {e:#}")),
    }
}

/// A command with no terminal: fast and silent on success, so only its failure
/// is worth a word — and that goes to the footer, since nothing else is open.
fn run_in_background(root: &Path, action: Action, notices: &Sender<String>) {
    let (root, notices) = (root.to_path_buf(), notices.clone());
    thread::spawn(move || {
        let out = std::process::Command::new("ddev")
            .arg("tryout")
            .args(&action.args)
            .current_dir(&root)
            .output();
        let notice = match out {
            Ok(o) if o.status.success() => format!("✓ {}", action.label),
            Ok(o) => {
                let err = worktrees::strip_ansi(&String::from_utf8_lossy(&o.stderr));
                let last = err
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("failed");
                format!("{}: {}", action.label, last.trim())
            }
            Err(e) => format!("{}: {e}", action.label),
        };
        let _ = notices.send(notice);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::buffer::Buffer;
    use std::time::Duration;

    fn load(_: &Path) -> Loaded {
        Ok(crate::tui::app::tests::fixture())
    }

    /// A server on a temp socket, in a thread; joined to see it end.
    fn start() -> (tempfile::TempDir, PathBuf, thread::JoinHandle<Result<()>>) {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("s.sock");
        let (root, at) = (dir.path().to_path_buf(), sock.clone());
        let h = thread::spawn(move || serve(root, &at, load));
        for _ in 0..100 {
            if sock.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        (dir, sock, h)
    }

    /// A client that paints frames into a buffer instead of a terminal.
    struct Screen {
        stream: UnixStream,
        buf: Buffer,
        bye: Option<String>,
    }

    impl Screen {
        fn attach(sock: &Path) -> Self {
            Self::attach_as(sock, VERSION)
        }

        fn attach_as(sock: &Path, version: u32) -> Self {
            let mut stream = UnixStream::connect(sock).unwrap();
            proto::write_msg(
                &mut stream,
                &ClientMsg::Hello {
                    version,
                    cols: 100,
                    rows: 30,
                },
            )
            .unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            Self {
                stream,
                buf: Buffer::empty(Rect::new(0, 0, 100, 30)),
                bye: None,
            }
        }

        fn text(&self) -> String {
            (0..30)
                .map(|y| {
                    (0..100)
                        .map(|x| self.buf[(x, y)].symbol())
                        .collect::<String>()
                        + "\n"
                })
                .collect()
        }

        /// Read frames until `what` is on screen (or a goodbye), for up to 10s.
        fn until(&mut self, what: impl Fn(&Self) -> bool) -> bool {
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline && self.bye.is_none() {
                match proto::read_msg::<_, ServerMsg>(&mut self.stream) {
                    Ok(ServerMsg::Frame(f)) => {
                        if f.clear {
                            self.buf.reset();
                        }
                        for (x, y, c) in &f.cells {
                            self.buf[(*x, *y)] = c.to_cell();
                        }
                    }
                    Ok(ServerMsg::Bye { reason }) => self.bye = Some(reason),
                    Err(_) => {}
                }
                if what(self) {
                    return true;
                }
            }
            what(self)
        }

        fn shows(&mut self, text: &str) -> bool {
            self.until(|s| s.text().contains(text))
        }

        fn key(&mut self, code: KeyCode, m: KeyModifiers) {
            let ev = Event::Key(KeyEvent::new(code, m));
            proto::write_msg(&mut self.stream, &ClientMsg::Event(ev)).unwrap();
        }

        fn typing(&mut self, text: &str) {
            for c in text.chars() {
                self.key(KeyCode::Char(c), KeyModifiers::NONE);
            }
        }
    }

    #[test]
    fn a_shell_keeps_running_while_nobody_is_attached() {
        let (_dir, sock, server) = start();
        let mut a = Screen::attach(&sock);
        assert!(a.shows("Worktrees"), "no first frame:\n{}", a.text());
        a.key(KeyCode::Enter, KeyModifiers::NONE); // a shell in main
        thread::sleep(Duration::from_millis(500));
        // The answer is not in the command line, so seeing it means it ran.
        a.typing("echo hi-$((20+22))");
        a.key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(a.shows("hi-42"), "the shell never answered:\n{}", a.text());

        drop(a); // the terminal closes; the session must not notice more than that
        thread::sleep(Duration::from_millis(300));

        let mut b = Screen::attach(&sock);
        assert!(
            b.shows("hi-42"),
            "the shell or its screen did not survive:\n{}",
            b.text()
        );

        // q from the list detaches: goodbye to the terminal, the server stays.
        b.key(KeyCode::Char('g'), KeyModifiers::CONTROL);
        b.key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(b.until(|s| s.bye.is_some()));
        assert!(b.bye.as_deref().unwrap().contains("detached"));
        assert!(!server.is_finished());

        let mut s = UnixStream::connect(&sock).unwrap();
        proto::write_msg(&mut s, &ClientMsg::Stop).unwrap();
        server.join().unwrap().unwrap();
        assert!(!sock.exists(), "a closed session leaves no socket behind");
    }

    fn slow_load(_: &Path) -> Loaded {
        thread::sleep(Duration::from_millis(1500));
        Ok(crate::tui::app::tests::fixture())
    }

    #[test]
    fn a_list_that_arrives_after_the_attach_is_drawn_without_a_key() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("s.sock");
        let (root, at) = (dir.path().to_path_buf(), sock.clone());
        thread::spawn(move || serve(root, &at, slow_load));
        for _ in 0..100 {
            if sock.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let mut a = Screen::attach(&sock);
        assert!(a.shows("Loading worktrees"), "{}", a.text());
        assert!(
            a.shows("bugfix"),
            "the list arrived but was never drawn:\n{}",
            a.text()
        );
    }

    #[test]
    fn a_closing_server_leaves_a_successors_socket_alone() {
        let (_dir, sock, server) = start();
        // Connected before the swap, so the old server can still be told to stop.
        let mut old = UnixStream::connect(&sock).unwrap();
        // A successor binds the same path, as `stop` then `ddev tryout ui` does.
        std::fs::remove_file(&sock).unwrap();
        let _successor = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        let theirs = socket_inode(&sock);
        proto::write_msg(&mut old, &ClientMsg::Stop).unwrap();
        server.join().unwrap().unwrap();
        assert!(
            sock.exists(),
            "the old server removed its successor's socket"
        );
        assert_eq!(socket_inode(&sock), theirs);
    }

    #[test]
    fn a_second_terminal_takes_over_and_the_first_is_told() {
        let (_dir, sock, _server) = start();
        let mut a = Screen::attach(&sock);
        assert!(a.shows("Worktrees"));
        let mut b = Screen::attach(&sock);
        assert!(b.shows("Worktrees"), "the newcomer gets the whole screen");
        assert!(a.until(|s| s.bye.is_some()));
        assert!(a.bye.as_deref().unwrap().contains("another terminal"));
    }

    #[test]
    fn a_terminal_of_another_version_is_refused_with_the_way_out() {
        let (_dir, sock, _server) = start();
        let mut a = Screen::attach_as(&sock, VERSION + 1);
        assert!(a.until(|s| s.bye.is_some()));
        assert!(a.bye.as_deref().unwrap().contains("ddev tryout ui stop"));
    }

    #[test]
    fn capital_q_then_y_closes_the_session() {
        let (_dir, sock, server) = start();
        let mut a = Screen::attach(&sock);
        assert!(a.shows("Worktrees"));
        a.key(KeyCode::Char('Q'), KeyModifiers::SHIFT);
        assert!(a.shows("Close the session?"), "{}", a.text());
        a.key(KeyCode::Char('y'), KeyModifiers::NONE);
        assert!(a.until(|s| s.bye.is_some()));
        assert_eq!(a.bye.as_deref(), Some("session closed"));
        server.join().unwrap().unwrap();
    }
}
