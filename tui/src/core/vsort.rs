//! `sort -V`: GNU's version order (gnulib `filevercmp`), for branches and PHP
//! versions. Digit runs compare as numbers, letters sort
//! before other characters, `~` before everything, and a file-like suffix
//! (`.tar.gz`) only breaks ties. Equal versions fall back to a byte compare, as
//! sort does without `-s`.

use std::cmp::Ordering;

pub fn cmp(a: &str, b: &str) -> Ordering {
    filevercmp(a.as_bytes(), b.as_bytes()).then_with(|| a.cmp(b))
}

/// Sort ascending in `sort -V` order.
pub fn sort(v: &mut [String]) {
    v.sort_by(|a, b| cmp(a, b));
}

fn filevercmp(a: &[u8], b: &[u8]) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    match (a.is_empty(), b.is_empty()) {
        (true, _) => return Ordering::Less,
        (_, true) => return Ordering::Greater,
        _ => {}
    }
    // "." first, then "..", then other dot-names, then the rest.
    match (a[0] == b'.', b[0] == b'.') {
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        (true, true) => {
            for special in [&b"."[..], &b".."[..]] {
                match (a == special, b == special) {
                    (true, _) => return Ordering::Less,
                    (_, true) => return Ordering::Greater,
                    _ => {}
                }
            }
        }
        _ => {}
    }
    let (ap, bp) = (prefix_len(a), prefix_len(b));
    match verrevcmp(&a[..ap], &b[..bp]) {
        Ordering::Equal => verrevcmp(a, b),
        o => o,
    }
}

/// Length of `s` without a trailing `(\.[A-Za-z~][A-Za-z0-9~]*)*`.
fn prefix_len(s: &[u8]) -> usize {
    let n = s.len();
    let mut prefix = 0;
    let mut i = 0;
    loop {
        if i == n {
            return prefix;
        }
        i += 1;
        prefix = i;
        while i + 1 < n && s[i] == b'.' && (s[i + 1].is_ascii_alphabetic() || s[i + 1] == b'~') {
            i += 2;
            while i < n && (s[i].is_ascii_alphanumeric() || s[i] == b'~') {
                i += 1;
            }
        }
    }
}

fn order(c: u8) -> i32 {
    if c.is_ascii_digit() {
        0
    } else if c.is_ascii_alphabetic() {
        c as i32
    } else if c == b'~' {
        -1
    } else {
        c as i32 + 256
    }
}

fn verrevcmp(a: &[u8], b: &[u8]) -> Ordering {
    let at = |i: usize| a.get(i).copied();
    let bt = |i: usize| b.get(i).copied();
    let digit = |c: Option<u8>| c.is_some_and(|c| c.is_ascii_digit());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        while (i < a.len() && !digit(at(i))) || (j < b.len() && !digit(bt(j))) {
            let ac = at(i).map_or(0, order);
            let bc = bt(j).map_or(0, order);
            if ac != bc {
                return ac.cmp(&bc);
            }
            i += 1;
            j += 1;
        }
        while at(i) == Some(b'0') {
            i += 1;
        }
        while bt(j) == Some(b'0') {
            j += 1;
        }
        let mut first_diff = 0i32;
        while digit(at(i)) && digit(bt(j)) {
            if first_diff == 0 {
                first_diff = at(i).unwrap() as i32 - bt(j).unwrap() as i32;
            }
            i += 1;
            j += 1;
        }
        if digit(at(i)) {
            return Ordering::Greater;
        }
        if digit(bt(j)) {
            return Ordering::Less;
        }
        if first_diff != 0 {
            return first_diff.cmp(&0);
        }
    }
    Ordering::Equal
}

/// The order a branch picker offers: main, then release branches newest first,
/// then everything else (the legacy TYPO3_x-y refs) — `sort -rV` for both groups.
pub fn picker_order(branches: &[String]) -> Vec<String> {
    let mut rest: Vec<String> = branches.iter().filter(|b| *b != "main").cloned().collect();
    sort(&mut rest);
    rest.reverse();
    let (numeric, other): (Vec<_>, Vec<_>) = rest
        .into_iter()
        .partition(|b| b.as_bytes().first().is_some_and(u8::is_ascii_digit));
    let mut out = vec!["main".to_string()];
    out.extend(numeric);
    out.extend(other);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(v: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = v.iter().map(|s| s.to_string()).collect();
        sort(&mut v);
        v
    }

    #[test]
    fn orders_like_sort_v() {
        // The expected order is what `sort -V` prints for the same input.
        let input = [
            "main",
            "13.4",
            "12.4",
            "TYPO3_4-5",
            "11.5",
            "9.5",
            "10.4",
            "TYPO3_6-2",
            "8.10",
            "8.9",
            "8.2",
            "1.0~rc1",
            "1.0",
            "a.tar.gz",
            "a1",
            "a01",
            "feature-x",
            "14.0",
            "v1.2",
        ];
        let want = [
            "1.0~rc1",
            "1.0",
            "8.2",
            "8.9",
            "8.10",
            "9.5",
            "10.4",
            "11.5",
            "12.4",
            "13.4",
            "14.0",
            "TYPO3_4-5",
            "TYPO3_6-2",
            "a.tar.gz",
            "a01",
            "a1",
            "feature-x",
            "main",
            "v1.2",
        ];
        assert_eq!(sorted(&input), want);
    }

    #[test]
    fn the_picker_puts_main_first_and_legacy_refs_last() {
        let v: Vec<String> = ["9.5", "TYPO3_8-7", "13.4", "main", "14.3", "12.4"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            picker_order(&v),
            ["main", "14.3", "13.4", "12.4", "9.5", "TYPO3_8-7"]
        );
    }
}
