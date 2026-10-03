import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  test: {
    include: ['app/src/**/*.test.{ts,tsx}'],
    environment: 'jsdom',
    setupFiles: ['app/vitest.setup.ts'],
  },
});
