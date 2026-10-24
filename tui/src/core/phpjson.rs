//! JSON written the way the add-on's PHP generators wrote it —
//! `json_encode($x, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES)` — so the files
//! they own (composer.tryout.json above all) do not churn when Rust takes over:
//! four-space indent, `"key": value`, slashes as is, and every non-ASCII
//! character as a lowercase `\uXXXX` escape (a surrogate pair above the BMP).

use std::io::{self, Write};

use serde::Serialize;
use serde_json::ser::{CharEscape, Formatter, PrettyFormatter};

/// Pretty JSON plus the trailing newline the generators appended.
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> String {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, PhpFormatter::new());
    value
        .serialize(&mut ser)
        .expect("serialising to memory cannot fail");
    buf.push(b'\n');
    String::from_utf8(buf).expect("the formatter writes ASCII only")
}

struct PhpFormatter<'a>(PrettyFormatter<'a>);

impl PhpFormatter<'_> {
    fn new() -> Self {
        Self(PrettyFormatter::with_indent(b"    "))
    }
}

impl Formatter for PhpFormatter<'_> {
    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        for c in fragment.chars() {
            if c.is_ascii() {
                w.write_all(&[c as u8])?;
            } else {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    write!(w, "\\u{u:04x}")?;
                }
            }
        }
        Ok(())
    }

    fn write_char_escape<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        escape: CharEscape,
    ) -> io::Result<()> {
        // PHP escapes the same set serde_json does, with the same short forms.
        serde_json::ser::CompactFormatter.write_char_escape(w, escape)
    }

    fn begin_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_array(w)
    }
    fn end_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_array(w)
    }
    fn begin_array_value<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.0.begin_array_value(w, first)
    }
    fn end_array_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_array_value(w)
    }
    fn begin_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_object(w)
    }
    fn end_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object(w)
    }
    fn begin_object_key<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.0.begin_object_key(w, first)
    }
    fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_object_value(w)
    }
    fn end_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object_value(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matches_php_pretty_print() {
        let v = json!({
            "_comment": "tryout — generated",
            "url": "../../typo3/sysext/*",
            "emoji": "🙂",
            "quote": "a \"b\" \\ c\n",
            "empty_list": [],
            "empty_map": {},
            "n": 1,
            "list": [true, null]
        });
        // What PHP 8 prints for the same value, key order included.
        let want = r#"{
    "_comment": "tryout \u2014 generated",
    "url": "../../typo3/sysext/*",
    "emoji": "\ud83d\ude42",
    "quote": "a \"b\" \\ c\n",
    "empty_list": [],
    "empty_map": {},
    "n": 1,
    "list": [
        true,
        null
    ]
}
"#;
        assert_eq!(to_string(&v), want);
    }
}
