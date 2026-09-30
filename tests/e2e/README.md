# Browser tests

Opt-in. The bats suites remain the default gate; nothing here runs unless you ask.

These answer questions `curl` cannot, for **every served worktree URL**:

- **The frontend renders a real TYPO3 page** — where the add-on could provision one
  (the styleguide demo, TYPO3 13.4+). A 200 is not enough; the test checks the body
  carries TYPO3's `generator` meta tag, so it is a rendered page and not a 404 or an
  install screen. On a version without the styleguide generator (12.4) there is no
  frontend and that check skips.
- **The backend logs in.** The login posts a form, sets a secure session cookie and
  redirects into a module — invisible to `curl`. This suite caught exactly such a
  bug: a served site whose vhost never told PHP the request was TLS dropped to
  `http://`, so the cookie was never returned and login failed with "Please
  activate Cookies".

### Version caveats

Both checks pass on TYPO3 13.4 and main. TYPO3 12.4 is best-effort: its styleguide
has no CLI frontend generator, and a *served secondary* 12.4 site routes its backend
login differently (`POST /typo3/login` 404s through the served vhost), so the
frontend check and — for a served secondary only — the backend check skip there. A
12.4 instance as the primary logs in normally.

## Running them

```bash
cd tests/e2e
npm install
npx playwright install chromium        # ~150 MB, once

TRYOUT_PROJECT=~/path/to/your-tryout-project npm test
```

`TRYOUT_PROJECT` must point at a provisioned DDEV project with at least one served
worktree. Without it every test skips with a message rather than failing.

The sites under test are discovered from `ddev tryout worktree list`, so serving
another worktree is covered without editing anything here.

## Why it is not in the default suite

The add-on itself needs no Node tooling (the docs site in `docs/` has its own);
`node_modules` and a browser download are a large dependency for one class of check. Keeping this
directory self-contained means someone who never runs it pays nothing.

## Project mode

`project.spec.ts` covers tryout on a project of your own: the fixture app from
`tests/fixture-app.sh`, two worktrees on different branches, each served. It
checks every served worktree URL answers from its own checkout (its branch's
`VERSION`), reached its own database and had `composer install` run.

```bash
mkdir ~/tmp/tryout-app && cd ~/tmp/tryout-app
ddev config --project-type=php --docroot=public
bash <tryout checkout>/tests/fixture-app.sh
ddev add-on get <tryout checkout> && ddev start -y
ddev tryout worktree add one feature --serve
ddev tryout worktree add two main --serve --db sqlite

cd <tryout checkout>/tests/e2e
TRYOUT_APP_PROJECT=~/tmp/tryout-app npx playwright test project.spec.ts
```
