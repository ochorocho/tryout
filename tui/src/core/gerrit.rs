//! Gerrit's REST API (review.typo3.org), over HTTPS from Rust instead of curl
//! and jq. Every answer starts with the `)]}'` XSSI guard line, which is
//! stripped before parsing.

use std::time::Duration;

use serde_json::Value;

use super::ctx::GERRIT_PROJECT;

/// Why a Gerrit call failed — the 2/3 exit codes the scripts signalled.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Gerrit could not be reached, or answered with an error status.
    Fetch,
    /// The answer was not the JSON expected.
    Parse,
}

/// The API base: review.typo3.org, or TRYOUT_GERRIT_API (for tests).
pub fn api() -> String {
    std::env::var("TRYOUT_GERRIT_API")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| super::ctx::GERRIT_API.into())
}

/// GET `<api><path>` and parse the JSON behind the XSSI guard.
pub fn get(path_and_query: &str) -> Result<Value, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
    let body = agent
        .get(&format!("{}{path_and_query}", api()))
        .call()
        .map_err(|_| Error::Fetch)?
        .body_mut()
        .read_to_string()
        .map_err(|_| Error::Fetch)?;
    parse(&body)
}

/// Strip the guard line and parse. An answer that is not Gerrit JSON loses its
/// only line here — that is a parse failure, not "nothing found".
pub fn parse(body: &str) -> Result<Value, Error> {
    let json = body.split_once('\n').map_or("", |(_, rest)| rest);
    if json.trim().is_empty() {
        return Err(Error::Parse);
    }
    serde_json::from_str(json).map_err(|_| Error::Parse)
}

/// One open change, as the picker shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub number: u64,
    /// "WIP " + the subject, one line, cut to 68 characters.
    pub subject: String,
    pub owner: String,
    /// "CR+2 V+1", or empty.
    pub scores: String,
}

impl Change {
    /// The TSV row the bash listing printed.
    pub fn tsv(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}",
            self.number, self.subject, self.owner, self.scores
        )
    }
}

/// The open changes for `branch` ("-" for every branch), newest first.
pub fn list_open(branch: &str, limit: u32) -> Result<Vec<Change>, Error> {
    let mut query = format!("project:{GERRIT_PROJECT}+status:open");
    if branch != "-" {
        query.push_str(&format!("+branch:{branch}"));
    }
    changes(&get(&format!(
        "/changes/?q={query}&n={limit}&o=LABELS&o=DETAILED_ACCOUNTS"
    ))?)
}

pub fn changes(v: &Value) -> Result<Vec<Change>, Error> {
    let list = v.as_array().ok_or(Error::Parse)?;
    list.iter()
        .map(|c| {
            let number = c
                .get("_number")
                .and_then(Value::as_u64)
                .ok_or(Error::Parse)?;
            let subject = one_line(
                c.get("subject")
                    .and_then(Value::as_str)
                    .unwrap_or("no subject"),
            );
            let subject = if subject.chars().count() > 68 {
                format!("{}...", subject.chars().take(65).collect::<String>())
            } else {
                subject
            };
            let wip = if c.get("work_in_progress").and_then(Value::as_bool) == Some(true) {
                "WIP "
            } else {
                ""
            };
            let owner = one_line(
                c.pointer("/owner/name")
                    .and_then(Value::as_str)
                    .unwrap_or("?"),
            );
            let scores: Vec<String> = [("Code-Review", "CR"), ("Verified", "V")]
                .iter()
                .filter_map(|(label, short)| score(c, label).map(|s| format!("{short}{s}")))
                .collect();
            Ok(Change {
                number,
                subject: format!("{wip}{subject}"),
                owner,
                scores: scores.join(" "),
            })
        })
        .collect()
}

/// A label's state: approved +2, rejected -2, else the sign of its value.
fn score(change: &Value, label: &str) -> Option<&'static str> {
    let l = change.get("labels")?.get(label)?;
    let truthy = |k: &str| {
        l.get(k)
            .is_some_and(|v| !v.is_null() && v != &Value::Bool(false))
    };
    let value = l.get("value").and_then(Value::as_i64).unwrap_or(0);
    if truthy("approved") {
        Some("+2")
    } else if truthy("rejected") {
        Some("-2")
    } else if value > 0 {
        Some("+1")
    } else if value < 0 {
        Some("-1")
    } else {
        None
    }
}

/// No newlines or tabs: they would break the row format.
fn one_line(s: &str) -> String {
    s.replace(['\n', '\t'], " ")
}

/// `patch --list --json`: the changes as one JSON array of objects.
pub fn changes_json(changes: &[Change]) -> String {
    use super::out::json_str;
    let items: Vec<String> = changes
        .iter()
        .map(|c| {
            format!(
                "{{\"number\":{},\"subject\":{},\"owner\":{},\"scores\":{}}}",
                c.number,
                json_str(&c.subject),
                json_str(&c.owner),
                json_str(&c.scores)
            )
        })
        .collect();
    format!("[{}]\n", items.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = r#")]}'
[{"_number":91234,"subject":"[BUGFIX] Keep\tthe\nline","work_in_progress":true,"owner":{"name":"Ada"},
  "labels":{"Code-Review":{"approved":{"_account_id":1}},"Verified":{"value":-1}}},
 {"_number":91000,"subject":"[FEATURE] A subject that is long enough to need cutting because it goes on and on","owner":{},
  "labels":{"Code-Review":{"value":0},"Verified":{"rejected":{"_account_id":2}}}}]
"#;

    #[test]
    fn a_listing_becomes_pickable_rows() {
        let c = changes(&parse(LISTING).unwrap()).unwrap();
        assert_eq!(
            c[0].tsv(),
            "91234\tWIP [BUGFIX] Keep the line\tAda\tCR+2 V-1"
        );
        assert_eq!(c[1].subject.chars().count(), 68);
        assert!(c[1].subject.ends_with("..."));
        assert_eq!(c[1].owner, "?");
        assert_eq!(c[1].scores, "V-2");
    }

    #[test]
    fn a_non_json_answer_is_a_parse_failure_not_an_empty_list() {
        assert_eq!(parse("<html>proxy error</html>"), Err(Error::Parse));
        assert_eq!(parse(")]}'\n"), Err(Error::Parse));
        assert_eq!(parse(")]}'\n[]").unwrap(), Value::Array(vec![]));
    }

    #[test]
    fn the_json_carries_every_field() {
        let c = changes(&parse(LISTING).unwrap()).unwrap();
        assert!(changes_json(&c[..1]).starts_with(
            r#"[{"number":91234,"subject":"WIP [BUGFIX] Keep the line","owner":"Ada","scores":"CR+2 V-1"}"#
        ));
    }
}
