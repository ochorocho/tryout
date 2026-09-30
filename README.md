<p align="center">
  <img src="docs/public/logo.svg" width="140" alt="tryout: a happy lab flask with git branches, each ending in a small website">
</p>

<h1 align="center">tryout</h1>

<p align="center">
  Every branch of your DDEV project, served side by side.<br>
  <a href="https://bmack.github.io/tryout/"><b>Documentation</b></a>
</p>

<p align="center">
  <a href="https://addons.ddev.com"><img src="https://img.shields.io/badge/DDEV-Add--on_Registry-blue" alt="add-on registry"></a>
  <a href="https://github.com/bmack/tryout/actions/workflows/tests.yml?query=branch%3Amain"><img src="https://github.com/bmack/tryout/actions/workflows/tests.yml/badge.svg?branch=main" alt="tests"></a>
  <a href="https://github.com/bmack/tryout/releases/latest"><img src="https://img.shields.io/github/v/release/bmack/tryout" alt="release"></a>
</p>

tryout is a [DDEV](https://ddev.com) add-on. It gives every git worktree of your
project its own site: its own URL, its own PHP version and its own database. You
can try a branch, a pull request or a fix next to your main site, without
switching branches back and forth.

It works in two modes. tryout picks the mode from what is in your project:

- **Your own project** — any DDEV project in a git repository: Laravel,
  Symfony, Drupal, WordPress, a TYPO3 site, plain PHP and more. Installing
  tryout changes nothing in your project.
- **TYPO3 Core** — a TYPO3 Core checkout with a running TYPO3 in a few minutes,
  Gerrit patches one command away, and everything set up to contribute.

## Quick start: your own project

In a DDEV project whose root is a git repository:

```bash
ddev add-on get bmack/tryout
ddev restart

# A worktree of branch feature/x, served at https://feature-x.<project>.ddev.site
ddev tryout worktree add feature-x feature/x --serve

# Pull request #42 of your GitHub or GitLab repository, served as pr-42
ddev tryout worktree add --pr 42

# All of it in a terminal UI
ddev tryout ui
```

A new site starts with a copy of your main site's database, so there is data to
work with right away.

## Quick start: TYPO3 Core

```bash
mkdir my-typo3-site && cd my-typo3-site
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
ddev add-on get bmack/tryout
ddev start
```

Then open `https://my-typo3-site.ddev.site/typo3/` and log in as `admin` /
`Password.1`.

## What tryout knows about your framework

tryout tells each site where its own database is, in the way its framework
expects:

| Support | Project types |
|---|---|
| From environment variables | Laravel, Symfony, Craft CMS, Shopware 6, Silverstripe, CodeIgniter, CakePHP, Asterios, WordPress with Bedrock, `php`, `generic` |
| Through a settings file | Drupal 6–12, Backdrop, WordPress, TYPO3 site projects |
| Served only, for now | Joomla, Magento, Magento 2, Maho, MODX |

The [framework pages](https://bmack.github.io/tryout/frameworks/) explain what
tryout does for each type, and what you need to do yourself.

## Documentation

Everything else — installation, served sites, databases, pull requests, Gerrit,
the terminal UI, every command and file — is in the documentation:

**https://bmack.github.io/tryout/**

## Contributing

tryout is one Rust program in `tui/`, shipped as ready-built binaries in
`tryout/bin/`. See [Development](https://bmack.github.io/tryout/reference/development)
for how to build it, run the tests and preview the documentation.

## License

MIT
