import { expect, test } from '@playwright/test';

const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15';
const WINDOWS = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36';

test('loads without errors and says what it is', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  await page.goto('/');
  await expect(page).toHaveTitle('Screen Side');
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  expect(errors).toEqual([]);
});

test.describe('on a Mac', () => {
  test.use({ userAgent: MAC });
  test('offers the dmg', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('link', { name: /Download for macOS/ })).toHaveAttribute('href', /ScreenSide_universal\.dmg$/);
  });
});

test.describe('on Windows', () => {
  test.use({ userAgent: WINDOWS });
  test('offers the installer', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('link', { name: /Download for Windows/ })).toHaveAttribute('href', /ScreenSide_x64-setup\.exe$/);
  });
});

test('the command line tab copies the one-liner', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/');
  await page.locator('label[for="t-cli"]').click();
  const panel = page.locator('.panel.p-cli');
  await expect(panel.getByText(/install\.sh \| sh/)).toBeVisible();
  await panel.getByRole('button', { name: 'Copy' }).first().click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toContain('install.sh | sh');
});

test('the demo moves the monitor and says where the pointer crosses', async ({ page }) => {
  await page.goto('/');
  const demo = page.getByRole('group', { name: 'Try it' });
  await demo.getByRole('button', { name: 'Above' }).click();
  await expect(page.locator('#demo-caption')).toContainText('top edge of the laptop');
  await expect(demo.getByRole('button', { name: 'Above' })).toHaveAttribute('aria-pressed', 'true');
  await demo.getByRole('button', { name: 'Right' }).click();
  await expect(page.locator('#demo-caption')).toContainText('right edge of the laptop');
});

test('the theme toggle switches and remembers', async ({ page }) => {
  await page.goto('/');
  const toggle = page.getByRole('button', { name: 'Dark theme' });
  const before = await page.evaluate(() => document.documentElement.dataset.theme ?? 'system');
  await toggle.click();
  const after = await page.evaluate(() => document.documentElement.dataset.theme);
  expect(after).not.toBe(before);
  await page.reload();
  expect(await page.evaluate(() => document.documentElement.dataset.theme)).toBe(after);
});

test('the installers and images are served', async ({ request, page }) => {
  for (const path of ['install.sh', 'install.ps1', 'og.png', 'favicon.svg']) {
    expect((await request.get(path)).status(), path).toBe(200);
  }
  await page.goto('/');
  for (const img of await page.locator('img').all()) {
    await expect(img).toHaveJSProperty('complete', true);
    expect(await img.evaluate((i: HTMLImageElement) => i.naturalWidth)).toBeGreaterThan(0);
  }
});

test('fits a phone without sideways scrolling', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  await page.goto('/');
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  expect(overflow).toBeLessThanOrEqual(0);
});
