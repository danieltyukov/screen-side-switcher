import { test } from '@playwright/test';
import { fileURLToPath } from 'node:url';

/*
 * Writes the screenshots the README and the site use. Run with
 * SCREENSHOTS=1 npx playwright test e2e/screenshots.spec.ts (in app/).
 */
const out = (name: string) => fileURLToPath(new URL(`../../docs/img/${name}`, import.meta.url));

test.use({ deviceScaleFactor: 2 });

for (const theme of ['light', 'dark'] as const) {
  test(`arrange, ${theme}`, async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto(`/?screens=3&theme=${theme}`);
    await page.getByRole('radiogroup', { name: 'Screen to move' }).getByText('DELL U2723QE').click();
    await page.getByRole('group', { name: 'Side' }).getByRole('button', { name: 'Left' }).waitFor();
    await page.screenshot({ path: out(`app-${theme}.png`) });
  });
}

test('layouts', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/?screens=2&theme=light');
  await page.getByRole('tab', { name: 'Layouts' }).click();
  for (const [name, auto] of [['Office', true], ['Home desk', false]] as const) {
    await page.getByLabel('Name').fill(name);
    if (auto) await page.getByLabel(/Apply automatically when these screens connect/).check();
    await page.getByRole('button', { name: 'Save' }).click();
    await page.getByRole('status').getByText(`Saved ${name}`).waitFor();
  }
  await page.getByRole('status').evaluate((el) => (el.textContent = ''));
  await page.screenshot({ path: out('app-layouts.png') });
});
