# Introduction

tryout is an add-on for [DDEV](https://docs.ddev.com/). It lets you run several
branches of one project at the same time. Each branch gets its own URL, its own
PHP version and its own database.

This is useful when you want to:

- test a feature branch while your main branch keeps running,
- review a pull request without touching your own work,
- compare two versions of a site side by side.

You use it with `ddev tryout …` commands, or with `ddev tryout ui`, a menu-driven
screen in your terminal.

## A few words first

- **Worktree:** a second checkout of the same git repository, in its own folder.
  tryout puts them in `worktrees/<name>`. A worktree shares the repository's
  history, so it is quick to create.
- **Served site:** a worktree that has its own URL, `<name>.<project>.ddev.site`.
- **Primary:** the site at the project's main URL. It is DDEV's normal site.

## Two modes

tryout works in one of two modes. You do not pick the mode. tryout looks at the
project root and decides.

| Mode | When | What tryout does |
|---|---|---|
| **Project mode** | The project root is a git repository of your own, of any DDEV type | Serves worktrees of *your* repository and its pull requests. A new site's database starts as a copy of the primary's. The install changes nothing in your project. |
| **Core mode** | The project root is a TYPO3 Core checkout, or it is empty | Clones TYPO3 Core into the project, builds a TYPO3 site from it, applies patches from Gerrit (TYPO3's code review) and prepares you to contribute to TYPO3 Core. |

This is how tryout decides:

1. There is no `.git` in the project root → **core mode**. The install will clone TYPO3 Core.
2. `typo3/sysext/core/composer.json` exists → **core mode**.
3. `origin` points to TYPO3's repository (`typo3/typo3` or `Packages/TYPO3.CMS`) → **core mode**.
4. Anything else → **project mode**.

To see which mode your project uses, run `ddev tryout status`:

```text
  Mode:      project (laravel)
  Sites:     ✓ database and URL from the environment
             DB_*, APP_URL
```

## What works the same in both modes

- **Worktrees.** `ddev tryout worktree add <name> <branch>` creates
  `worktrees/<name>` at that branch.
- **Served sites.** `ddev tryout worktree serve <name>` gives a worktree its own
  URL, its own PHP version and its own database.
- **Other tools.** `exec`, `launch`, `status`, the terminal UI, tab completion
  and the guided prompts.

## What is different

| | Project mode | Core mode |
|---|---|---|
| The project root | your repository, never cloned | the TYPO3 Core clone |
| Where a site's app lives | in the worktree itself, with its own `composer install` | in `TYPO3-Instances/<name>/`, built from the worktree's code |
| A new site's database | a copy of the primary's (`--db-from`, `--db-empty`) | a fresh TYPO3 setup |
| How the app finds its database | environment variables or a settings file, depending on the [framework](/frameworks/) | `additional.php` |
| Changes to try out | pull and merge requests (`--pr`) | Gerrit changes (`patch`) |
| TYPO3 Core commands | `download`, `checkout`, `patch`, `reset`, `composer`, `cs` and `worktree use` are not available | all available |

## Next steps

1. [Install tryout](/guide/install).
2. Read [project mode](/guide/project-mode) or [TYPO3 Core mode](/guide/typo3-core),
   whichever fits your project.
