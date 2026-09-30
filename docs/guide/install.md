# Installation

This page shows how to add tryout to a project, update it and remove it.

## Requirements

- [DDEV](https://docs.ddev.com/) v1.24.10 or newer. The install checks this.
- Docker Desktop, OrbStack or Colima.
- macOS or Linux. On Windows, use WSL2. The install stops in a normal Windows shell.
- Git on your computer. Git 2.48 or newer is best. With an older git, tryout
  still works, but `git worktree` commands on your computer may fail on
  worktrees tryout created. The install tells you if your git is older.
- An SSH client, only if you push to Gerrit from your computer.

You do not need to install anything else. tryout is one program. The add-on
ships a build for each platform, and picks the right one for your computer and
for DDEV's web container.

## Add tryout to your own project

Use this for a DDEV project whose root is a git repository (Laravel, Symfony,
Drupal, WordPress, a TYPO3 site, plain PHP and more):

```bash
ddev add-on get bmack/tryout   # install the add-on
ddev restart                       # load it
```

The install sees that your project is not TYPO3 Core. It prints
"Not a TYPO3 Core checkout: tryout serves this project's own worktrees." and
leaves your project as it is:

- It clones nothing and checks nothing out.
- It creates no `packages/` or `TYPO3-Instances/` folder, no Composer overlay
  and no `additional.php`.
- It adds no patch list and no TYPO3 or `COMPOSER` settings. `ddev composer`
  keeps using your `composer.json`.
- Your `.ddev/config.yaml`, docroot and project type stay the same.

tryout adds its own commands and files in `.ddev/`. It hides the files it
creates from `git status` by listing them in `.git/info/exclude`. It lists only
its own files, because your `.ddev/` folder is probably committed.

Next: [project mode](/guide/project-mode).

## Set up TYPO3 Core

Pick a folder name, for example `my-typo3-site`, and run:

```bash
mkdir my-typo3-site && cd my-typo3-site
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
ddev add-on get bmack/tryout   # also clones TYPO3 Core
ddev start                         # installs and sets up TYPO3
```

`ddev add-on get` clones the TYPO3 Core repository into the project folder. If
it cannot (for example without network), the first `ddev start` clones it
instead.

The install also creates:

- `packages/`, for your own extensions,
- `TYPO3-Instances/primary/`, the main TYPO3 site,
- `composer.tryout.json`, the file Composer uses instead of a `composer.json`
  (the "overlay"),
- `config/system/additional.php`,
- `.ddev/config.tryout-patches.yaml`, your list of Gerrit patches,
- `.ddev/config.tryout-core.yaml`, with the TYPO3 settings and
  `COMPOSER=composer.tryout.json`.

The first `ddev start` installs all TYPO3 Core system extensions with Composer
and sets up TYPO3 with a demo frontend. When it is done, open:

- **Frontend:** a styleguide demo page (TYPO3 13.4 and newer; a 12.4 site has
  the backend only)
- **Backend:** `https://my-typo3-site.ddev.site/typo3/`
- **User:** `admin`, password `Password.1`

Next: [TYPO3 Core mode](/guide/typo3-core).

## Update and remove

```bash
ddev add-on get bmack/tryout   # update: run the install again
ddev restart
ddev add-on remove tryout          # remove the add-on
```

DDEV copies the add-on into your project once. It does not update itself. An
older copy keeps working, with the commands it had. `ddev tryout status` tells
you when your copy is older than the one you installed from:

```text
  ! This project runs an older copy of the tryout add-on
    the command and its tab-completion offer the previous feature set
    → ddev add-on get <path-to-tryout> && ddev restart
```

An update replaces every file that carries a `#ddev-generated` line. If you
remove that line from a file, the file is yours and updates leave it alone.
Updates never touch your `config.yaml`, your `composer.json`, your patch list or
your worktrees.

Removing the add-on deletes:

- its files (the overlay and `additional.php` only if you did not change them),
- the lines it added to `.git/info/exclude`,
- the markers of served sites and the generated web server files,
- the extra database servers and their data.

It keeps `packages/`, the worktrees, the TYPO3 instance folders, a patch list you
filled in, and your project's own database.

## Install from a branch, a commit or a local folder

`ddev add-on get` accepts a GitHub repository, a local folder or a tarball URL.
With `--version` you can pick a tag, a branch or a commit. The program files are
part of the repository (in `tryout/bin/`), so every branch and commit installs
without a build step:

```bash
ddev add-on get bmack/tryout --version main      # a branch
ddev add-on get bmack/tryout --version v1.2.0    # a tag
ddev add-on get bmack/tryout --version b50ac77   # a commit
ddev add-on get ~/src/tryout                          # a local folder, as it is
ddev add-on list --installed                          # show what is installed
```

Working on tryout itself? See [development](/reference/development). It explains
how to rebuild the program and test a local build with `TRYOUT_BIN`.

## Move from the old template layout

tryout used to be a template repository. You cloned it, ran
`rm -rf .git && git init`, and the files became your project. To move such a
project to the add-on:

```bash
cd your-existing-tryout
rm -rf .ddev/scripts .ddev/templates .ddev/commands/host/cs \
       .ddev/config.patches.yaml .ddev/commands/host/tryout
ddev add-on get bmack/tryout
ddev restart
```

Two things change:

- `ddev cs` is now `ddev tryout cs`.
- Composer uses the overlay `composer.tryout.json`. Delete the old generated
  `composer.json` and `composer.lock`, unless you added your own dependencies.
  A `composer.json` you keep is merged into the overlay.

Your TYPO3 Core checkout, your database and your patches stay as they are.
