# Gerrit patches

TYPO3 reviews code changes on [review.typo3.org](https://review.typo3.org), a
Gerrit server. In Core mode, tryout can apply any of these changes to your
checkout with one command. This lets you test a change before it is merged.

```bash
ddev tryout patch 56947              # apply change 56947 to the primary
ddev tryout patch 56947 91003        # several changes, in this order, one rebuild at the end
ddev tryout patch 56947 v13          # apply it to the served site v13 (or: --site v13)
```

What tryout does:

1. It asks Gerrit for the latest version of the change (for example
   `refs/changes/47/56947/12`).
2. It fetches it and adds it on top of the site's code (`git cherry-pick`).
3. It skips changes that are merged, abandoned or already applied.

If a change does not apply cleanly, tryout stops that change and tells you which
one failed:

```text
✗ Cherry-pick failed for change 56947 (merge conflict)
✗   Cherry-pick has been aborted automatically.
✗   → Verify: https://review.typo3.org/c/Packages/TYPO3.CMS/+/56947
```

When `patch <id>` fails, it exits with code 1.

## Browsing open changes

Run `patch` without a change number. If a site besides the primary is served,
tryout first asks which site to patch. Then it lists the 50 newest open changes
**on that site's branch**. A 13.4 site gets 13.4's changes:

```text
? Apply which changes?
  [ ] 95347   [TASK] Skip database setup for database-free…   Wouter Wolters    CR+1 V+1
> [x] 95074   [BUGFIX] Avoid stale deleted state on reproc…   Benni Mack        CR+2 V+2
  [ ] 95671   [BUGFIX] Ensure numeric site identifiers sta…   Oli Bartsch       CR+1 V+1
[↑↓ to move, space to select one, → to all, ← to none, type to filter]
```

The columns are: change number, title, owner and review state (`CR` =
Code-Review, `V` = Verified). Unfinished work starts with `WIP`. Type to filter
the list. `Enter` applies the changes you ticked. After that, tryout asks once
whether to add them to your patch list, so they are applied again on the next
`ddev start`.

More options:

- `--all-branches` lists changes on every branch.
- `--list` prints the list and does not ask. `--list --json` prints only JSON.
- Without a terminal (during `ddev start`, or in a script), a plain `patch`
  applies the changes in your patch list.

To see more than the newest 50 changes, use **Apply Gerrit patch…** in the
[terminal UI](/guide/terminal-ui#applying-gerrit-changes). It searches Gerrit
directly and pages through every open change.

## Seeing and removing applied changes

```bash
ddev tryout status          # shows "Patches: 2 applied", with their titles
ddev tryout reset           # back to the branch: patches removed, site rebuilt
ddev tryout reset v13       # the same, for the served site v13
```

When tryout applies a change, it notes the change number in the git config
(`tryout.change-<Change-Id>`). So lists can show patches by number even without
network.

## Auto-applying patches

To apply changes on every `ddev start` and `ddev restart`, list them in
`.ddev/config.tryout-patches.yaml`:

```yaml
web_environment:
  - TRYOUT_PATCHES=56947,12345
```

On each start, tryout resets TYPO3 Core to the current branch and applies the
listed changes in this order.

This file is yours. An add-on update never overwrites it. Removing the add-on
deletes it only if the list is still empty.
