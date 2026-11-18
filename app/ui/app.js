'use strict';
// slothDJ UI. Plain JS, no build step; talks to the Rust core through Tauri's invoke/events.

const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const listen = (event, cb) => T.event.listen(event, (e) => cb(e.payload));
const appWindow = T.window.getCurrentWindow();
const REPO = 'https://github.com/BaraaJadaan/slothDJ';

// ---------- Icons (Lucide, ISC license) ----------
const ICONS = {
  link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
  download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/><path d="M12 15V3"/>',
  sliders: '<path d="M21 4h-7M10 4H3M21 12h-9M8 12H3M21 20h-5M12 20H3M14 2v4M8 10v4M16 18v4"/>',
  x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
  minus: '<path d="M5 12h14"/>',
  square: '<rect width="13" height="13" x="5.5" y="5.5" rx="1.5"/>',
  folder: '<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>',
  play: '<path d="M6 3.5v17a1 1 0 0 0 1.5.86l14-8.5a1 1 0 0 0 0-1.72l-14-8.5A1 1 0 0 0 6 3.5Z"/>',
  retry: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
  trash: '<path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  music: '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
  clipboard: '<rect width="8" height="4" x="8" y="2" rx="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
  external: '<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  puzzle: '<path d="M19.44 7.85c-.05.32.06.65.29.88l1.57 1.57c.47.47.7 1.09.7 1.7s-.23 1.23-.7 1.7l-1.61 1.61a.98.98 0 0 1-.84.28c-.47-.07-.8-.48-.97-.93a2.5 2.5 0 1 0-3.21 3.21c.45.17.86.5.93.97a.98.98 0 0 1-.28.84l-1.61 1.61a2.4 2.4 0 0 1-1.7.7 2.4 2.4 0 0 1-1.7-.7l-1.57-1.57a1.03 1.03 0 0 0-.88-.29c-.49.07-.84.5-1.02.97a2.5 2.5 0 1 1-3.24-3.24c.46-.18.9-.53.97-1.02a1.03 1.03 0 0 0-.29-.88L2.7 13.7A2.4 2.4 0 0 1 2 12c0-.62.24-1.23.7-1.7L4.23 8.77c.24-.24.58-.35.92-.3.51.08.88.53 1.07 1.01a2.5 2.5 0 1 0 3.26-3.26c-.48-.2-.93-.56-1.01-1.07-.05-.34.06-.68.3-.92L10.3 2.7A2.4 2.4 0 0 1 12 2c.62 0 1.23.24 1.7.7l1.57 1.57c.23.23.56.34.88.29.49-.07.84-.5 1.02-.97a2.5 2.5 0 1 1 3.24 3.24c-.46.18-.9.53-.97 1.02Z"/>',
};
const icon = (name) => `<svg class="i" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ''}</svg>`;
function hydrateIcons(root = document) {
  root.querySelectorAll('[data-icon]:not([data-icon-done])').forEach((el) => {
    el.insertAdjacentHTML('afterbegin', icon(el.dataset.icon));
    el.setAttribute('data-icon-done', '');
  });
}

// ---------- Helpers ----------
const $ = (sel, root = document) => root.querySelector(sel);
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
const RUNNING = ['starting', 'downloading', 'converting'];
const FINISHED = ['done', 'failed', 'canceled'];
const fmtSpeed = (b) => (!b ? '' : b >= 1048576 ? `${(b / 1048576).toFixed(1)} MB/s` : `${Math.max(1, Math.round(b / 1024))} KB/s`);
const fmtEta = (s) => {
  if (s == null || !isFinite(s) || s <= 0) return '';
  s = Math.round(s);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')} left`;
};
const prettyPath = (p) => {
  const sep = p.includes('\\') ? '\\' : '/';
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts.length > 2 ? `…${sep}${parts.slice(-2).join(sep)}` : p;
};

const state = { jobs: new Map(), settings: null, engine: {}, formats: [], filter: 'all', port: null };
const formatLabel = (id) => state.formats.find((f) => f.id === id)?.label ?? id;

