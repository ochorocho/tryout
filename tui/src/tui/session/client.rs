//! The client: a terminal attached to a session. It sends what the user does
//! and paints what the server drew — it holds no state of its own, which is
//! what lets it come and go while everything keeps running.

use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::buffer::Cell;

use super::proto::{self, ClientMsg, Frame, ServerMsg, VERSION};

/// Attach this terminal until the server lets it go. Returns why it did.
pub fn attach(stream: UnixStream) -> Result<String> {
    let mut writer = stream.try_clone()?;
    let (cols, rows) = crossterm::terminal::size()?;
    proto::write_msg(
        &mut writer,
        &ClientMsg::Hello {
            version: VERSION,
            cols,
            rows,
        },
    )?;

    // Frames arrive on their own thread, so painting never waits on the keyboard.
    let (tx, rx) = mpsc::channel();
    let mut reader = stream;
    thread::spawn(move || {
        loop {
            let msg = proto::read_msg::<_, ServerMsg>(&mut reader);
            let end = !matches!(msg, Ok(ServerMsg::Frame(_)));
            if tx.send(msg).is_err() || end {
                return;
            }
        }
    });

    // init() installs a panic hook that restores the terminal; mouse reporting is
    // ours to turn on and, even on a panic, off again.
    let _ = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        hook(info);
    }));
    let result = run(&mut writer, &rx);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn run(writer: &mut UnixStream, frames: &mpsc::Receiver<io::Result<ServerMsg>>) -> Result<String> {
    let mut backend = CrosstermBackend::new(io::stdout());
    loop {
        // Everything that arrived, painted before looking at the keyboard again.
        loop {
            match frames.try_recv() {
                Ok(Ok(ServerMsg::Frame(frame))) => paint(&mut backend, &frame)?,
                Ok(Ok(ServerMsg::Bye { reason })) => return Ok(reason),
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    return Ok("the session server went away".into());
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if event::poll(Duration::from_millis(10))? {
            let ev = event::read()?;
            if proto::write_msg(writer, &ClientMsg::Event(ev)).is_err() {
                return Ok("the session server went away".into());
            }
        }
    }
}

fn paint<B: Backend>(backend: &mut B, frame: &Frame) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    if frame.clear {
        backend.clear()?;
    }
    let cells: Vec<(u16, u16, Cell)> = frame
        .cells
        .iter()
        .map(|(x, y, c)| (*x, *y, c.to_cell()))
        .collect();
    backend.draw(cells.iter().map(|(x, y, c)| (*x, *y, c)))?;
    match frame.cursor {
        Some((x, y)) => {
            backend.set_cursor_position((x, y))?;
            backend.show_cursor()?;
        }
        None => backend.hide_cursor()?,
    }
    Backend::flush(backend)?;
    io::stdout().flush()?;
    Ok(())
}
