//! The output helpers every verb reports through: `info`, `success` and `warn`
//! on stdout, `error` on stderr — and, with `TRYOUT_EVENTS=1`, a
//! machine-readable twin of each on the same stream,
//! `@@tryout {"level":"info","msg":"…"}`. The TUI shows a command's progress
//! from those twins; the format is add-only, because a released TUI parses it.

use std::io::Write;

pub const RED: &str = "\x1b[0;31m";
pub const GREEN: &str = "\x1b[0;32m";
pub const YELLOW: &str = "\x1b[1;33m";
pub const CYAN: &str = "\x1b[0;36m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
/// Ordinary text, stated rather than inherited: an embedded terminal need not
/// share the user's default foreground, and 37 is legible on every theme.
pub const TEXT: &str = "\x1b[37m";
pub const NC: &str = "\x1b[0m";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warn,
    Error,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Success => "success",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }

    fn prefix(self) -> String {
        match self {
            Level::Info => format!("{CYAN}==>{NC}"),
            Level::Success => format!("{GREEN}==>{NC}"),
            Level::Warn => format!("{YELLOW}==>{NC}"),
            Level::Error => format!("{RED}✗{NC}"),
        }
    }
}

pub fn info(msg: impl AsRef<str>) {
    emit(Level::Info, msg.as_ref(), false);
}
pub fn success(msg: impl AsRef<str>) {
    emit(Level::Success, msg.as_ref(), false);
}
pub fn warn(msg: impl AsRef<str>) {
    emit(Level::Warn, msg.as_ref(), false);
}
pub fn error(msg: impl AsRef<str>) {
    emit(Level::Error, msg.as_ref(), true);
}

/// A notice on stderr whatever its level: for output a caller captures, where a
/// line on stdout would be read back as part of the answer.
pub fn notice(level: Level, msg: impl AsRef<str>) {
    emit(level, msg.as_ref(), true);
}

fn emit(level: Level, msg: &str, stderr: bool) {
    let text = render(level, msg, events_enabled());
    if stderr {
        let _ = std::io::stderr().write_all(text.as_bytes());
    } else {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    }
}

/// Pad to `width` BYTES — printf's `%-Ns`, for output that has always been
/// laid out that way.
pub fn pad_display_bytes(s: &str, width: usize) -> String {
    if s.len() >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - s.len()))
    }
}

/// Text on stdout, flushed.
pub fn print(s: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(s.as_bytes());
    let _ = out.flush();
}

/// A plain line on stdout.
pub fn print_line(s: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{s}");
    let _ = out.flush();
}

pub fn events_enabled() -> bool {
    std::env::var("TRYOUT_EVENTS").as_deref() == Ok("1")
}

/// The line a helper prints, plus its event twin when events are on.
pub fn render(level: Level, msg: &str, events: bool) -> String {
    let mut s = format!("{} {msg}\n", level.prefix());
    if events {
        s.push_str(&event(level, msg));
    }
    s
}

/// `@@tryout {"level":…,"msg":…}` with colour codes stripped from the message.
pub fn event(level: Level, msg: &str) -> String {
    format!(
        "@@tryout {{\"level\":\"{}\",\"msg\":{}}}\n",
        level.name(),
        json_str(&strip_sgr(msg))
    )
}

/// Text from elsewhere (a Gerrit subject or name, a commit message) made safe
/// to print on one line: whitespace controls become spaces, every other
/// control character — ESC above all, which could retitle the terminal, write
/// the clipboard or rewrite the line — is dropped.
pub fn printable(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\n' | '\t' | '\r' => Some(' '),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

/// Drop `ESC[…m` colour sequences, and their spelled-out `\033[…m` form.
pub fn strip_sgr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        let skip = if let Some(r) = rest.strip_prefix('\x1b') {
            sgr_len(r).map(|n| 1 + n)
        } else if let Some(r) = rest.strip_prefix("\\033") {
            sgr_len(r).map(|n| 4 + n)
        } else {
            None
        };
        match skip {
            Some(n) => rest = &rest[n..],
            None => {
                let c = rest.chars().next().unwrap();
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// Length of `[<digits;>*]m` at the start of `s`.
fn sgr_len(s: &str) -> Option<usize> {
    let body = s.strip_prefix('[')?;
    let end = body.find(|c: char| !(c.is_ascii_digit() || c == ';'))?;
    (body.as_bytes()[end] == b'm').then_some(end + 2)
}

/// A JSON string literal: quotes, backslashes,
/// tab, newline and CR escaped, every other control character dropped, and
/// non-ASCII left as UTF-8.
pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `json_str`, or `null` for an empty value.
pub fn json_str_or_null(s: &str) -> String {
    if s.is_empty() {
        "null".into()
    } else {
        json_str(s)
    }
}

/// Pad to `width` columns counting characters, not bytes, so "Frédéric" or an
/// ellipsis keeps the columns after it aligned.
pub fn pad_display(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_has_a_twin_when_events_are_on() {
        let s = render(Level::Info, &format!("Serving {BOLD}x{NC} \"now\""), true);
        let mut lines = s.lines();
        assert_eq!(
            lines.next().unwrap(),
            format!("{CYAN}==>{NC} Serving {BOLD}x{NC} \"now\"")
        );
        assert_eq!(
            lines.next().unwrap(),
            r#"@@tryout {"level":"info","msg":"Serving x \"now\""}"#
        );
        assert_eq!(
            render(Level::Warn, "w", false),
            format!("{YELLOW}==>{NC} w\n")
        );
    }

    #[test]
    fn the_spelled_out_colour_form_is_stripped_too() {
        assert_eq!(strip_sgr("a\\033[1;33mb\\033[0mc"), "abc");
        assert_eq!(strip_sgr("keep \\033 and [1m"), "keep \\033 and [1m");
    }

    #[test]
    fn a_json_string_survives_quotes_backslashes_and_controls() {
        assert_eq!(
            json_str("a\"b\\c\td\ne\rf\x01g\x1fé"),
            r#""a\"b\\c\td\ne\rfgé""#
        );
        assert_eq!(json_str_or_null(""), "null");
    }

    #[test]
    fn padding_counts_characters() {
        assert_eq!(pad_display("Frédéric", 10), "Frédéric  ");
        assert_eq!(pad_display("long name", 4), "long name");
    }
}