// ---------- Theme ----------
const darkQuery = matchMedia('(prefers-color-scheme: dark)');
function applyTheme() {
  const t = state.settings?.theme ?? 'system';
  const dark = t === 'dark' || (t === 'system' && darkQuery.matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}
darkQuery.addEventListener('change', applyTheme);

// ---------- Toast ----------
let toastTimer;
function toast(msg, isErr = false) {
  const el = $('#toast');
  el.textContent = msg;
  el.classList.toggle('err', isErr);
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 3400);
}

// ---------- Omnibox ----------
const input = $('#url');
const form = $('#add-form');
const hint = $('#hint');
const preview = $('#preview');
let previewSeq = 0;
let previewTimer;

function setHint(text, isErr = false) {
  hint.textContent = text || '';
  hint.classList.toggle('err', isErr && !!text);
  form.classList.toggle('invalid', isErr && !!text);
}

async function onInput() {
  const value = input.value.trim();
  const seq = ++previewSeq;
  clearTimeout(previewTimer);
  preview.hidden = true;
  setHint('');
  if (!value) return;
  try {
    await invoke('check_url', { url: value });
  } catch (e) {
    if (value.length > 10 && seq === previewSeq) setHint(String(e), true);
    return;
  }
  previewTimer = setTimeout(async () => {
    try {
      const p = await invoke('preview', { url: value });
      if (seq === previewSeq) showPreview(p);
    } catch (e) {
      if (seq === previewSeq) setHint(String(e), true);
    }
  }, 220);
}

function showPreview(p) {
  const img = preview.querySelector('img');
  if (p.artwork) {
    img.src = p.artwork;
    img.style.visibility = '';
  } else {
    img.removeAttribute('src');
    img.style.visibility = 'hidden';
  }
  preview.querySelector('strong').textContent = p.title || 'Untitled';
  preview.querySelector('.preview-text span').textContent = p.artist || '';
  preview.querySelector('.kind-pill').textContent = { track: 'Track', set: 'Set', profile: 'Profile' }[p.kind] || '';
  preview.hidden = false;
}

async function submit(url) {
  url = String(url ?? input.value).trim();
  if (!url) {
    input.focus();
    setHint('Paste a SoundCloud link first.', true);
    return;
  }
  try {
    const job = await invoke('add_download', { url, format: $('#format').value });
    input.value = '';
    preview.hidden = true;
    setHint('');
    hideClip();
    if (state.filter === 'finished') setFilter('all');
    upsert(job);
  } catch (e) {
    setHint(String(e), true);
    input.focus();
  }
}

form.addEventListener('submit', (e) => {
  e.preventDefault();
  submit();
});
input.addEventListener('input', onInput);
$('#paste').addEventListener('click', async () => {
  const text = await invoke('read_clipboard').catch(() => null);
  if (!text) return toast('Your clipboard is empty.');
  input.value = text;
  input.focus();
  onInput();
});
$('#format').addEventListener('change', (e) => saveSettings({ format: e.target.value }));

