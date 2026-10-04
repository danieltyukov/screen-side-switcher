import { expect, test } from '@playwright/test';

test('moving the only external screen lights the matching edge', async ({ page }) => {
  await page.goto('/?screens=2');
  const sides = page.getByRole('group', { name: 'Side' });
  await sides.getByRole('button', { name: 'Right' }).click();
  await expect(sides.getByRole('button', { name: 'Right' })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByText(/leaves the right edge of the built-in screen/)).toBeVisible();
  await expect(page.locator('[data-edge="HDMI-1"]')).toHaveCount(1);
});

test('one chosen screen of three moves on its own', async ({ page }) => {
  await page.goto('/?screens=3');
  await page.getByRole('radiogroup', { name: 'Screen to move' }).getByText('LG HDR 4K').click();
  await page.getByRole('group', { name: 'Side' }).getByRole('button', { name: 'Left' }).click();
  const dell = page.getByRole('button', { name: /DELL U2723QE, left of/ });
  const lg = page.getByRole('button', { name: /LG HDR 4K, left of/ });
  await expect(lg).toBeVisible();
  const [dellBox, lgBox] = [await dell.boundingBox(), await lg.boundingBox()];
  expect(lgBox!.x).toBeLessThan(dellBox!.x);
});

test('a saved layout is listed with Apply and Auto', async ({ page }) => {
  await page.goto('/?screens=2');
  await page.getByRole('tab', { name: 'Layouts' }).click();
  await page.getByLabel('Name').fill('office');
  await page.getByLabel(/Apply automatically when these screens connect/).check();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('status')).toHaveText('Saved office');
  const item = page.getByRole('listitem');
  await expect(item.getByText('In use')).toBeVisible();
  await expect(item.getByRole('switch', { name: 'Apply automatically' })).toHaveAttribute('aria-checked', 'true');
});

test('wlroots has no primary screen to set', async ({ page }) => {
  await page.goto('/?screens=2&backend=wlroots');
  await expect(page.getByRole('group', { name: 'Side' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Make primary' })).toHaveCount(0);
});

test('without a backend the problem and diagnostics are offered', async ({ page }) => {
  await page.goto('/?error=This%20is%20a%20Wayland%20session%20without%20wlr-randr.');
  await expect(page.getByRole('alert')).toContainText('without wlr-randr');
  await expect(page.getByRole('button', { name: 'Copy diagnostics' })).toBeVisible();
});

test('dark theme is dark', async ({ page }) => {
  await page.goto('/?screens=2&theme=dark');
  const background = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  expect(background).not.toBe('rgb(255, 255, 255)');
  expect(background).toBe('rgb(14, 23, 38)');
});

test('tabs work from the keyboard', async ({ page }) => {
  await page.goto('/?screens=2');
  await page.getByRole('tab', { name: 'Arrange' }).focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('tab', { name: 'Layouts' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('tab', { name: 'Layouts' })).toBeFocused();
  await page.keyboard.press('End');
  await expect(page.getByRole('tab', { name: 'Settings' })).toHaveAttribute('aria-selected', 'true');
});

test('no console errors', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/?screens=3');
  await page.getByRole('tab', { name: 'Settings' }).click();
  await page.getByRole('tab', { name: 'Layouts' }).click();
  expect(errors).toEqual([]);
});
