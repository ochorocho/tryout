//! Pull and merge requests of a project of your own — project mode's answer to
//! Gerrit patches. Fetching needs no API: GitHub serves every pull request as
//! `refs/pull/<n>/head`, GitLab every merge request as
//! `refs/merge-requests/<n>/head`, and the host's git has the user's
//! credentials. Only the picker's list asks `gh` or `glab`, where installed.

use std::path::Path;

use serde_json::Value;

use super::gerrit::Change;
use super::{git, out, proc};

/// Where the project's origin lives, as far as its URL tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    GitLab,
    /// A mirror, a self-hosted server or a path: both ref layouts are tried.
    Unknown,
}

pub fn forge(origin_url: &str) -> Forge {
    let u = origin_url.to_lowercase();
    if u.contains("github") {
        Forge::GitHub
    } else if u.contains("gitlab") {
        Forge::GitLab
    } else {
        Forge::Unknown
    }
}

/// The refs a request's head may be published under, most likely first.
pub fn head_refs(forge: Forge, number: u64) -> Vec<String> {
    let github = format!("refs/pull/{number}/head");
    let gitlab = format!("refs/merge-requests/{number}/head");
    match forge {
        Forge::GitHub => vec![github],
        Forge::GitLab => vec![gitlab],
        Forge::Unknown => vec![github, gitlab],
    }
}

/// Where tryout keeps a fetched request's head: a ref of its own, never a
/// branch, so it collides with nothing and a worktree starts detached on it.
pub fn local_ref(number: u64) -> String {
    format!("refs/tryout/pr/{number}")
}

/// `--pr 123` / `#123`: the number, when it is one.
pub fn parse_number(s: &str) -> Option<u64> {
    s.trim_start_matches('#').parse().ok().filter(|n| *n > 0)
}

fn origin(root: &Path) -> Option<String> {
    git::out(root, &["remote", "get-url", "origin"]).filter(|u| !u.is_empty())
}

/// Fetch request `number`'s head from origin into `local_ref`. Runs on the
/// host, with the user's credentials. The error says what to do.
pub fn fetch(root: &Path, number: u64) -> Result<String, Vec<String>> {
    let Some(url) = origin(root) else {
        return Err(vec![
            "This project has no origin to fetch a pull request from".into(),
            "  → git remote add origin <url>".into(),
        ]);
    };
    let local = local_ref(number);
    for r in head_refs(forge(&url), number) {
        out::info(format!("Fetching {r} from origin..."));
        if proc::quiet(
            "git",
            &[
                "-C",
                &root.to_string_lossy(),
                "fetch",
                "-q",
                "origin",
                &format!("+{r}:{local}"),
            ],
            None,
        ) {
            return Ok(local);
        }
    }
    Err(vec![
        format!("Origin has no pull or merge request #{number}"),
        format!(
            "  → git -C {} ls-remote origin 'refs/pull/*' 'refs/merge-requests/*'",
            root.display()
        ),
    ])
}

/// The open requests, as the picker lists them — `gh pr list` or `glab mr
/// list`, whichever fits origin and is installed. Titles and names are the
/// remote's text: made printable.
pub fn list_open(root: &Path, search: &str) -> Result<Vec<Change>, String> {
    let url = origin(root).ok_or("this project has no origin")?;
    let tools: &[Forge] = match forge(&url) {
        Forge::GitHub => &[Forge::GitHub],
        Forge::GitLab => &[Forge::GitLab],
        Forge::Unknown => &[Forge::GitHub, Forge::GitLab],
    };
    let mut last = String::from("neither gh nor glab is installed — open one by number instead");
    for tool in tools {
        let (bin, args): (&str, Vec<&str>) = match tool {
            Forge::GitHub => (
                "gh",
                vec![
                    "pr",
                    "list",
                    "--state",
                    "open",
                    "--limit",
                    "50",
                    "--json",
                    "number,title,author,headRefName,isDraft",
                    "--search",
                    search,
                ],
            ),
            _ => (
                "glab",
                vec![
                    "mr",
                    "list",
                    "--per-page",
                    "50",
                    "-F",
                    "json",
                    "--search",
                    search,
                ],
            ),
        };
        let Some(o) = proc::capture(bin, &args, Some(root)) else {
            continue;
        };
        if !o.status.success() {
            let err = String::from_utf8_lossy(&o.stderr);
            last = format!(
                "{bin}: {}",
                out::printable(err.lines().next().unwrap_or("failed"))
            );
            continue;
        }
        return parse(*tool, &String::from_utf8_lossy(&o.stdout))
            .ok_or(format!("{bin}'s answer was not understood"));
    }
    Err(last)
}

