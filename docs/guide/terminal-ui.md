# Terminal UI

`ddev tryout ui` opens a full-screen view of your project in the terminal. You
can see all worktrees, open shells in them and run tryout commands from menus,
without typing the commands yourself.

```bash
ddev tryout ui        # open (or come back to) the terminal UI
ddev tryout ui stop   # end it
```

The screen has four parts:

- **Worktrees**, on the left. Each worktree has three lines: its name; its
  branch, patches and commit; its PHP version and database. A served worktree
  also shows its URL.
- **Agents**, below the worktrees. Every tab that runs a coding agent (claude,
  codex, …), marked as working, waiting for you, or idle.
- **Activity**, below that. The commands you started, each with its current
  step, then ✓ (done) or ✗ (failed).
- **Tabs**, on the right. The shells of the selected worktree, as many as you
  open. They keep running while you look at another worktree.

Press `a` or right-click a worktree to see the commands for it. If a command
needs input (a name, a branch, a change, or a confirmation before it deletes
something), a form asks for it. The command then runs in the Activity panel
while you keep working. If a command needs your password (DDEV does for
`/etc/hosts`), a popup asks for it.

The keys below work while the **list** on the left is active. Inside a shell,
every key goes to the shell, except `Ctrl-G`, which brings you back to the list.

## Moving around

| Keyboard            | Mouse                                 | Does                                                        |
|---------------------|---------------------------------------|-------------------------------------------------------------|
| `↑` `↓` or `k` `j`  | click a worktree                      | select a worktree                                           |
| `Home` / `End`      |                                       | first / last worktree                                       |
| `Enter`, `→` or `l` | click in the pane                     | into the worktree's active shell (a new one if it has none) |
| `Ctrl-G`            |                                       | out of the shell, back to the list                          |
| `{` / `}`           | drag the border between list and pane | narrow / widen the list (double-click the border resets it) |
| `r`                 |                                       | reload the list                                             |

## Shells and tabs

