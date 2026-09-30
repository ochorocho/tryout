# TYPO3 Core mode

Use Core mode to work on TYPO3 Core itself: test a patch, fix a bug, or try a
new version. The project folder **is** a clone of the TYPO3 Core repository.
tryout builds working TYPO3 sites from it. When you edit a file in
`typo3/sysext/`, the change is live right away. You do not need to reinstall.

The project looks like this:

```text
my-typo3-site/
├── typo3/sysext/            TYPO3 Core source code (the project root is the clone)
├── Build/                   Core's own build tools, not touched by tryout
├── packages/                your own extensions
├── TYPO3-Instances/
│   ├── primary/             the site at the project URL (docroot: public/)
│   └── <name>/              the site of a served worktree
└── worktrees/<name>/        more checkouts of TYPO3 Core
```

More details are in [architecture](/reference/architecture).

## How a TYPO3 site is built

A site does not contain its own copy of TYPO3 Core. It uses the code from the
checkout:

- Each site has a file `composer.tryout.json`, called the **overlay**. It tells
  Composer to take the TYPO3 system extensions from the checkout's
  `typo3/sysext/`, and your extensions from `packages/`.
- Composer links them into `vendor/`. So the code you edit is the code that
  runs.
- tryout never changes your `composer.json`. The setting
  `COMPOSER=composer.tryout.json` makes Composer use the overlay. The overlay
  includes your `composer.json`, so your own dependencies are installed too.

On every `ddev start`, tryout does these steps inside the web container:

1. It clones TYPO3 Core if the project has no clone yet.
2. It resets to the current branch and applies your
   [patch list](/guide/gerrit#auto-applying-patches), if you have one.
3. It updates the overlay and runs `composer install`.
4. On the first start, it sets up TYPO3 (with a styleguide demo frontend on
   13.4 and newer).
5. It runs `extension:setup` and clears the caches, for the primary and every
   served site.

## Switching TYPO3 versions

tryout starts on the `main` branch. To work on another version:

```bash
ddev tryout checkout 13.4          # switch the primary site to 13.4
ddev tryout checkout 13.4 v13      # switch the served site v13 (or: --site v13)
ddev tryout checkout               # choose from a list of branches
```

`checkout` fetches the latest code, switches the checkout to the branch, updates
the overlay, reinstalls `vendor/` and rebuilds the site. Each TYPO3 version has
different system extensions. The overlay always lists exactly the ones in the
checkout.

Several sites can run the same version at the same time. A served site's
worktree switches without taking the branch for itself ("detached"). Only the
project root takes the branch. If an older worktree still holds that branch,
tryout lets it go first. That worktree keeps the same code, and the branch keeps
its commits.

::: tip
TYPO3 works with a `main`-based workflow. Submit your fix against `main`, even if
it is for an older version. The TYPO3 team backports it.
:::

To choose the branch for new clones, set it in `.ddev/config.local.yaml`:

```yaml
web_environment:
  - TRYOUT_BRANCH=13.4
```

## Updating and resetting

```bash
ddev tryout download               # get the latest code of the current branch
ddev tryout download --reset       # (or -r) reset to origin/<branch>; local changes are lost
ddev tryout download v13 --reset   # the same, for the served site v13
ddev tryout reset                  # reset Core to the branch (removes applied patches) and rebuild
ddev tryout reset v13              # the same, for v13
ddev tryout composer               # only update the overlay (it does not run Composer)
```

Good to know:

- A worktree does not hold a branch. So a plain `download` on it stops, instead
  of throwing away commits you may not have pushed. Use `--reset` to reset it to
  its base branch, or `checkout` to switch versions.
- tryout finds a worktree's base branch by looking for the nearest commit that
  is on an `origin/*` branch.
- `ddev tryout composer` takes no arguments. To run Composer, use
  `ddev composer …`.

## Starting over

```bash
ddev tryout delete --yes           # the primary: new database, empty fileadmin, new settings.php
ddev tryout delete v13 --yes       # the served site v13
ddev tryout delete --all --yes     # every site, the primary too
```

Without `--yes`, `delete` lists what it will delete and asks you first. Without a
terminal to ask in, it stops and tells you to add `--yes`. With `--yes` it asks
nothing at all. If you give no site name, it means the primary.

## Several Core versions in one project

A worktree is another checkout of the same TYPO3 Core clone. It shares the
clone's history, so it needs no second download and little disk space:

```bash
ddev tryout worktree add v13 13.4          # create worktrees/v13 at 13.4
ddev tryout worktree use v13               # the primary site now runs v13's code
ddev tryout worktree use main              # back to the project root's code
ddev tryout worktree serve v13             # or: v13 on its own URL, PHP and database
```

There are two ways to use worktrees:

- **`use`**: one site at the project URL. You choose which checkout it runs.
  tryout points the primary's overlay at the worktree and reinstalls `vendor/`.
  `status` warns you if the two ever do not match.
- **`serve`**: every worktree runs at the same time at
  `<name>.<project>.ddev.site`, each with its own site in
  `TYPO3-Instances/<name>/`. See [served sites](/guide/sites).

A served worktree gets the newest PHP version **its own** TYPO3 version allows
(`require.php` in that branch's `composer.json`). So a 13.4 worktree and a
`main` worktree each get a PHP version that fits. Before every Composer run,
tryout checks the PHP version. If it does not fit, you see the fix:

```text
✗ TYPO3 Core requires PHP ^8.2, but site 'v13' runs PHP 8.1
✗   → ddev tryout worktree serve v13 --php 8.4
```

Removing worktrees:

- You cannot remove the worktree the primary runs, or the project root.
- `worktree remove` always asks first and names the folder. `--yes` skips the
  question.
- A worktree from an older tryout version may still hold a branch. `remove`
  deletes that branch if it is merged and keeps it otherwise. `--force` deletes
  it anyway.

## Your own extensions

`packages/` is a folder Composer reads extensions from. Put an extension there
and install it:

```bash
ddev composer require myvendor/my-extension:@dev   # install the extension
ddev typo3 extension:setup                         # activate it and update the database
```

Run `extension:setup` after every `composer require`. Removing the add-on never
deletes `packages/`.

## Several projects

Each tryout is a normal DDEV project. For separate projects, use separate
folders. Each has its own Core clone, database and patches:

```bash
mkdir tryout-v13 && cd tryout-v13
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
ddev add-on get bmack/tryout && ddev start
```

::: warning
Older versions suggested sharing one TYPO3 Core clone between projects. That no
longer works: git runs inside the web container, and it cannot reach a clone
outside the project. Use worktrees inside one project instead.
:::
