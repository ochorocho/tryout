# tryout for every DDEV project type

## Goal

tryout today is built for one thing: a **TYPO3 Core** checkout (the project root
is a Core clone, instances overlay its sysexts, patches come from TYPO3's Gerrit,
sites are set up with `typo3 setup`). Most of what it does is not TYPO3-specific
at all: git worktrees, a site per worktree on its own URL, PHP version and
database server, the terminal UI, jobs, `launch`, `exec`.

The goal is **project mode**: tryout on *your own* DDEV project, whatever its
`type:` — every worktree of the project's repository served side by side with
its own URL, PHP and database, driven from the TUI. TYPO3 Core contribution
stays, as one mode among others (**core mode**), unchanged.

## What is TYPO3-specific today (the seams)

| Area | Core mode today | Project mode needs |
|---|---|---|
| What the root is | a TYPO3 Core clone, cloned by install | the user's own repository, never cloned |
| An instance | `TYPO3-Instances/<n>/` + overlay onto a worktree's sysexts | the worktree itself: its own `composer.json`, its own docroot |
| Composer | generated overlay (`composer.tryout.json`) | the worktree's own `composer install` |
| Setup | `typo3 setup` + styleguide frontend | per type: none, a copy of the primary's database, or the framework's installer |
| DB wiring | `additional.php` reads the site's dbname | per type: env vars (dotenv frameworks) or a generated settings file |
| Patches | Gerrit changes | later: pull/merge requests into a worktree |
| Contribution | `cs` (Gerrit hooks, template) | not applicable |
| Install | clones Core, forces docroot `TYPO3-Instances/primary/public` | leaves the project as it is |

Generic already: worktree add/list/remove/rename (detached, relative paths),
served sites (vhost, FPM per PHP, hostname set, restart gate), database servers
per type and version, `exec`, `launch`, status, the TUI, jobs and claims.

## How DDEV wires a database per type (the adapter contract)

DDEV writes one settings file for the project's docroot, with host `db` and
database `db` hard-coded. A served worktree needs the same with **its own
database name** (and, for some types, its own base URL). Three families:

1. **Env-driven** — `laravel`, `symfony`, `craftcms`, `shopware6`, `cakephp`,
   `codeigniter`, `php`, `generic`: real environment variables win over `.env`,
   so the vhost's per-site variables (as `TYPO3_DB_DBNAME` today) and `exec`'s
   environment are the whole wiring. No file is written.
2. **Settings file** — `drupal*`, `backdrop`, `wordpress`, `wp-bedrock`,
   `joomla`, `modx`, `typo3` (a site project, not Core), `silverstripe`: tryout
   generates the per-site equivalent of DDEV's file in the worktree
   (`settings.ddev.php`, `wp-config-ddev.php`, `additional.php` …), reading the
   site's database from the environment; DDEV's own file stays for the primary.
3. **Database-bound URLs** — `wordpress`, `shopware6`, `magento2`: the site URL
   lives in the database, so a copied database needs its URLs rewritten
   (`wp search-replace`, `sales-channel:update:domain`, `setup:store-config:set`).

Adapters are a Rust trait (`core::kind::ProjectKind`): `docroot(worktree)`,
`db_env(site)`, `write_settings(site)`, `after_db_copy(site)`, `install(site)`,
`support()` (full / env-only / serve-only). Unknown types fall back to the
generic adapter: served, with env wiring, and a note that the framework may need
its own configuration.