// Offer a SoundCloud link that's sitting on the clipboard.
let lastClip = '';
async function checkClipboard() {
  const text = await invoke('read_clipboard').catch(() => null);
  if (!text || text === lastClip || text === input.value.trim()) return;
  let url;
  try {
    url = await invoke('check_url', { url: text });
  } catch {
    return;
  }
  lastClip = text;
  if ([...state.jobs.values()].some((j) => j.url === url)) return;
  const chip = $('#clip-chip');
  chip.querySelector('.clip-text').textContent = url.replace(/^https:\/\//, '');
  chip.dataset.url = text;
  chip.hidden = false;
}
function hideClip() {
  $('#clip-chip').hidden = true;
}
$('#clip-chip').addEventListener('click', (e) => submit(e.currentTarget.dataset.url));
window.addEventListener('focus', checkClipboard);

// ---------- Download list ----------
const list = $('#jobs');
const isVisible = (j) =>
  state.filter === 'active' ? !FINISHED.includes(j.status) : state.filter === 'finished' ? FINISHED.includes(j.status) : true;

function upsert(job) {
  state.jobs.set(job.id, job);
  renderJob(job);
  updateCounts();
}

function renderJob(job) {
  let li = list.querySelector(`[data-id="${job.id}"]`);
  if (!isVisible(job)) {
    li?.remove();
    updateEmpty();
    return;
  }
  if (!li) {
    li = createJobEl(job);
    const before = [...list.children].find((el) => (state.jobs.get(+el.dataset.id)?.created ?? 0) < job.created);
    list.insertBefore(li, before ?? null);
  }
  patchJob(li, job);
  updateEmpty();
}

function createJobEl(job) {
  const li = document.createElement('li');
  li.className = 'job';
  li.dataset.id = job.id;
  li.innerHTML = `
    <div class="art"><div class="ph">${icon('music')}</div><span class="badge"></span></div>
    <div class="job-main">
      <div class="job-title"><strong></strong><span class="kind-pill" hidden></span></div>
      <div class="job-meta"></div>
      <div class="bar"><div class="fill"></div></div>
    </div>
    <div class="job-actions"></div>`;
  return li;
}

function statusText(j) {
  const setPrefix = j.kind !== 'track' && j.total ? `Track ${j.index} of ${j.total}` : '';
  switch (j.status) {
    case 'queued':
      return state.engine.ready ? 'Waiting for a free slot…' : 'Waiting for the engine…';
    case 'starting':
      return 'Starting…';
    case 'downloading':
      if (setPrefix) return [setPrefix, esc(j.current), fmtSpeed(j.speed)].filter(Boolean).join(' · ');
      return [`${Math.round(j.progress * 100)}%`, fmtSpeed(j.speed), fmtEta(j.eta)].filter(Boolean).join(' · ');
    case 'converting': {
      const what = j.format === 'original' ? 'Finishing up…' : 'Converting to MP3…';
      return setPrefix ? `${setPrefix} · ${what}` : what;
    }
    case 'done': {
      const n = j.files.length;
      let text = j.kind === 'track' ? `Saved · ${esc(formatLabel(j.format))}` : `${n} track${n === 1 ? '' : 's'} saved`;
      if (j.warning) text += ` <span class="warn">· ${esc(j.warning)}</span>`;
      return text;
    }
    case 'failed':
      return esc(j.error || 'Download failed.');
    case 'canceled':
      return 'Canceled';
    default:
      return '';
  }
}

const btn = (action, ic, title, cls = '') =>
  `<button class="icon-btn ${cls}" data-action="${action}" title="${title}" aria-label="${title}">${icon(ic)}</button>`;

function actionsFor(j) {
  if (!FINISHED.includes(j.status)) return btn('cancel', 'x', 'Cancel', 'danger');
  if (j.status === 'done') {
    const file = j.files[0];
    return (
      (file && j.kind === 'track' ? btn('play', 'play', 'Play') : '') +
      (file ? btn('reveal', 'folder', 'Show in folder') : '') +
      btn('remove', 'trash', 'Remove from list', 'danger')
    );
  }
  return btn('retry', 'retry', 'Retry') + btn('remove', 'trash', 'Remove from list', 'danger');
}

function patchJob(li, j) {
  li.dataset.status = j.status;

  const art = li.querySelector('.art');
  if (j.artwork && art.dataset.src !== j.artwork) {
    art.dataset.src = j.artwork;
    const img = new Image();
    img.alt = '';
    img.onload = () => art.querySelector('.ph, img')?.replaceWith(img);
    img.src = j.artwork;
  }
  const badge = art.querySelector('.badge');
  const b = j.status === 'done' ? 'check' : j.status === 'failed' ? 'x' : '';
  if (badge.dataset.k !== b) {
    badge.dataset.k = b;
    badge.innerHTML = b ? icon(b) : '';
  }

  const title = li.querySelector('.job-title strong');
  title.textContent = j.title || 'Untitled';
  title.title = j.title || '';
  const pill = li.querySelector('.kind-pill');
  pill.hidden = j.kind === 'track';
  pill.textContent = j.kind === 'set' ? 'Set' : 'Profile';

  const meta = [j.artist ? esc(j.artist) : '', `<span class="job-status">${statusText(j)}</span>`].filter(Boolean);
  li.querySelector('.job-meta').innerHTML = meta.join('<span class="sep">•</span>');

  const bar = li.querySelector('.bar');
  bar.classList.toggle('indeterminate', j.status === 'starting' || (j.status === 'converting' && j.kind === 'track'));
  li.querySelector('.fill').style.width = `${(j.progress * 100).toFixed(1)}%`;

  const actions = li.querySelector('.job-actions');
  const key = `${j.status}|${j.files.length > 0}|${j.kind}`;
  if (actions.dataset.k !== key) {
    actions.dataset.k = key;
    actions.innerHTML = actionsFor(j);
  }
}

list.addEventListener('click', async (e) => {
  const b = e.target.closest('[data-action]');
  if (!b) return;
  const id = Number(b.closest('.job').dataset.id);
  const job = state.jobs.get(id);
  try {
    switch (b.dataset.action) {
      case 'cancel': await invoke('cancel_job', { id }); break;
      case 'retry': await invoke('retry_job', { id }); break;
      case 'remove': await invoke('remove_job', { id }); break;
      case 'play': await invoke('open_path', { path: job.files[0] }); break;
      case 'reveal': await invoke('reveal_path', { path: job.files[0] }); break;
    }
  } catch (err) {
    toast(String(err), true);
  }
});
list.addEventListener('dblclick', (e) => {
  const li = e.target.closest('.job');
  const job = li && state.jobs.get(Number(li.dataset.id));
  if (job?.status === 'done' && job.files[0] && !e.target.closest('button')) {
    invoke(job.kind === 'track' ? 'open_path' : 'reveal_path', { path: job.files[0] }).catch((err) => toast(String(err), true));
  }
});

function rerenderAll() {
  list.innerHTML = '';
  [...state.jobs.values()]
    .sort((a, b) => b.created - a.created)
    .filter(isVisible)
    .forEach((j) => {
      const li = createJobEl(j);
      list.appendChild(li);
      patchJob(li, j);
    });
  updateCounts();
  updateEmpty();
}

function updateCounts() {
  const all = [...state.jobs.values()];
  $('#count').textContent = all.length ? String(all.length) : '';
  $('#clear').hidden = !all.some((j) => FINISHED.includes(j.status));
  $('#filter').hidden = all.length === 0;
}

function updateEmpty() {
  const nothing = state.jobs.size === 0;
  $('#empty').hidden = !nothing;
  let note = $('#filtered-empty');
  const filteredOut = !nothing && list.children.length === 0;
  if (filteredOut && !note) {
    note = document.createElement('p');
    note.id = 'filtered-empty';
    note.className = 'hint';
    note.style.padding = '18px 4px';
    list.after(note);
  }
  if (note) {
    note.hidden = !filteredOut;
    note.textContent = state.filter === 'active' ? 'Nothing downloading right now.' : 'No finished downloads yet.';
  }
}

function setFilter(f) {
  state.filter = f;
  $('#filter').querySelectorAll('button').forEach((x) => x.classList.toggle('on', x.dataset.filter === f));
  rerenderAll();
}
$('#filter').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) setFilter(b.dataset.filter);
});
$('#clear').addEventListener('click', () => invoke('clear_finished'));

