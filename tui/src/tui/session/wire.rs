//! A ratatui backend with no terminal behind it: each draw becomes a `Frame` for
//! the attached client to paint. ratatui's `Terminal` still does the diffing, so
//! a frame carries only the cells that changed.

use std::io;

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

use super::proto::{Frame, WireCell};

pub struct WireBackend {
    size: Size,
    pending: Frame,
    cursor: Position,
    cursor_visible: bool,
}

impl WireBackend {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            size: Size::new(cols, rows),
            pending: Frame::default(),
            cursor: Position::ORIGIN,
            cursor_visible: false,
        }
    }

    /// The client's terminal changed size; ratatui picks it up on the next draw.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.size = Size::new(cols, rows);
    }

    /// Everything drawn since the last call, with the cursor as it now stands.
    pub fn take_frame(&mut self) -> Frame {
        let mut frame = std::mem::take(&mut self.pending);
        frame.cursor = self
            .cursor_visible
            .then_some((self.cursor.x, self.cursor.y));
        frame
    }
}

impl Backend for WireBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.pending
            .cells
            .extend(content.map(|(x, y, c)| (x, y, WireCell::from(c))));
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.cursor_visible = false;
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.cursor_visible = true;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.cursor = position.into();
        Ok(())
    }

    /// A clear throws away anything drawn before it in this frame, and tells the
    /// client to blank its screen first.
    fn clear(&mut self) -> io::Result<()> {
        self.pending.clear = true;
        self.pending.cells.clear();
        Ok(())
    }

    fn clear_region(&mut self, _: ClearType) -> io::Result<()> {
        self.clear()
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: self.size,
            pixels: Size::new(0, 0),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};
    use ratatui::widgets::{Block, Paragraph};

    fn scene(text: &str) -> impl Fn(&mut ratatui::Frame) + '_ {
        move |f| {
            let p = Paragraph::new(text)
                .style(Style::new().fg(Color::Yellow))
                .block(Block::bordered().title(" wire "));
            f.render_widget(p, f.area());
            f.set_cursor_position((3, 2));
        }
    }

    /// Paint frames onto a buffer the way a client paints them onto a terminal.
    fn paint(buf: &mut Buffer, frame: &Frame) {
        if frame.clear {
            buf.reset();
        }
        for (x, y, c) in &frame.cells {
            buf[(*x, *y)] = c.to_cell();
        }
    }

    #[test]
    fn painting_the_frames_reproduces_what_ratatui_drew() {
        let mut wire = Terminal::new(WireBackend::new(30, 6)).unwrap();
        let mut test = Terminal::new(TestBackend::new(30, 6)).unwrap();
        let mut client = Buffer::empty(Rect::new(0, 0, 30, 6));

        for text in ["first frame", "second, a diff"] {
            wire.draw(scene(text)).unwrap();
            test.draw(scene(text)).unwrap();
            let frame = wire.backend_mut().take_frame();
            paint(&mut client, &frame);
            assert_eq!(&client, test.backend().buffer(), "after {text:?}");
            assert_eq!(frame.cursor, Some((3, 2)));
        }
    }

    #[test]
    fn an_unchanged_frame_sends_no_cells_and_a_clear_sends_all() {
        let mut wire = Terminal::new(WireBackend::new(20, 4)).unwrap();
        wire.draw(scene("same")).unwrap();
        wire.backend_mut().take_frame();
        wire.draw(scene("same")).unwrap();
        assert!(wire.backend_mut().take_frame().cells.is_empty());

        // An attach clears, so the new client gets the whole screen.
        wire.clear().unwrap();
        wire.draw(scene("same")).unwrap();
        let frame = wire.backend_mut().take_frame();
        assert!(frame.clear);
        assert_eq!(frame.cells.len(), 20 * 4);
    }

    #[test]
    fn a_hidden_cursor_travels_as_none() {
        let mut wire = Terminal::new(WireBackend::new(10, 2)).unwrap();
        wire.draw(|f| f.render_widget(Paragraph::new("x"), f.area()))
            .unwrap();
        assert_eq!(wire.backend_mut().take_frame().cursor, None);
    }
}
