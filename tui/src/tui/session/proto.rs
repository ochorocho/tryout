//! The wire between a client (a terminal) and the session server (everything
//! else). Length-prefixed postcard frames over a Unix socket.
//!
//! The server renders and the client paints — tmux's model — so a message is
//! either what the user did (a crossterm event) or what the screen now shows (the
//! cells that changed). The UI itself never crosses the wire.

use std::io::{self, Read, Write};

use ratatui::buffer::Cell;
use ratatui::style::{Color, Modifier};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Bumped whenever a message changes shape. A server outlives the binary that
/// started it — an add-on update does not stop it — so the client says which
/// version it speaks and a server of another one refuses with the way out.
pub const VERSION: u32 = 1;

/// Anything bigger is not a frame of ours; refuse it rather than allocate it.
const MAX_FRAME: u32 = 64 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// The first message: who is attaching, and how big their terminal is.
    Hello { version: u32, cols: u16, rows: u16 },
    /// A key, a click, a paste or a resize, exactly as the terminal reported it.
    Event(crossterm::event::Event),
    /// End the session — `tryout ui stop`, which attaches no UI to ask it.
    Stop,
}

/// What changed on screen since the last frame.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Blank the screen before painting: an attach, or a resize.
    pub clear: bool,
    pub cells: Vec<(u16, u16, WireCell)>,
    /// Where the cursor goes; None hides it.
    pub cursor: Option<(u16, u16)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    Frame(Frame),
    /// The server is done with this client: it detached, another took over,
    /// or the session closed. The reason is shown after the terminal is restored.
    Bye {
        reason: String,
    },
}

/// A screen cell as it crosses the wire. Our own type rather than ratatui's
/// serde form: that one reads colours through an untagged enum, which a compact
/// format like postcard cannot decode — and it has changed shape before.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireCell {
    pub symbol: String,
    pub fg: u32,
    pub bg: u32,
    pub modifier: u16,
}

impl From<&Cell> for WireCell {
    fn from(c: &Cell) -> Self {
        Self {
            symbol: c.symbol().to_string(),
            fg: color_to_wire(c.fg),
            bg: color_to_wire(c.bg),
            modifier: c.modifier.bits(),
        }
    }
}

impl WireCell {
    pub fn to_cell(&self) -> Cell {
        let mut c = Cell::default();
        c.set_symbol(&self.symbol);
        c.fg = color_from_wire(self.fg);
        c.bg = color_from_wire(self.bg);
        c.modifier = Modifier::from_bits_truncate(self.modifier);
        c
    }
}

// A colour in 32 bits: the top byte says which kind, the rest carries RGB or
// the palette index.
const RGB: u32 = 0x0100_0000;
const INDEXED: u32 = 0x0200_0000;
const NAMED: [Color; 17] = [
    Color::Reset,
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

fn color_to_wire(c: Color) -> u32 {
    match c {
        Color::Rgb(r, g, b) => RGB | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
        Color::Indexed(i) => INDEXED | u32::from(i),
        named => NAMED.iter().position(|n| *n == named).unwrap_or(0) as u32,
    }
}

fn color_from_wire(v: u32) -> Color {
    match v & 0xff00_0000 {
        RGB => Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8),
        INDEXED => Color::Indexed(v as u8),
        _ => NAMED.get(v as usize).copied().unwrap_or(Color::Reset),
    }
}

pub fn write_msg<W: Write, T: Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let body = postcard::to_stdvec(msg).map_err(io::Error::other)?;
    let len = u32::try_from(body.len()).map_err(io::Error::other)?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(&body)?;
    w.flush()
}

pub fn read_msg<R: Read, T: DeserializeOwned>(r: &mut R) -> io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len);
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body)?;
    postcard::from_bytes(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::{Color, Style};
    use std::os::unix::net::UnixStream;

    #[test]
    fn messages_survive_the_socket_both_ways() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        let hello = ClientMsg::Hello {
            version: VERSION,
            cols: 120,
            rows: 40,
        };
        let key = ClientMsg::Event(Event::Key(KeyEvent::new(
            KeyCode::Char('ä'),
            KeyModifiers::CONTROL,
        )));
        write_msg(&mut a, &hello).unwrap();
        write_msg(&mut a, &key).unwrap();
        assert_eq!(read_msg::<_, ClientMsg>(&mut b).unwrap(), hello);
        assert_eq!(read_msg::<_, ClientMsg>(&mut b).unwrap(), key);

        let mut cell = Cell::new("é");
        cell.set_style(Style::new().fg(Color::Cyan).bg(Color::Rgb(1, 2, 3)).bold());
        let frame = ServerMsg::Frame(Frame {
            clear: true,
            cells: vec![(3, 4, WireCell::from(&cell))],
            cursor: Some((5, 6)),
        });
        write_msg(&mut b, &frame).unwrap();
        let back = read_msg::<_, ServerMsg>(&mut a).unwrap();
        assert_eq!(back, frame);
        let ServerMsg::Frame(f) = back else {
            unreachable!()
        };
        assert_eq!(f.cells[0].2.to_cell(), cell, "the cell itself survives");
    }

    #[test]
    fn every_colour_survives_the_wire() {
        let mut all: Vec<Color> = NAMED.to_vec();
        all.extend([
            Color::Rgb(0, 0, 0),
            Color::Rgb(255, 128, 1),
            Color::Indexed(0),
            Color::Indexed(255),
        ]);
        for c in all {
            assert_eq!(color_from_wire(color_to_wire(c)), c, "{c:?}");
        }
    }

    #[test]
    fn an_oversized_frame_is_refused_not_allocated() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        a.write_all(&u32::MAX.to_be_bytes()).unwrap();
        let err = read_msg::<_, ServerMsg>(&mut b).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