// ---------- Engine ----------
let engineCardTimer;
function renderEngine(e) {
  state.engine = e;
  const card = $('#engine-card');
  const setup = e.busy && !e.ready;
  clearTimeout(engineCardTimer);
  if (e.error) {
    card.hidden = false;
    card.classList.add('error');
    $('#engine-title').textContent = 'The download engine couldn’t start';
    $('#engine-stage').textContent = e.error;
    $('#engine-retry').hidden = false;
    card.querySelector('.bar').hidden = true;
  } else if (setup) {
    card.classList.remove('error');
    $('#engine-retry').hidden = true;
    card.querySelector('.bar').hidden = false;
    $('#engine-title').textContent = e.ytdlpVersion ? 'Updating the download engine' : 'Setting up the download engine';
    $('#engine-stage').textContent = e.stage || 'One-time setup, about 50 MB.';
    card.querySelector('.bar').classList.toggle('indeterminate', !e.progress);
    $('#engine-fill').style.width = `${(e.progress || 0) * 100}%`;
    // Quick checks on launch take a blink; only show the card for real work.
    if (card.hidden) engineCardTimer = setTimeout(() => (card.hidden = !(state.engine.busy && !state.engine.ready)), 700);
  } else {
    card.hidden = true;
  }

  const dot = $('#engine-status .dot');
  dot.className = `dot ${e.error ? 'err' : e.busy ? 'busy' : e.ready ? 'ok' : ''}`;
  $('#engine-status span:last-child').textContent = e.error
    ? 'Engine problem'
    : e.busy
      ? e.stage || 'Checking engine…'
      : e.ready
        ? `Ready · yt-dlp ${e.ytdlpVersion ?? ''}`
        : 'Starting…';

  $('#s-ytdlp').textContent = e.ytdlpVersion ? `Version ${e.ytdlpVersion}` : e.busy ? e.stage : 'Not installed yet';
  $('#s-ffmpeg').textContent =
    e.ffmpeg === 'system' ? 'Using the copy already on this computer' : e.ffmpeg === 'managed' ? 'Managed by slothDJ' : '…';
  state.jobs.forEach((j) => j.status === 'queued' && renderJob(j));
}
$('#engine-retry').addEventListener('click', () => invoke('update_engine').catch((err) => toast(String(err), true)));

