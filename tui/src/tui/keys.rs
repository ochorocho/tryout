//! Key events to the bytes a terminal sends for them, for writing into a pane.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The bytes for a key, or None for keys a terminal has no encoding for.
pub fn to_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let mut bytes = match key.code {
        KeyCode::Char(c) if ctrl => ctrl_byte(c).map(|b| vec![b])?,
        KeyCode::Char(c) => c.to_string().into_bytes(),
        // Raw mode: Enter is a carriage return, as a real terminal sends it.
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n @ 1..=4) => format!("\x1bO{}", (b'P' + n - 1) as char).into_bytes(),
        KeyCode::F(n @ 5..=12) => {
            const CODES: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
            format!("\x1b[{}~", CODES[(n - 5) as usize]).into_bytes()
        }
        _ => return None,
    };
    if alt {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

fn ctrl_byte(c: char) -> Option<u8> {
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => Some(c as u8 - b'a' + 1),
        ' ' | '@' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '7' | '/' => Some(0x1f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, m: KeyModifiers) -> Option<Vec<u8>> {
        to_bytes(KeyEvent::new(code, m))
    }

    #[test]
    fn plain_text_and_enter() {
        assert_eq!(
            key(KeyCode::Char('ä'), KeyModifiers::NONE),
            Some("ä".into())
        );
        assert_eq!(key(KeyCode::Enter, KeyModifiers::NONE), Some(vec![b'\r']));
    }

    #[test]
    fn control_and_alt_combinations() {
        assert_eq!(
            key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(vec![3])
        );
        assert_eq!(
            key(KeyCode::Char('D'), KeyModifiers::CONTROL),
            Some(vec![4])
        );
        assert_eq!(
            key(KeyCode::Char('b'), KeyModifiers::ALT),
            Some(vec![0x1b, b'b'])
        );
    }

    #[test]
    fn cursor_and_function_keys() {
        assert_eq!(
            key(KeyCode::Up, KeyModifiers::NONE),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            key(KeyCode::F(1), KeyModifiers::NONE),
            Some(b"\x1bOP".to_vec())
        );
        assert_eq!(
            key(KeyCode::F(5), KeyModifiers::NONE),
            Some(b"\x1b[15~".to_vec())
        );
    }
}
