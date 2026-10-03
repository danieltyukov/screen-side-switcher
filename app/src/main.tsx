import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import type { Backend } from './backend/types';
import './base.css';

// Under Tauri the window talks to the shell; in a browser tab it gets the
// mock. Neither import reaches the other's bundle path at runtime.
async function pickBackend(): Promise<Backend> {
  const { resolveBackend } = await import('./backend/resolve');
  return resolveBackend();
}

void pickBackend().then((backend) => {
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <App backend={backend} />
    </StrictMode>,
  );
});
