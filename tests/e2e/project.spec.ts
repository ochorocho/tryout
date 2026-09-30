import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';

// Project mode: a project of the user's own (tests/fixture-app.sh), each served
// worktree answering at its own URL from its own checkout and database.
// TRYOUT_APP_PROJECT points at such a project; without it the suite skips.
const APPROOT = process.env.TRYOUT_APP_PROJECT || '';

type Site = { name: string; url: string };

// `worktree list --plain` (NAME HEAD BRANCH STATE PHP DB URL): the served
// worktrees are the rows with a URL; the primary is DDEV's own site.
function servedSites(): Site[] {
  if (!APPROOT) return [];
  const out = execFileSync('ddev', ['tryout', 'worktree', 'list', '--plain'], {
    cwd: APPROOT, encoding: 'utf8',
  }).replace(/\u001b\[[0-9;]*m/g, '');
  const sites: Site[] = [];
  for (const line of out.split('\n')) {
    const m = line.match(/^\s+(\S+)\s.*\s(https:\/\/\S+)/);
    if (m && !/← primary/.test(line)) sites.push({ name: m[1], url: m[2] });
  }
  return sites;
}

const sites = servedSites();

test.describe('a project of your own, served per worktree', () => {
  test.skip(!APPROOT, 'set TRYOUT_APP_PROJECT to a provisioned project-mode DDEV project');
  test.skip(sites.length < 2, 'needs two served worktrees — see tests/e2e/README.md');

  for (const site of sites) {
    test(`${site.name} answers from its own worktree and database`, async ({ page }) => {
      const response = await page.goto(site.url);
      expect(response?.status()).toBe(200);
      const body = page.locator('body');
      await expect(body).toContainText(`fixture site=${site.name} `);
      await expect(body).toContainText(' connected ');
      await expect(body).toContainText('vendor=installed');
    });
  }

  test('each site runs its own branch', async ({ page }) => {
    const versions = new Set<string>();
    for (const site of sites) {
      await page.goto(site.url);
      const text = await page.locator('body').innerText();
      versions.add(text.match(/version=(\S+)/)?.[1] ?? '');
    }
    expect(versions.size).toBe(sites.length);
  });
});