| Keyboard  | Mouse                    | Does                                                                                    |
|-----------|--------------------------|-----------------------------------------------------------------------------------------|
| `t`       | click `+` after the tabs | open a new shell tab in the selected worktree                                           |
| `1` … `9` | click a tab              | switch to that tab                                                                      |
| `[` / `]` |                          | previous / next tab                                                                     |
| `<` / `>` | drag a tab               | move the tab left / right                                                               |
| `,`       | double-click a tab       | rename it (Enter keeps it, Esc cancels, an empty name goes back to its program's title) |
| `w`       |                          | close the tab                                                                           |

## Commands

| Keyboard       | Mouse                          | Does                                                                          |
|----------------|--------------------------------|-------------------------------------------------------------------------------|
| `a` or `Space` | right-click a worktree         | the commands for that worktree, then the project-wide ones — the same menu both ways |
| `+`            | click **+ new** above the list | create a worktree: asks its name, the branch, and whether to serve it now     |
|                | click the URL in the title     | open that site in the browser                                                 |

Inside the menu:

| Keyboard            | Does                                                        |
|---------------------|-------------------------------------------------------------|
| `↑` `↓` or `k` `j`  | choose                                                      |
| `Enter`, `→` or `l` | run it, or open a submenu (marked ▸, e.g. **Serve on PHP**) |
| `←`, `Esc` or `h`   | out of a submenu                                            |
| `Esc` or `q`        | close the menu                                              |

The menu only shows commands that fit the worktree and the
[mode](/guide/introduction#two-modes). An entry that ends in … asks something
first.

| Entry | Mode | When |
|---|---|---|
| Serve · Serve on PHP ▸ · Serve on database ▸ | both | not served |
| Serve with an empty database | project | not served |
| PHP ▸ · Database ▸ (switch; the current one ticked) | both | served, not the primary |
| Open site | both | served |
| Open backend | Core | served |
| Make primary | Core | not the primary |
| Unserve · Unserve and drop its database | both | served, not the primary |
| Update from its base branch · Switch TYPO3 version… · Apply Gerrit patch… · Reset Core + rebuild | Core | served |
| Run command… | both | served |
| Fresh install… | Core | served |
| Reset its database… | project | served |
| Rename… · Remove… | both | not the root checkout |
| Status | both | always |
| Regenerate the overlay | Core | always |
| Open a pull request… | project | always |

## Forms

| Keyboard                      | Does                                                 |
|-------------------------------|------------------------------------------------------|
| `Tab` / `Shift-Tab`           | next / previous field                                |
| `Enter`                       | submit                                               |
| `Esc`                         | cancel                                               |
| typing, `Backspace`, `Ctrl-U` | edit a text field (`Ctrl-U` clears it)               |
| typing, `↑` `↓`               | in a branch list: filter, then choose                |
| `Space`                       | toggle a checkbox                                    |
| `y`                           | confirm a destructive command; any other key cancels |

## Applying Gerrit changes

**Apply Gerrit patch…** (Core mode) shows a search box and, below it, the open
changes for the worktree's branch, 25 per page. Each row shows the change number,
title, owner and votes (`CR` = Code-Review, `V` = Verified, `·` = no vote yet).

Shortly after you stop typing, the search asks Gerrit (it searches all changes,
not only this page):

- A number finds that change.
- A word with a colon is a Gerrit search option, for example `owner:jdoe`,
  `-is:wip` or `topic:foo`.
- Any other word must appear in the commit message.

You can combine them: `owner:jdoe -is:wip cache`.

| Keyboard                                  | Mouse                         | Does                                                                                         |
|-------------------------------------------|-------------------------------|----------------------------------------------------------------------------------------------|
| typing, `Backspace`                       |                               | edit the search (in the search box, `Space` types a space)                                   |
| `↓` or `Tab`                              |                               | from the search box into the list                                                            |
| `↑` `↓`                                   | wheel                         | choose a change (`↑` on the first row goes back to the search)                               |
| `Space`                                   | click a change                | tick or untick it                                                                            |
| `PgUp` / `PgDn`, or `←` / `→` in the list | click `‹ previous` / `next ›` | turn the page (ticks are kept across pages)                                                  |
| a letter                                  |                               | from the list back to the search, typing it                                                  |
| `Enter`                                   |                               | apply the ticked changes in the order you ticked them, or the selected one if none is ticked |
| `Esc`                                     |                               | cancel                                                                                       |

## Opening a pull request

**Open a pull request…** (project mode) uses the same form. It lists the open
pull or merge requests of `origin` with number, branch, title and author. For
the list, tryout uses `gh` (GitHub) or `glab` (GitLab), if installed. `Enter`
opens the selected request as the served worktree `pr-<number>`. This is the
same as `ddev tryout worktree add --pr <number>`. See
[pull requests](/guide/pull-requests).

## Activity and logs

| Keyboard | Mouse                   | Does                                                                      |
|----------|-------------------------|---------------------------------------------------------------------------|
| `L`      | click an Activity row   | open that command's log (`L` opens the first; `↑` `↓` then walk the list) |
| `R`      | click ↻ on a failed row | run the failed command again                                              |

Commands on different worktrees run at the same time. A patch on `v13` does not
wait for a rebuild of `main`. Two commands on the same worktree run one after
the other.

Some commands always run **alone**: serve, unserve, rename, remove, Make primary
and Regenerate the overlay. They can restart DDEV, which would stop every other
command, or they change settings all sites share. A command that has to wait
says what it waits for (`Serve v13 · after Reset main`). A later command never
jumps ahead of an earlier one it conflicts with.

A command that worked leaves the list after 5 seconds. A failed one stays until
you run it again. Inside the log:

| Keyboard                          | Mouse                 | Does                                                                              |
|-----------------------------------|-----------------------|-----------------------------------------------------------------------------------|
| `↑` `↓`                           |                       | the previous / next command's log, in Activity order (the title shows e.g. `2/4`) |
| `k` `j`                           | wheel (3 lines)       | scroll a line                                                                     |
| `PgUp` / `PgDn`, or `b` / `Space` |                       | scroll a page                                                                     |
| `Home` / `End`, or `g` / `G`      |                       | top / bottom                                                                      |
| `r`                               | click **↻ retry (r)** | run the command again                                                             |
| `Esc`, `q` or `Enter`             |                       | close the log                                                                     |

## Agents

| Keyboard | Mouse          | Does                                                                        |
|----------|----------------|-----------------------------------------------------------------------------|
| `n`      | click an agent | go to the next agent that needs you: waiting first, then working, then idle |

## Popups

| Popup                                     | Keyboard                                                                                                               |
|-------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
| **Password**, when a command asks for one | type it (shown as •), `Enter` sends it to that command, `Esc` or `Ctrl-C` cancels. It is not stored.                   |
| **Rename tab**                            | the old name starts selected, so typing replaces it; `←` `→` `Home` `End` keep it; `Ctrl-U` clears it; `Enter` / `Esc` |
| **Close the session?**                    | `y` closes, any other key keeps it                                                                                     |

While a popup or form is open, clicks outside it do nothing.

## The session

| Keyboard        | Does                                                 |
|-----------------|------------------------------------------------------|
| `q` or `Ctrl-C` | detach: shells, agents and running commands carry on |
| `Q`, then `y`   | close the session and everything in it               |

The terminal UI runs as a **session**, like tmux. When you leave it with `q`,
everything keeps running. The next `ddev tryout ui` shows it exactly as you left
it. Only `Q` or `ddev tryout ui stop` ends it.

- Only one terminal can show the session at a time. Opening it from another
  terminal moves it there.
- You cannot start `ddev tryout ui` inside one of its own shells.
- A session keeps running the tryout version it started with. After an add-on
  update, `ddev tryout ui` tells you and offers to restart the session.

## On the command line: completion and prompts

The normal command line helps you in a similar way.

**Tab completion.** Press `Tab` to complete commands, subcommands and flags, each
with a short description. It also completes the names tryout knows, and only the
ones that fit:

```console
$ ddev tryout worktree serve <TAB>            # only worktrees not served yet
fancy-pants  -- not served
main         -- primary — at the project URL

$ ddev tryout worktree serve wonka --php <TAB>   # what its composer.json accepts
8.5  -- default for wonka

$ ddev tryout exec <TAB>
@primary  -- the primary site at the project URL
benni     -- served worktree · PHP 8.5
```

Good to know:

- A flag you already typed is not offered again.
- A flag that does not fit the mode is not offered (`--pr`, `--db-from` and
  `--db-empty` only in project mode).
- Branch names come from what is already in the project, never from the
  network, so `Tab` is always fast.
- Descriptions show in Zsh and Fish. Bash needs version 4.4 or newer.

Completion needs DDEV's own shell completion, set up once per computer. See
[DDEV's shell completion docs](https://docs.ddev.com/en/stable/users/install/shell-completion/).
With Homebrew, Bash also needs `brew install bash-completion`, and Zsh needs
`$(brew --prefix)/share/zsh/site-functions` on `FPATH` before `compinit`.

**Guided prompts.** If you leave out an argument, the command asks for it
instead of failing:

```console
$ ddev tryout worktree serve          # pick from the worktrees not served yet
$ ddev tryout checkout                # pick a branch: main, then releases newest first
$ ddev tryout worktree add            # asks for the name, then the branch
$ ddev tryout exec                    # pick the site, then type the command
```

Type to filter a list. `Esc` cancels.

Without a terminal (in a script), nobody can answer. A line piped into the
command is used as the answer. If there is nothing to read, the command prints
how to use it and exits with code 1. With `--yes`, a command never asks
anything.