// ---------- Settings ----------
const drawer = $('#drawer');
const openSettings = () => (drawer.hidden = false);
const closeSettings = () => (drawer.hidden = true);
$('#btn-settings').addEventListener('click', openSettings);
drawer.addEventListener('click', (e) => e.target.closest('[data-close]') && closeSettings());

function segment(sel, value) {
  $(sel).querySelectorAll('button').forEach((b) => b.classList.toggle('on', b.dataset.v === value));
}
function renderSettings() {
  const s = state.settings;
  $('#s-dir').textContent = s.outputDir;
  $('#s-dir').title = s.outputDir;
  $('#folder-path').textContent = prettyPath(s.outputDir);
  $('#folder-btn').title = `Open ${s.outputDir}`;
  $('#format').value = s.format;
  $('#s-format').value = s.format;
  $('#s-art').checked = s.embedArtwork;
  segment('#s-parallel', String(s.maxConcurrent));
  segment('#s-theme', s.theme);
  applyTheme();
}
async function saveSettings(patch) {
  state.settings = await invoke('save_settings', { settings: { ...state.settings, ...patch } });
  renderSettings();
}
function renderPort(port) {
  state.port = port;
  $('#ext-status span:last-child').textContent = port ? `Extension · port ${port}` : 'Extension API offline';
  $('#s-port').textContent = port ? `Listening on 127.0.0.1:${port}` : 'Not listening';
}

$('#s-format').addEventListener('change', (e) => saveSettings({ format: e.target.value }));
$('#s-art').addEventListener('change', (e) => saveSettings({ embedArtwork: e.target.checked }));
$('#s-parallel').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) saveSettings({ maxConcurrent: Number(b.dataset.v) });
});
$('#s-theme').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) saveSettings({ theme: b.dataset.v });
});
$('#s-dir-change').addEventListener('click', async () => {
  state.settings = await invoke('pick_output_dir');
  renderSettings();
});
$('#s-update').addEventListener('click', () =>
  invoke('update_engine').then(() => toast('Checking for engine updates…')).catch((err) => toast(String(err), true)),
);
$('#s-repair').addEventListener('click', () =>
  invoke('repair_engine').then(() => toast('Re-downloading the engine…')).catch((err) => toast(String(err), true)),
);
const openExt = () => invoke('open_url', { url: `${REPO}#browser-extension` });
$('#s-ext').addEventListener('click', openExt);
$('#get-ext').addEventListener('click', openExt);
$('#s-github').addEventListener('click', () => invoke('open_url', { url: REPO }));
$('#folder-btn').addEventListener('click', () => invoke('open_output_dir').catch((err) => toast(String(err), true)));

