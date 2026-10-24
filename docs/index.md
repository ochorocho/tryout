---
layout: home

hero:
  name: tryout
  text: Every branch, served side by side.
  tagline: A DDEV add-on. Each git worktree of your project gets its own URL, its own PHP version and its own database — for your own project, or for TYPO3 Core.
  image:
    src: /logo.svg
    alt: A happy lab flask with git branches, each ending in a small website
  actions:
    - theme: brand
      text: Get started
      link: /guide/introduction
    - theme: alt
      text: Supported frameworks
      link: /frameworks/
    - theme: alt
      text: Commands
      link: /reference/commands

features:
  - title: Your own project
    details: Laravel, Symfony, Drupal, WordPress, a TYPO3 site, plain PHP — any DDEV project. Installing tryout changes nothing in it.
    link: /guide/project-mode
  - title: One site per branch
    details: "ddev tryout worktree add feature-x feature/x --serve — and feature-x.&lt;project&gt;.ddev.site is up."
    link: /guide/sites
  - title: A database for each site
    details: A new site starts with a copy of the primary's database. MariaDB, MySQL, PostgreSQL or SQLite, in the version you pick.
    link: /guide/sites
  - title: Pull requests
    details: "ddev tryout worktree add --pr 42 opens a GitHub or GitLab pull request as its own site."
    link: /guide/pull-requests
  - title: TYPO3 Core
    details: A TYPO3 Core checkout in minutes, with Gerrit patches one command away and everything set up to contribute.
    link: /guide/typo3-core
  - title: A terminal UI
    details: "ddev tryout ui shows every worktree and runs the commands for you, side by side."
    link: /guide/terminal-ui
---
