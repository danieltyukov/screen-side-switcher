import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

/*
 * One static page. It shares app/src/tokens.css (imported by style.css) and
 * the screenshots in docs/img with the app, which live above this root.
 */
const repoRoot = fileURLToPath(new URL('..', import.meta.url));

export default defineConfig({
  // Relative, so the page works under /screen-side-switcher/ on Pages and
  // anywhere else it is served from.
  base: './',
  build: { outDir: 'dist', emptyOutDir: true, target: 'es2020', assetsInlineLimit: 0 },
  server: { port: 5174, strictPort: true, fs: { allow: [repoRoot] } },
  preview: { port: 4174, strictPort: true },
});