// ---------- Window ----------
$('#win-min').addEventListener('click', () => appWindow.minimize());
$('#win-max').addEventListener('click', () => appWindow.toggleMaximize());
$('#win-close').addEventListener('click', () => {
  const running = [...state.jobs.values()].filter((j) => RUNNING.includes(j.status)).length;
  if (running && !confirm(`${running} download${running > 1 ? 's are' : ' is'} still running. Quit anyway?`)) return;
  appWindow.close();
});

// Drag a link from the browser onto the window.
let dragDepth = 0;
const hasText = (e) => [...(e.dataTransfer?.types || [])].some((t) => t === 'text/uri-list' || t === 'text/plain');
window.addEventListener('dragenter', (e) => {
  if (!hasText(e)) return;
  e.preventDefault();
  dragDepth++;
  $('#drop-overlay').hidden = false;
});
window.addEventListener('dragover', (e) => {
  if (!hasText(e)) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = 'copy';
});
window.addEventListener('dragleave', () => {
  if (--dragDepth <= 0) {
    dragDepth = 0;
    $('#drop-overlay').hidden = true;
  }
});
window.addEventListener('drop', (e) => {
  e.preventDefault();
  dragDepth = 0;
  $('#drop-overlay').hidden = true;
  const text = e.dataTransfer.getData('text/uri-list') || e.dataTransfer.getData('text/plain') || '';
  const url = text.split(/\s+/).find((x) => /^https?:\/\//.test(x)) || text.trim();
  if (url) submit(url);
});

// Keyboard
document.addEventListener('keydown', (e) => {
  const mod = e.ctrlKey || e.metaKey;
  const key = e.key.toLowerCase();
  if (e.key === 'Escape' && !drawer.hidden) closeSettings();
  else if (mod && key === ',') {
    e.preventDefault();
    openSettings();
  } else if (mod && key === 'l') {
    e.preventDefault();
    input.focus();
    input.select();
  } else if (mod && key === 'v' && !e.target.closest('input, select')) {
    input.focus(); // the paste lands in the link box
  }
});
// No browser-style context menu, except in the text field.
document.addEventListener('contextmenu', (e) => {
  if (!e.target.closest('input')) e.preventDefault();
});

// ---------- Boot ----------
async function init() {
  hydrateIcons();
  const s = await invoke('get_state');
  if (s.platform === 'macos') document.body.classList.add('mac');
  state.formats = s.formats;
  state.settings = s.settings;
  const options = s.formats.map((f) => `<option value="${f.id}">${esc(f.label)}</option>`).join('');
  $('#format').innerHTML = options;
  $('#s-format').innerHTML = options;
  $('#s-version').textContent = `v${s.version}`;
  s.jobs.forEach((j) => state.jobs.set(j.id, j));
  renderSettings();
  renderEngine(s.engine);
  rerenderAll();
  renderPort(s.port);
  if (!s.port) setTimeout(async () => renderPort((await invoke('get_state')).port), 900);

  await listen('job', upsert);
  await listen('job-removed', (id) => {
    state.jobs.delete(id);
    list.querySelector(`[data-id="${id}"]`)?.remove();
    updateCounts();
    updateEmpty();
  });
  await listen('engine', renderEngine);
  await listen('settings', (st) => {
    state.settings = st;
    renderSettings();
  });
  await listen('toast', (m) => toast(m));

  input.focus();
  checkClipboard();
}

init().catch((e) => {
  console.error(e);
  toast(`Something went wrong: ${e}`, true);
});
