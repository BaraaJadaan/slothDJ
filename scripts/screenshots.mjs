// Renders README screenshots of the real UI with a mocked backend and made-up demo tracks.
// Also handy for UI work without compiling Rust: it writes the preview pages to .preview/.
//
//   node scripts/screenshots.mjs            (needs Edge or Chrome; set BROWSER=path to override)
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'docs');
const tmp = join(root, '.preview');
mkdirSync(out, { recursive: true });
mkdirSync(tmp, { recursive: true });

const BROWSER =
  process.env.BROWSER ||
  [
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Google/Chrome/Application/chrome.exe',
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ].find(existsSync);

// ---------- demo data ----------
const DEMO = String.raw`
const art = (a, b, shape) => 'data:image/svg+xml;utf8,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="' + a + '"/><stop offset="1" stop-color="' + b + '"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/>' + shape + '</svg>');
const SUN = '<circle cx="50" cy="54" r="19" fill="#fff" opacity=".85"/><rect y="68" width="100" height="32" fill="#000" opacity=".22"/>';
const WAVES = '<path d="M0 58 Q25 44 50 58 T100 58 V100 H0Z" fill="#fff" opacity=".22"/><path d="M0 72 Q25 58 50 72 T100 72 V100 H0Z" fill="#fff" opacity=".34"/>';
const LEAF = '<path d="M28 76 C28 40 55 24 76 24 C76 56 60 76 28 76Z" fill="#fff" opacity=".82"/><path d="M28 76 L62 40" stroke="#000" stroke-opacity=".22" stroke-width="3"/>';
const RINGS = '<circle cx="50" cy="50" r="30" fill="none" stroke="#fff" stroke-opacity=".5" stroke-width="6"/><circle cx="50" cy="50" r="11" fill="#fff" opacity=".85"/>';
const BARS = [0,1,2,3,4,5,6,7].map(i => { const h = 18 + ((i * 37) % 30) * 1.4; return '<rect x="' + (13 + i * 9.6) + '" y="' + (50 - h / 2) + '" width="6" height="' + h + '" rx="3" fill="#fff" opacity=".85"/>'; }).join('');
const ART = {
  canopy: art('#065f46', '#a3e635', LEAF),
  midnight: art('#1e3a8a', '#7c3aed', SUN),
  slow: art('#9d174d', '#f97316', WAVES),
  sunday: art('#0e7490', '#5eead4', RINGS),
  branches: art('#3f3f46', '#a1a1aa', BARS),
  night: art('#312e81', '#ec4899', WAVES),
};
const now = Date.now();
const job = (o) => Object.assign({ url: 'https://soundcloud.com/demo/' + o.id, kind: 'track', format: 'mp3_320', progress: 0, speed: null, eta: null, current: null, index: null, total: null, files: [], error: null, warning: null, source: 'app' }, o);
const FORMATS = [
  { id: 'mp3_320', label: 'MP3 · 320 kbps' }, { id: 'mp3_192', label: 'MP3 · 192 kbps' },
  { id: 'mp3_128', label: 'MP3 · 128 kbps' }, { id: 'original', label: 'Original (no re-encode)' },
];
const JOBS = [
  job({ id: 6, kind: 'set', title: 'Canopy Sessions', artist: 'Moss & Moon', artwork: ART.canopy, status: 'downloading', progress: 0.31, speed: 3.3e6, current: 'Hammock Theory', index: 4, total: 12, created: now - 1e3 }),
  job({ id: 5, title: 'Midnight Drive', artist: 'Lowtide', artwork: ART.midnight, status: 'downloading', progress: 0.64, speed: 2.4e6, eta: 6, created: now - 2e3 }),
  job({ id: 4, title: 'Slow Motion', artist: 'Cecropia', artwork: ART.slow, status: 'converting', progress: 1, created: now - 3e3 }),
  job({ id: 3, title: 'Sunday Hammock', artist: 'Moss & Moon', artwork: ART.sunday, status: 'done', progress: 1, files: ['x.mp3'], created: now - 4e3 }),
  job({ id: 2, title: 'Branches (Live)', artist: 'The Three-Toed', artwork: ART.branches, status: 'failed', error: 'This needs a SoundCloud Go+ login, so it can’t be downloaded.', created: now - 5e3 }),
  job({ id: 1, title: 'Leaf Shuffle', artist: 'Cecropia', artwork: ART.canopy, format: 'original', status: 'done', progress: 1, files: ['x.m4a'], created: now - 6e3 }),
];
`;

