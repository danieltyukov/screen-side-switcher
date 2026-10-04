import './demo.js';

/*
 * Theme toggle, the download button for the visitor's system, and copy
 * buttons. The head script has already applied a stored theme.
 */

const root = document.documentElement;
const media = window.matchMedia('(prefers-color-scheme: dark)');
const toggle = document.getElementById('theme');

const isDark = () => {
  const choice = root.dataset.theme;
  return choice === 'dark' || (choice === undefined && media.matches);
};
const syncToggle = () => toggle.setAttribute('aria-pressed', String(isDark()));

toggle.addEventListener('click', () => {
  const next = isDark() ? 'light' : 'dark';
  root.dataset.theme = next;
  try {
    localStorage.setItem('screen-side-theme', next);
  } catch {
    // Storage refused; the choice holds for this page view.
  }
  syncToggle();
});
media.addEventListener('change', syncToggle);
syncToggle();

// The user agent string, because a browser's own platform field ignores
// overrides and some browsers no longer fill it in.
const LATEST = 'https://github.com/danieltyukov/screen-side-switcher/releases/latest/download/';
const ua = navigator.userAgent;
const download = document.getElementById('download');
const offer = /Windows/.test(ua)
  ? { os: 'Windows', file: 'ScreenSide_x64-setup.exe', tab: 't-windows' }
  : /Macintosh|Mac OS X/.test(ua)
    ? { os: 'macOS', file: 'ScreenSide_universal.dmg', tab: 't-macos' }
    : /Linux|X11|CrOS/.test(ua)
      ? { os: 'Linux', file: 'screen-side_x86_64.AppImage', tab: 't-linux' }
      : null;
if (offer) {
  download.textContent = `Download for ${offer.os}`;
  download.href = LATEST + offer.file;
  document.getElementById(offer.tab).checked = true;
}

const status = document.getElementById('copy-status');
for (const button of document.querySelectorAll('.copy')) {
  button.addEventListener('click', async () => {
    const text = button.parentElement.querySelector('code').textContent;
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = 'Copied';
      status.textContent = 'Copied to the clipboard.';
      setTimeout(() => (button.textContent = 'Copy'), 1600);
    } catch {
      status.textContent = 'Copying is blocked here. Select the command and copy it.';
    }
  });
}
