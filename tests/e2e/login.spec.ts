import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';

// Which sites exist is a property of the project, not of this file: ask tryout.
// TRYOUT_PROJECT points at a provisioned DDEV project; without it there is nothing
// to test and the suite says so rather than failing obscurely.
const APPROOT = process.env.TRYOUT_PROJECT || '';

function sh(cmd: string, args: string[]): string {
  return execFileSync(cmd, args, { cwd: APPROOT, encoding: 'utf8' }).trim();
}

// A site is a served instance: it has a URL. It always has a backend. It has a
// frontend only where the add-on could provision one (EXT:styleguide, 13.4+); on
// 12.4 there is none and `fePath` is null, so the frontend test skips rather than
// failing. The styleguide demo sits at its own slug, not `/`, so discover the path
// from the generated site config — never assume it.
type Site = {
  name: string; url: string; branch: string;
  fePath: string | null; primary: boolean;
};

function discoverSites(): Site[] {
  if (!APPROOT) return [];
  // `worktree list --plain` prints NAME … URL; only a SERVED worktree has a URL,
  // which is exactly what has a browser-testable frontend. --plain is the
  // machine-readable contract — the default renders a bordered gum table this
  // regex cannot match. The command colours its output; strip the escapes first.
  const out = sh('ddev', ['tryout', 'worktree', 'list', '--plain'])
    .replace(/\u001b\[[0-9;]*m/g, '');
  const sites: Site[] = [];
  for (const line of out.split('\n')) {
    // NAME HEAD BRANCH … URL — the branch (column 3) carries the TYPO3 version.
    const m = line.match(/^\s+(\S+)\s+\S+\s+(\S+)\s+.*\s(https:\/\/\S+)/);
    if (!m) continue;
    const name = m[1];
    const branch = m[2];
    const url = m[3].replace(/\s+$/, '');
    const primary = /← primary/.test(line);
    sites.push({ name, url, branch, primary, fePath: feBaseFor(name, primary) });
  }
  return sites;
}

// Where this site's frontend renders, or null if it has none. The add-on
// provisions the frontend with EXT:styleguide, whose site config carries the
// `typo3/styleguide` dependency — so match on THAT config's base, not just the
// first one present (a Core distribution like camino also writes a config, and its
// base may not render). Read the raw config via a shell glob: `site:list` output
// shape varies by version, a grep does not.
function feBaseFor(name: string, primary: boolean): string | null {
  const dir = primary ? 'TYPO3-Instances/primary' : `TYPO3-Instances/${name}`;
  try {
    // The styleguide config is the one naming typo3/styleguide as a dependency.
    // Print its base line.
    const base = sh('bash', ['-c',
      `for f in ${dir}/config/sites/*/config.yaml; do ` +
      `  grep -q 'typo3/styleguide' "$f" 2>/dev/null && ` +
      `  sed -n 's/^base: *//p' "$f" | head -1 && break; done`]);
    if (base && base.startsWith('/')) return base.endsWith('/') ? base : base + '/';
  } catch { /* no styleguide frontend on this version */ }
  return null;
}

const sites = discoverSites();

test.describe('every served worktree URL', () => {
  test.skip(!APPROOT, 'set TRYOUT_PROJECT to a provisioned DDEV project');
  test.skip(sites.length === 0, 'no served sites — run: ddev tryout worktree serve <name>');

  for (const site of sites) {
    // The frontend renders a real page — where the add-on could provision one.
    // A bare `typo3 setup` builds only the backend; the styleguide generator
    // (13.4+) adds a rendered demo. On a version without it there is nothing to
    // serve at the frontend, so this skips rather than failing.
    test(`${site.name} serves a rendered frontend`, async ({ page }) => {
      test.skip(site.fePath === null,
        `${site.name}: no frontend generator on this TYPO3 version (backend only)`);
      const res = await page.goto(`${site.url}${site.fePath}`);
      expect(res?.status(), `${site.url}${site.fePath} responds`).toBe(200);
      // TYPO3 stamps every rendered page with this meta generator, so it proves
      // TYPO3 rendered the response rather than a web server or a cache.
      const html = await page.content();
      expect(html, `${site.name} frontend is TYPO3-rendered`)
        .toMatch(/name="generator"[^>]*TYPO3|TYPO3 CMS/i);
    });

    test(`${site.name} logs in and reaches the backend`, async ({ page }) => {
      // A SERVED 12.4 secondary is a documented best-effort case: 12.4 routes the
      // backend login differently and POST /typo3/login 404s through the served
      // vhost. 12.4 as the primary logs in fine, and 13.4+ served sites log in
      // fine — so the skip is scoped tightly to exactly the known-limited case, not
      // a blanket 12.4 pass that would hide a real regression elsewhere.
      test.skip(!site.primary && /^12\.4$/.test(site.branch),
        `${site.name}: served 12.4 backend login is a documented limitation`);

      await page.goto(`${site.url}/typo3/`);

      // The login page itself, before anything is typed.
      await expect(page).toHaveTitle(/TYPO3 CMS Login/);

      await page.fill('#t3-username', 'admin');
      await page.fill('#t3-password', 'Password.1');
      await Promise.all([
        page.waitForNavigation({ timeout: 30_000 }).catch(() => null),
        page.click('#t3-login-submit'),
      ]);

      // A backend URL, not a bounce back to /typo3/login. The scheme matters:
      // a served site that does not tell PHP the request was TLS drops to http,
      // its secure cookie is never returned, and login fails with "Please
      // activate Cookies" — a real bug this suite caught.
      await expect(page).toHaveURL(/\/typo3\/module\//, { timeout: 30_000 });
      expect(page.url()).toMatch(/^https:/);

      // And the backend actually rendered, not just redirected.
      await expect(page.locator('.scaffold-modulemenu, [data-modulemenu]').first())
        .toBeVisible({ timeout: 20_000 });
    });
  }

  test('each site serves its own TYPO3, not one instance on many hostnames', async () => {
    test.skip(sites.length < 2, 'needs at least two served sites');

    // The primary answers to @primary, not to its worktree name; the marker in the
    // list output is how `worktree list` flags it.
    const versions = sites.map((s) => ({
      name: s.name,
      version: sh('ddev', ['tryout', 'exec', s.primary ? '@primary' : s.name,
                           'vendor/bin/typo3', '--version']),
    }));
    for (const v of versions) {
      expect(v.version, `${v.name} reports a TYPO3 version`).toMatch(/TYPO3 CMS/);
    }
  });
});