const APP_MOCK = String.raw`
${DEMO}
const SCENE = new URLSearchParams(location.search).get('scene') || 'main';
const setup = SCENE === 'setup';
const engine = setup
  ? { ready: false, busy: true, stage: 'Downloading yt-dlp · 13 of 22 MB', progress: 0.58, ytdlpVersion: null, ffmpeg: null, error: null }
  : { ready: true, busy: false, stage: '', progress: 0, ytdlpVersion: '2026.10.26', ffmpeg: 'managed', error: null };
const handlers = {
  get_state: () => ({
    version: '1.0.0', platform: 'windows', port: 37771, formats: FORMATS, engine,
    jobs: setup ? [] : JOBS,
    settings: { outputDir: 'C:\\Users\\you\\Music\\slothDJ', format: 'mp3_320', embedArtwork: true, maxConcurrent: 2, theme: SCENE === 'light' ? 'light' : 'dark', lastUpdateCheck: 0 },
  }),
  check_url: ({ url }) => /soundcloud\.com/.test(url) ? url : Promise.reject('slothDJ supports SoundCloud links for now.'),
  preview: () => ({ url: 'https://soundcloud.com/lowtide/night-swim', kind: 'track', title: 'Night Swim', artist: 'Lowtide', artwork: ART.night }),
  read_clipboard: () => null,
  save_settings: ({ settings }) => settings,
};
window.__TAURI__ = {
  core: { invoke: (cmd, args) => Promise.resolve().then(() => (handlers[cmd] || (() => null))(args || {})) },
  event: { listen: () => Promise.resolve(() => {}) },
  window: { getCurrentWindow: () => ({ minimize() {}, toggleMaximize() {}, close() {} }) },
};
addEventListener('load', () => setTimeout(() => {
  if (SCENE === 'settings') document.getElementById('drawer').hidden = false;
  if (SCENE === 'main' || SCENE === 'light') {
    const i = document.getElementById('url');
    i.value = 'https://soundcloud.com/lowtide/night-swim';
    i.dispatchEvent(new Event('input'));
    i.blur();
  }
}, 200));
`;

const POPUP_MOCK = String.raw`
${DEMO}
const OEMBED = { title: 'Night Swim by Lowtide', author_name: 'Lowtide', thumbnail_url: ART.night };
window.fetch = async () => ({ ok: true, json: async () => OEMBED });
const byId = Object.fromEntries(JOBS.map(j => [j.id, j]));
window.chrome = {
  runtime: {
    getURL: (p) => p,
    sendMessage: async (m) => m.type === 'status'
      ? { online: true, app: 'slothDJ', version: '1.0.0', formats: FORMATS, defaultFormat: 'mp3_320' }
      : m.type === 'job' ? { ok: true, job: byId[m.id] } : { ok: true },
  },
  tabs: { query: async () => [{ id: 1, url: 'https://soundcloud.com/lowtide/night-swim' }], create() {}, update() {} },
  storage: {
    sync: { get: async () => ({ format: '' }), set: async () => {} },
    local: { get: async () => ({ recent: [5, 6, 3].map(id => ({ id, title: byId[id].title })) }), set: async () => {} },
  },
};
`;

function page(srcHtml, baseDir, mock, name) {
  const base = pathToFileURL(baseDir + '/').href;
  // Screenshots freeze animations so they capture the final state, not a mid-transition frame.
  const still = '<style>*,*::before,*::after{animation:none!important;transition:none!important}</style>';
  const html = readFileSync(srcHtml, 'utf8').replace('<head>', `<head>\n<base href="${base}">\n${still}\n<script>${mock}</script>`);
  const file = join(tmp, `${name}.html`);
  writeFileSync(file, html);
  return pathToFileURL(file).href;
}

function shot(url, file, w, h) {
  execFileSync(BROWSER, [
    '--headless=new', '--disable-gpu', '--hide-scrollbars', '--force-device-scale-factor=2',
    `--window-size=${w},${h}`, '--virtual-time-budget=2500', '--allow-file-access-from-files',
    `--screenshot=${file}`, url,
  ], { stdio: 'ignore' });
  console.log('wrote', file.replace(root, '.'));
}

if (!BROWSER) throw new Error('Set BROWSER to a Chrome/Edge executable');
const appHtml = join(root, 'app/ui/index.html');
const appPage = page(appHtml, join(root, 'app/ui'), APP_MOCK, 'app');
for (const scene of ['main', 'light', 'settings', 'setup']) shot(`${appPage}?scene=${scene}`, join(out, `app-${scene}.png`), 1000, 700);

const popupPage = page(join(root, 'extension/popup.html'), join(root, 'extension'), POPUP_MOCK, 'popup');
shot(popupPage, join(out, 'extension-popup.png'), 360, 396);