## Stage 1: Seams, no behaviour change
**Goal**: The TYPO3-specific parts sit behind a `Mode` (core / project) and a
`ProjectKind` trait with one implementation, `typo3-core`; everything else calls
through them.
**Success Criteria**: Every existing suite passes unchanged; no user-visible
change; `mode` is detected (root repository is TYPO3 Core → core) and shown by
`status`.
**Tests**: Unit: mode detection (Core origin, TYPO3-Instances layout, other
repo, no repo); the trait's `typo3-core` answers equal today's constants
(golden generator tests unchanged). Bats: existing unit/install/lifecycle suites
green. E2E: existing browser suite green.
**Status**: Complete — every suite green in CI (run 36663002091), lifecycle included
**Done so far**: `core::kind` with `Mode` (detected from the root: no repo or
Core's sysexts or a TYPO3 origin → core; anything else → project) and the
`ProjectKind` trait, `Typo3Core` its one implementation. Routed through it:
clone source and review remote, base branches (worktree bases, detached-base
detection), git excludes, PHP constraint, setup and rebuild commands (one
`run_rebuild_commands` for primary and sites, post-start included), `launch
--backend`, and a verb gate in the host dispatch. `status` names the mode;
`DdevEnv` reads `DDEV_PROJECT_TYPE` and `DDEV_DOCROOT`. Left for Stage 2, where
project mode needs them: the Composer overlay and the `TYPO3-Instances`
layout, Gerrit/`cs` constants (gated as whole verbs), and the TUI's menu.

## Stage 2: Project mode for plain PHP projects (`php`, `generic`)
**Goal**: On a project that is not TYPO3 Core, tryout installs without cloning
anything and serves worktrees of the project's own repository: `worktree
add/list/serve/unserve/remove/rename`, `exec`, `launch`, `status`, the TUI.
**Success Criteria**: `ddev add-on get` into an existing `php` project leaves it
as it was (no clone, no docroot change, no TYPO3 env); a worktree is served at
`<name>.<project>.ddev.site` from its own docroot after its own `composer
install`, on its own PHP and database (all types and versions from the database
picker); core-only verbs (`patch`, `cs`, `checkout`, `download`) say so and
point at what to use instead.
**Tests**: Unit: docroot resolution per worktree, vhost for a project-mode site
(golden), install actions skipped in project mode. Bats: a new `project.bats`
with a tiny fixture app built in the test (`composer.json` + `public/index.php`
printing its worktree and `getenv` database name) — install, serve two
worktrees, each answers with its own name and database, unserve, remove. E2E:
Playwright opens both URLs and reads the marker text.
**Status**: Complete locally (unit, unit.bats, install suite, project.bats green); CI pending
**Done so far**: install asks the binary (`__mode`, a shell fallback without
one) and records `.ddev/tryout/.mode`; every Core action (clone, patch list,
instance dirs, overlay, `additional.php`) is skipped in project mode, the
TYPO3/Composer environment moved to `config.tryout-core.yaml`, written in core
mode only; excludes per kind (a project's `.ddev/` stays visible). Post-start
in project mode only notes PHP versions and keeps excludes/relative paths. A
project site is its worktree; its marker and SQLite file wait in
`.ddev/tryout-sites/<name>/`; serve = database + `composer install` (through
`exec`, so scripts see the site's database) + vhost with `TRYOUT_DB_*`
(driver, name, host, port, user, password), which `exec` sets too. Unserve
keeps the worktree (a `.kept` note holds the server); `delete` empties a
site's database and refuses the project's own. `worktree add` starts from
origin's branch or a local one. `status`, verbs (`worktree use` included) and
TUI menus follow the kind. Tests: unit (vhost, env, unserve, base, menus,
install excludes in sync), unit.bats running the install/removal actions in
both modes, `tests/project.bats` (suite `project` in CI), `project.spec.ts`.

## Stage 3: Databases for project sites
**Goal**: A served site gets a database that makes sense for an app:
`--db-from primary` (default: a copy of the primary's database into the site's,
across types where DDEV's tools allow — same type first), `--db-empty`, and
`delete` as "reset to a fresh copy". Env-driven types wired through the vhost
and `exec` environment (`DB_DATABASE`, `DATABASE_URL`, `DB_HOST`, `DB_PORT`,
`DB_CONNECTION`/driver per server).
**Success Criteria**: A Laravel and a Symfony project serve two worktrees, each
reading and writing its own database; a copy starts with the primary's data.
**Tests**: Unit: env per type and server (MariaDB/MySQL/Postgres/SQLite →
`DATABASE_URL` forms, Laravel `DB_*`); copy command lines per engine pair.
Bats lifecycle (tag `project,db`): Laravel quickstart (`composer create-project
laravel/laravel`, `artisan migrate`), write a row in the primary, serve a
worktree with `--db-from primary`, read it back through `ddev tryout exec`; the
site's own write does not reach the primary. E2E: the Laravel welcome page per
site.
**Status**: Not Started

## Stage 4: Settings-file and URL-bound types
**Goal**: Adapters for `drupal*`/`backdrop` (per-site `settings.ddev.php`),
`wordpress`/`wp-bedrock` (per-site `wp-config-ddev.php`, `wp search-replace`
after a copy), `typo3` site projects (per-site `additional.php`), `craftcms`,
`shopware6` (domain update), `magento2` (base URL), `silverstripe`, `joomla`,
`modx`, `cakephp`, `codeigniter` — each at the support level it can honestly
reach, recorded in a support matrix shown by `status` and the README.
**Success Criteria**: Drupal, WordPress and a TYPO3 site project each serve two
worktrees with their own databases, logging into each admin separately.
**Tests**: Unit: generated settings files per type (golden, like the vhosts);
support matrix covers every type DDEV lists. Bats lifecycle per type, sharded
and tagged (`project,drupal` …): Drupal (`drupal/recommended-project` + `drush
si`), WordPress (`wp core download` + `wp core install`), TYPO3 site. E2E: log
into each served site's admin (Drupal `/user/login`, WordPress `/wp-login.php`,
TYPO3 `/typo3/`).
**Status**: Not Started

## Stage 5: Changes into worktrees, docs, CI
**Goal**: Project mode's answer to `patch`: open a pull/merge request as a new
worktree (`worktree add --pr 123` via `gh`/`glab`, TUI picker like the Gerrit
one); README and docs restructured (project mode first, TYPO3 Core as a mode);
CI sharded by type, nightly and with `full-ci`.
**Success Criteria**: A PR of the project's GitHub repository opens as a served
worktree in one step; documentation describes both modes; CI runs every type's
lifecycle nightly.
**Tests**: Unit: PR ref resolution (fixtures, like the Gerrit parser); bats with
a local bare repository standing in for the remote; TUI snapshot of the picker.
**Status**: Not Started

## Cross-cutting rules

- Core mode stays the default for a Core checkout and is never degraded: every
  stage keeps its suites green.
- Adapters never write outside the worktree's own tree and `.ddev/`; generated
  files carry `#ddev-generated` and are excluded from `git status` through
  `.git/info/exclude`, as today.
- An unknown or new DDEV type is served generically with a stated support level,
  never refused.
- Each stage ends with `cargo fmt/clippy/test`, `bats tests/unit.bats`, the
  install suite and its own lifecycle tags green, then a commit.
