import { defineConfig, devices } from '@playwright/test';

/*
 * Runs against `vite preview` of the built app, so it covers the bundle that
 * ships, with the browser mock behind it. SCREENSHOTS=1 also writes the
 * images in docs/img that the README and the site show.
 */
export default defineConfig({
  testDir: 'e2e',
  testIgnore: process.env.SCREENSHOTS ? [] : ['**/screenshots.spec.ts'],
  fullyParallel: true,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: 'http://localhost:4173',
    viewport: { width: 560, height: 700 },
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 560, height: 700 } } }],
  webServer: {
    command: 'npm run build && npx vite preview --port 4173 --strictPort',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