/// `gh pr list --json …` or `glab mr list -F json` as picker rows: number,
/// title (a draft marked), author, and the branch in the scores column.
pub fn parse(tool: Forge, json: &str) -> Option<Vec<Change>> {
    let v: Value = serde_json::from_str(json).ok()?;
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let rows = v.as_array()?.iter().filter_map(|r| {
        let (number, draft, author, branch) = match tool {
            Forge::GitHub => (
                r.get("number")?.as_u64()?,
                r.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
                r.get("author").map(|a| s(a, "login")).unwrap_or_default(),
                s(r, "headRefName"),
            ),
            _ => (
                r.get("iid")?.as_u64()?,
                r.get("draft").and_then(Value::as_bool).unwrap_or(false),
                r.get("author")
                    .map(|a| s(a, "username"))
                    .unwrap_or_default(),
                s(r, "source_branch"),
            ),
        };
        let title = format!("{}{}", if draft { "Draft " } else { "" }, s(r, "title"));
        Some(Change {
            number,
            subject: out::printable(&title).chars().take(68).collect(),
            owner: out::printable(&author),
            scores: out::printable(&branch).chars().take(30).collect(),
        })
    });
    Some(rows.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forge_decides_where_a_request_is_fetched_from() {
        assert_eq!(forge("git@github.com:acme/shop.git"), Forge::GitHub);
        assert_eq!(
            forge("https://gitlab.example.org/acme/shop.git"),
            Forge::GitLab
        );
        assert_eq!(forge("/srv/git/shop.git"), Forge::Unknown);
        assert_eq!(head_refs(Forge::GitHub, 7), ["refs/pull/7/head"]);
        assert_eq!(head_refs(Forge::GitLab, 7), ["refs/merge-requests/7/head"]);
        assert_eq!(head_refs(Forge::Unknown, 7).len(), 2);
        assert_eq!(local_ref(7), "refs/tryout/pr/7");
        assert_eq!(parse_number("#12"), Some(12));
        assert_eq!(parse_number("0"), None);
        assert_eq!(parse_number("12; rm"), None);
    }

    #[test]
    fn gh_and_glab_answers_become_picker_rows() {
        let gh = r#"[{"author":{"login":"ada"},"headRefName":"feat/x","isDraft":true,"number":12,"title":"Add \u001b[31mX"}]"#;
        let rows = parse(Forge::GitHub, gh).unwrap();
        assert_eq!(rows[0].number, 12);
        assert_eq!(rows[0].owner, "ada");
        assert_eq!(rows[0].scores, "feat/x");
        assert!(rows[0].subject.starts_with("Draft Add"));
        assert!(!rows[0].subject.contains('\u{1b}'), "{:?}", rows[0].subject);

        let glab = r#"[{"iid":3,"title":"Fix","author":{"username":"bob"},"source_branch":"fix","draft":false}]"#;
        let rows = parse(Forge::GitLab, glab).unwrap();
        assert_eq!((rows[0].number, rows[0].owner.as_str()), (3, "bob"));
        assert_eq!(parse(Forge::GitHub, "not json"), None);
    }

    #[test]
    fn a_request_is_fetched_from_a_remote_that_publishes_it() {
        let g = |dir: &Path, args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t", "-C"])
                .arg(dir)
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "{args:?}");
        };
        // A bare origin that publishes request 7, as a forge would.
        let remote = tempfile::tempdir().unwrap();
        g(remote.path(), &["init", "-q", "--bare"]);
        let work = crate::core::ctx::tests::project_repo();
        g(
            work.path(),
            &["remote", "add", "origin", &remote.path().to_string_lossy()],
        );
        g(
            work.path(),
            &["push", "-q", "origin", "feature:refs/pull/7/head"],
        );

        assert_eq!(fetch(work.path(), 7).as_deref(), Ok("refs/tryout/pr/7"));
        assert!(git::ok(
            work.path(),
            &["rev-parse", "--verify", "-q", "refs/tryout/pr/7"]
        ));
        assert!(fetch(work.path(), 8).is_err());
    }
}
