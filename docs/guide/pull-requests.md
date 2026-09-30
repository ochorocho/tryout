# Pull requests

In [project mode](/guide/project-mode), you can try out a pull request (GitHub)
or a merge request (GitLab) of your project in one step. tryout fetches it,
creates a worktree and serves it at its own URL:

```bash
ddev tryout worktree add --pr 42           # serve pull request 42 as pr-42.<project>.ddev.site
ddev tryout worktree add review --pr 42    # the same, but name the site "review"
```

This is the project-mode version of [Gerrit patches](/guide/gerrit). On a TYPO3
Core checkout, `--pr` does not work; tryout points you to `ddev tryout patch 42`
instead.

## How tryout gets the code

tryout needs no API and no token. GitHub and GitLab publish the code of every
request under a special name (a "ref") in your repository:

| Forge | Ref |
|---|---|
| GitHub | `refs/pull/<n>/head` |
| GitLab | `refs/merge-requests/<n>/head` |

tryout looks at the URL of `origin` to see which one to use (`github` or
`gitlab` in it). For any other host, for example a mirror or your own server, it
tries both.

The steps:

1. **The fetch runs on your computer**, with your git and your login, just like
   `git fetch` in a terminal.
2. The code is stored as `refs/tryout/pr/<n>`. This is tryout's own ref, not a
   branch, so it does not get in the way of your branches.
3. tryout creates the worktree from that ref and serves it. Like any other site,
   it starts with a
   [copy of the primary's database](/guide/project-mode#where-a-new-site-s-data-comes-from).

If `origin` has no request with that number, tryout stops before it creates
anything:

```text
✗ Origin has no pull or merge request #8
✗   → git -C /path/to/project ls-remote origin 'refs/pull/*' 'refs/merge-requests/*'
```

A request is its own starting point, so `--pr` takes no branch. The other `add`
flags work as usual: `--php`, `--db`, `--db-from`, `--db-empty`, `--no-restart`.

## Picking a request in the terminal UI

In a project, the menu (`a`, or a right-click) has **Open a pull request…**. It
lists the open requests of `origin` with number, branch, title and author. For
the list, tryout uses the command-line tool of your forge, if it is installed:

- `gh pr list` for GitHub,
- `glab mr list` for GitLab.

Type to search. `Enter` opens the selected request as `pr-<number>`. If neither
tool is installed, the picker tells you so. `worktree add --pr <n>` works without
them.

Tab completion never goes online. After `--pr`, it only reminds you that a
number is expected.

## Getting new commits of a request

A worktree does not follow the request by itself. To get new commits, remove the
worktree and open the request again:

```bash
ddev tryout worktree remove pr-42 --yes   # delete the old worktree
ddev tryout worktree add --pr 42          # fetch and serve the request again
```
