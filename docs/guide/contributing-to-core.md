# Contributing to TYPO3 Core

A new Core-mode project can fetch and test changes, but it cannot send your own
changes for review yet. Run `ddev tryout cs` once to set this up:

```bash
ddev tryout cs              # asks for your review.typo3.org username
ddev tryout cs setup jdoe   # or give the username directly
```

You only need to do this once per project. Nothing runs on `ddev start`. `cs`
does six things:

1. **Finds your Gerrit username.** It uses the argument, or else
   `TRYOUT_GERRIT_USER`, or else the saved `tryout.gerritUser` git setting, or
   else it asks you. Use the username shown at
   <https://review.typo3.org/settings/>.
2. **Installs the Gerrit `commit-msg` hook.** It adds the `Change-Id` line that
   Gerrit needs to every commit.
3. **Installs TYPO3 Core's `pre-commit` hook.** It checks the coding guidelines
   before each commit.
4. **Sets up a commit message template** (`commit.template`). `git commit`
   without `-m` opens a TYPO3-style message to fill in: `[BUGFIX]`,
   `Resolves:`, `Releases:` and so on.
5. **Sets the Gerrit SSH push URL on `origin`**, so that
   `git push origin HEAD:refs/for/main` sends your change for review.
6. **Sets your commit author** (`user.email` and `user.name` for this
   repository) from your Gerrit account. Gerrit rejects a commit whose email
   address does not belong to your account. If another address was set before,
   `cs` shows how to fix earlier commits
   (`git commit --amend --reset-author --no-edit`).

All worktrees share the project root's git settings and hooks. So one `cs`
covers every checkout.

To use the same username in every project, set it in `.ddev/config.local.yaml`:

```yaml
web_environment:
  - TRYOUT_GERRIT_USER=jdoe
```

## Checking the setup

```bash
ddev tryout cs doctor
```

`doctor` checks each part: the hooks, the template, the push URL and the author.
It also tests the SSH connection to Gerrit twice:

- **from the web container**, where `ddev auth ssh` provides your SSH keys,
- **from your computer**, whose SSH agent is used when you `git push` in a
  normal terminal.

The result comes from `ssh` itself. Your public SSH key must be added at
<https://review.typo3.org/settings/#SSHKeys>. If the network blocks the port,
`doctor` tells you so.

## Sending a change for review

```bash
git commit                                  # the template opens
git push origin HEAD:refs/for/main          # send it to Gerrit
```

You can push from any checkout, worktrees included. The push always sends
`HEAD`, whether it is on a branch or not.

::: tip
TYPO3 works with a `main`-based workflow. Send your change against `main`, even
if the fix is for an older version.
:::

## Undoing the setup

```bash
ddev tryout cs uninstall
```

This removes both hooks and the commit template, sets the push URL back to the
fetch URL and forgets the saved username. Your commit author settings stay as
they are.

Contributions to TYPO3 Core itself do not go through the tryout repository.
TYPO3 Core is developed on [review.typo3.org](https://review.typo3.org).
