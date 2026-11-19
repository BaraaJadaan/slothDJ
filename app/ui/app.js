'use strict';
// SlothDJ UI. Plain JS, no build step; talks to the Rust core through Tauri's invoke/events.

const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const listen = (event, cb) => T.event.listen(event, (e) => cb(e.payload));
const appWindow = T.window.getCurrentWindow();
const REPO = 'https://github.com/BaraaJadaan/slothDJ';

// ---------- Icons (Lucide, ISC license) ----------
const ICONS = {
  link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
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
  film: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M7 3v18M17 3v18M3 7.5h4M3 12h18M3 16.5h4M17 7.5h4M17 16.5h4"/>',
  clipboard: '<rect width="8" height="4" x="8" y="2" rx="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
  external: '<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  sparkle: '<path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z"/><path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"/>',
  puzzle: '<path d="M19.44 7.85c-.05.32.06.65.29.88l1.57 1.57c.47.47.7 1.09.7 1.7s-.23 1.23-.7 1.7l-1.61 1.61a.98.98 0 0 1-.84.28c-.47-.07-.8-.48-.97-.93a2.5 2.5 0 1 0-3.21 3.21c.45.17.86.5.93.97a.98.98 0 0 1-.28.84l-1.61 1.61a2.4 2.4 0 0 1-1.7.7 2.4 2.4 0 0 1-1.7-.7l-1.57-1.57a1.03 1.03 0 0 0-.88-.29c-.49.07-.84.5-1.02.97a2.5 2.5 0 1 1-3.24-3.24c.46-.18.9-.53.97-1.02a1.03 1.03 0 0 0-.29-.88L2.7 13.7A2.4 2.4 0 0 1 2 12c0-.62.24-1.23.7-1.7L4.23 8.77c.24-.24.58-.35.92-.3.51.08.88.53 1.07 1.01a2.5 2.5 0 1 0 3.26-3.26c-.48-.2-.93-.56-1.01-1.07-.05-.34.06-.68.3-.92L10.3 2.7A2.4 2.4 0 0 1 12 2c.62 0 1.23.24 1.7.7l1.57 1.57c.23.23.56.34.88.29.49-.07.84-.5 1.02-.97a2.5 2.5 0 1 1 3.24 3.24c-.46.18-.9.53-.97 1.02Z"/>',
};
const icon = (name) => `<svg class="i" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ''}</svg>`;
function hydrateIcons(root = document) {
  root.querySelectorAll('[data-icon]:not([data-icon-done])').forEach((el) => {
    el.insertAdjacentHTML('afterbegin', icon(el.dataset.icon));
    el.setAttribute('data-icon-done', '');
  });
}
function setIcon(el, name) {
  el.dataset.icon = name;
  el.innerHTML = icon(name);
}

// ---------- Helpers ----------
const $ = (sel, root = document) => root.querySelector(sel);
/** User text (titles, artists, errors) in its own direction bubble, so Arabic/Hebrew can't reorder what's around it. */
const iso = (s) => `<bdi>${esc(s)}</bdi>`;
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
const RUNNING = ['starting', 'downloading', 'converting'];
const FINISHED = ['done', 'failed', 'canceled'];
const SITE_NAME = { soundcloud: 'SoundCloud', youtube: 'YouTube' };
const fmtSpeed = (b) => (!b ? '' : b >= 1048576 ? `${(b / 1048576).toFixed(1)} MB/s` : `${Math.max(1, Math.round(b / 1024))} KB/s`);
const fmtEta = (s) => {
  if (s == null || !isFinite(s) || s <= 0) return '';
  s = Math.round(s);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')} left`;
};
const fmtDuration = (s) => {
  if (!s) return '';
  s = Math.round(s);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, '0');
  return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`;
};
const prettyPath = (p) => {
  const sep = p.includes('\\') ? '\\' : '/';
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts.length > 2 ? `…${sep}${parts.slice(-2).join(sep)}` : p;
};
/** Is this text meant as a link (vs. a search)? */
const looksLikeLink = (t) => !/\s/.test(t.trim()) && /^(https?:\/\/)?([\w-]+\.)+[a-z]{2,}(\/|$|\?)/i.test(t.trim());

const state = { jobs: new Map(), settings: null, engine: {}, formats: [], filter: 'all', port: null, linkSite: null, platform: '' };
const formatInfo = (id) => state.formats.find((f) => f.id === id);
const formatLabel = (id) => formatInfo(id)?.label ?? id;
const isVideoFormat = (id) => !!formatInfo(id)?.video;

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

// ---------- Quality picker ----------
function formatOptions() {
  const opt = (f) => `<option value="${f.id}">${esc(f.label)}</option>`;
  const audio = state.formats.filter((f) => !f.video).map(opt).join('');
  const video = state.formats.filter((f) => f.video).map(opt).join('');
  return `<optgroup label="Audio">${audio}</optgroup><optgroup label="Video · YouTube only">${video}</optgroup>`;
}
/** Video options only make sense for YouTube links; SoundCloud is audio-only. */
function fitFormatToSite(site) {
  const sel = $('#format');
  const noVideo = site === 'soundcloud';
  sel.querySelector('optgroup[label^="Video"]').disabled = noVideo;
  if (noVideo && isVideoFormat(sel.value)) {
    const fallback = isVideoFormat(state.settings.format) ? 'mp3_320' : state.settings.format;
    sel.value = fallback;
  } else if (!noVideo && sel.value !== state.settings.format && !sel.dataset.touched) {
    sel.value = state.settings.format;
  }
}

// ---------- Omnibox: link or search ----------
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

function setMode(mode) {
  // mode: 'link' | 'search'
  const searching = mode === 'search';
  $('#go-label').textContent = searching ? 'Search' : 'Download';
  setIcon($('#go-icon'), searching ? 'search' : 'download');
  setIcon($('#omni-icon'), searching ? 'search' : 'link');
}

async function onInput() {
  const value = input.value.trim();
  const seq = ++previewSeq;
  clearTimeout(previewTimer);
  preview.hidden = true;
  setHint('');
  state.linkSite = null;
  if (!value) {
    setMode('link');
    fitFormatToSite(null);
    return;
  }
  if (!looksLikeLink(value)) {
    setMode('search');
    fitFormatToSite(null);
    if (value.length >= 2) setHint('Press Enter to search SoundCloud and YouTube.');
    return;
  }
  setMode('link');
  let info;
  try {
    info = await invoke('check_url', { url: value });
  } catch (e) {
    if (value.length > 10 && seq === previewSeq) setHint(String(e), true);
    return;
  }
  if (seq !== previewSeq) return;
  state.linkSite = info.site;
  fitFormatToSite(info.site);
  previewTimer = setTimeout(async () => {
    try {
      const p = await invoke('preview', { url: value });
      if (seq === previewSeq) showPreview(p);
    } catch (e) {
      if (seq === previewSeq) setHint(String(e), true);
    }
  }, 220);
}

function kindLabel(kind, site, format) {
  if (kind === 'set') return site === 'youtube' ? 'Playlist' : 'Set';
  if (kind === 'profile') return 'Profile';
  if (site === 'youtube') return isVideoFormat(format) ? 'Video' : '';
  return '';
}

let previewItem = null;
function showPreview(p) {
  const img = preview.querySelector('img');
  if (p.artwork) {
    img.src = p.artwork;
    img.style.visibility = '';
  } else {
    img.removeAttribute('src');
    img.style.visibility = 'hidden';
  }
  img.classList.toggle('wide', p.site === 'youtube');
  preview.querySelector('strong').textContent = p.title || 'Untitled';
  preview.querySelector('.preview-text span').innerHTML = [
    `<span class="site site-${p.site}">${SITE_NAME[p.site]}</span>`,
    p.artist ? iso(p.artist) : '',
  ].filter(Boolean).join(' · ');
  previewItem = p;
  const label = p.kind === 'track' ? (p.site === 'youtube' ? 'Video' : 'Track') : kindLabel(p.kind, p.site);
  preview.querySelector('.kind-pill').textContent = label;
  $('#preview-play').hidden = p.site === 'youtube' && p.kind !== 'track';
  preview.hidden = false;
}

async function addLink(url, format = $('#format').value) {
  const job = await invoke('add_download', { url, format });
  if (state.filter === 'finished') setFilter('all');
  upsert(job);
  return job;
}

async function submit(text) {
  const value = String(text ?? input.value).trim();
  if (!value) {
    input.focus();
    setHint('Paste a link or type a song name first.', true);
    return;
  }
  if (!looksLikeLink(value)) return runSearch(value);
  try {
    await addLink(value);
    input.value = '';
    preview.hidden = true;
    setHint('');
    hideClip();
    closeResults();
    setMode('link');
    fitFormatToSite(null);
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
$('#format').addEventListener('change', (e) => {
  e.target.dataset.touched = '1';
  // Remember the choice as the default, except video picks (SoundCloud can't use them).
  if (!isVideoFormat(e.target.value)) saveSettings({ format: e.target.value });
});

// Offer a link that's sitting on the clipboard.
let lastClip = '';
async function checkClipboard() {
  const text = await invoke('read_clipboard').catch(() => null);
  if (!text || text === lastClip || text === input.value.trim() || !looksLikeLink(text)) return;
  let info;
  try {
    info = await invoke('check_url', { url: text });
  } catch {
    return;
  }
  lastClip = text;
  if ([...state.jobs.values()].some((j) => j.url === info.url)) return;
  const chip = $('#clip-chip');
  chip.querySelector('.clip-text').textContent = info.url.replace(/^https:\/\/(www\.)?/, '');
  chip.dataset.url = text;
  chip.hidden = false;
}
function hideClip() {
  $('#clip-chip').hidden = true;
}
$('#clip-chip').addEventListener('click', (e) => submit(e.currentTarget.dataset.url));
window.addEventListener('focus', checkClipboard);

// ---------- Search ----------
const results = $('#results');
let searchSeq = 0;

function closeResults() {
  results.hidden = true;
  searchSeq++;
}
$('#results-close').addEventListener('click', closeResults);

async function runSearch(query) {
  const seq = ++searchSeq;
  setHint('');
  results.hidden = false;
  $('#results-title').innerHTML = `Results for “${iso(query)}”`;
  $('#results-body').innerHTML = `<div class="results-loading">Searching SoundCloud and YouTube…<div class="bar"><div class="fill"></div></div></div>`;
  let hits;
  try {
    hits = await invoke('search', { query });
  } catch (e) {
    if (seq === searchSeq) $('#results-body').innerHTML = `<p class="results-empty">${esc(e)}</p>`;
    return;
  }
  if (seq !== searchSeq) return;
  if (!hits.length) {
    $('#results-body').innerHTML = `<p class="results-empty">Nothing found. Try different words, or paste a link.</p>`;
    return;
  }
  const group = (site) => {
    const items = hits.filter((h) => h.site === site).slice(0, 6);
    if (!items.length) return '';
    return `<div class="results-group"><h3><span class="site site-${site}">${SITE_NAME[site]}</span></h3>${items
      .map(
        (h, i) => `<div class="hit ${site === 'soundcloud' ? 'sc' : ''}" data-site="${site}" data-i="${hits.indexOf(h)}">
          <img alt="" ${h.artwork ? `src="${esc(h.artwork)}"` : ''}>
          <div class="hit-text"><strong dir="auto" title="${esc(h.title)}">${esc(h.title)}</strong><span>${[h.artist ? iso(h.artist) : '', fmtDuration(h.duration)].filter(Boolean).join(' · ')}</span></div>
          <div class="hit-actions">
            <button class="ghost-btn small" data-play="${hits.indexOf(h)}" title="Listen or watch before downloading">${icon('play')}Preview</button>
            <button class="ghost-btn small" data-hit="${hits.indexOf(h)}">${icon('download')}Download</button>
          </div>
        </div>`,
      )
      .join('')}</div>`;
  };
  $('#results-body').innerHTML = group('youtube') + group('soundcloud');
  $('#results-body').querySelectorAll('[data-play]').forEach((b) =>
    b.addEventListener('click', () => openPlayer({ ...hits[Number(b.dataset.play)], kind: 'track' })),
  );
  $('#results-body').querySelectorAll('[data-hit]').forEach((b) =>
    b.addEventListener('click', async () => {
      const hit = hits[Number(b.dataset.hit)];
      try {
        await addLink(hit.url);
        b.classList.add('added');
        b.innerHTML = `${icon('check')}Added`;
        b.disabled = true;
      } catch (e) {
        toast(String(e), true);
      }
    }),
  );
}

// ---------- Removing downloads ----------
const trashName = () => (state.platform === 'windows' ? 'Recycle Bin' : 'Trash');
let removeTarget = null;

/** Remove a download, following Settings → "Remove button" (ask / keep the file / delete the file). */
async function removeJob(job, choice) {
  const hasFiles = job.status === 'done' && job.files.length > 0;
  if (!hasFiles) return invoke('remove_job', { id: job.id });
  const action = choice || state.settings.removeAction || 'ask';
  if (action === 'ask') return askRemove(job);
  const deleteFiles = action === 'file';
  const n = await invoke('remove_job', { id: job.id, deleteFiles });
  if (deleteFiles) toast(n ? `Moved ${n} file${n === 1 ? '' : 's'} to the ${trashName()}.` : 'Removed. The file was already gone.');
}

function askRemove(job) {
  removeTarget = job;
  const n = job.files.length;
  $('#remove-name').textContent = job.title || '';
  $('#remove-file-label').textContent = n > 1 ? `Delete ${n} files too` : 'Delete file too';
  $('#remove-explain').textContent = `Deleted files go to the ${trashName()}, so you can still restore them.`;
  $('#remove-remember').checked = false;
  $('#remove-dialog').hidden = false;
  $('#remove-list').focus();
}
function closeRemove() {
  $('#remove-dialog').hidden = true;
  removeTarget = null;
}
async function confirmRemove(action) {
  const job = removeTarget;
  if (!job) return;
  if ($('#remove-remember').checked) await saveSettings({ removeAction: action });
  closeRemove();
  try {
    await removeJob(job, action);
  } catch (e) {
    toast(String(e), true);
  }
}
$('#remove-dialog').addEventListener('click', (e) => e.target.closest('[data-close-remove]') && closeRemove());
$('#remove-list').addEventListener('click', () => confirmRemove('list'));
$('#remove-file').addEventListener('click', () => confirmRemove('file'));

// ---------- Preview player ----------
let playerItem = null;
let playerSeq = 0;

function closePlayer() {
  playerSeq++;
  $('#player').hidden = true;
  $('#player-stage').innerHTML = ''; // removing the media element stops playback
  playerItem = null;
}

function playerMessage(text, isErr) {
  const box = $('#player-stage .loading');
  if (!box) return;
  box.classList.toggle('err', !!isErr);
  box.textContent = text;
}

function updatePlayerDownload() {
  const fmt = $('#format').value;
  // SoundCloud is audio-only: a video pick falls back to the default audio quality.
  const effective =
    playerItem?.site === 'soundcloud' && isVideoFormat(fmt)
      ? isVideoFormat(state.settings.format) ? 'mp3_320' : state.settings.format
      : fmt;
  $('#player-download-label').textContent = `Download · ${formatLabel(effective)}`;
  $('#player-note').textContent = playerItem?.site === 'youtube' ? 'The preview plays in low quality; the download uses your choice.' : '';
}

async function openPlayer(item) {
  const seq = ++playerSeq;
  playerItem = item;
  $('#player-title').textContent = item.title || 'Preview';
  $('#player-sub').innerHTML = [`<span class="site site-${item.site}">${SITE_NAME[item.site]}</span>`, item.artist ? iso(item.artist) : '']
    .filter(Boolean)
    .join(' · ');
  const stage = $('#player-stage');
  stage.className = `player-stage${item.site === 'soundcloud' ? ' widget' : ''}`;
  stage.innerHTML = `${item.artwork ? `<img class="poster" alt="" src="${esc(item.artwork)}">` : ''}
    <div class="loading"><div class="spinner"></div>${item.site === 'youtube' ? 'Getting a preview from YouTube…' : 'Loading the SoundCloud player…'}</div>`;
  updatePlayerDownload();
  $('#player').hidden = false;
  let stream;
  try {
    stream = await invoke('preview_stream', { url: item.url });
  } catch (e) {
    if (seq === playerSeq) playerMessage(String(e), true);
    return;
  }
  if (seq !== playerSeq) return;
  if (stream.kind === 'widget') {
    stage.innerHTML = `<iframe src="${esc(stream.src)}" allow="autoplay; encrypted-media" title="SoundCloud player"></iframe>`;
  } else {
    const video = document.createElement('video');
    Object.assign(video, { src: stream.src, controls: true, autoplay: true, playsInline: true });
    video.addEventListener('error', () => {
      stage.innerHTML = `<div class="loading err">This one can't be previewed here, but you can still download it.</div>`;
    });
    stage.replaceChildren(video);
  }
}

$('#player').addEventListener('click', (e) => e.target.closest('[data-close-player]') && closePlayer());
$('#player-download').addEventListener('click', async () => {
  if (!playerItem) return;
  const item = playerItem;
  try {
    await addLink(item.url);
    closePlayer();
    toast(`Added: ${item.title || 'download'}`);
  } catch (e) {
    toast(String(e), true);
  }
});
$('#preview-play').addEventListener('click', () => previewItem && openPlayer(previewItem));
$('#format').addEventListener('change', updatePlayerDownload);

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
    <div class="art"><div class="ph">${icon(isVideoFormat(job.format) ? 'film' : 'music')}</div><span class="badge"></span></div>
    <div class="job-main">
      <div class="job-title"><strong></strong><span class="kind-pill" hidden></span></div>
      <div class="job-meta"></div>
      <div class="bar"><div class="fill"></div></div>
    </div>
    <div class="job-actions"></div>`;
  return li;
}

function statusText(j) {
  const word = j.site === 'youtube' ? 'Video' : 'Track';
  const setPrefix = j.kind !== 'track' && j.total ? `${word} ${j.index} of ${j.total}` : '';
  const video = isVideoFormat(j.format);
  switch (j.status) {
    case 'queued':
      return state.engine.ready ? 'Waiting for a free slot…' : 'Waiting for the engine…';
    case 'starting':
      return j.note ? iso(j.note) : 'Starting…';
    case 'downloading':
      if (setPrefix) return [setPrefix, j.current ? iso(j.current) : '', fmtSpeed(j.speed)].filter(Boolean).join(' · ');
      return [`${Math.round(j.progress * 100)}%`, fmtSpeed(j.speed), fmtEta(j.eta)].filter(Boolean).join(' · ');
    case 'converting': {
      const what = video ? 'Merging video and sound…' : j.format === 'original' ? 'Finishing up…' : 'Converting to MP3…';
      return setPrefix ? `${setPrefix} · ${what}` : what;
    }
    case 'done': {
      const n = j.files.length;
      const things = video ? 'video' : j.site === 'youtube' ? 'file' : 'track';
      let text = j.kind === 'track' ? `Saved · ${esc(formatLabel(j.format))}` : `${n} ${things}${n === 1 ? '' : 's'} saved`;
      if (j.warning) text += ` <span class="warn">· ${iso(j.warning)}</span>`;
      return text;
    }
    case 'failed':
      return iso(j.error || 'Download failed.');
    case 'canceled':
      return 'Canceled';
    default:
      return '';
  }
}

const removeTitle = () =>
  ({ ask: 'Remove…', list: 'Remove from list (keeps the file)', file: 'Remove and delete the file' })[state.settings?.removeAction] || 'Remove…';

const btn = (action, ic, title, cls = '') =>
  `<button class="icon-btn ${cls}" data-action="${action}" title="${title}" aria-label="${title}">${icon(ic)}</button>`;

function actionsFor(j) {
  if (!FINISHED.includes(j.status)) return btn('cancel', 'x', 'Cancel', 'danger');
  if (j.status === 'done') {
    const file = j.files[0];
    return (
      (file && j.kind === 'track' ? btn('play', 'play', 'Play') : '') +
      (file ? btn('reveal', 'folder', 'Show in folder') : '') +
      btn('remove', 'trash', removeTitle(), 'danger')
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
  title.dir = 'auto';
  const pill = li.querySelector('.kind-pill');
  const label = kindLabel(j.kind, j.site, j.format);
  pill.hidden = !label;
  pill.textContent = label;

  const meta = [
    `<span class="site site-${j.site}">${SITE_NAME[j.site] || ''}</span>`,
    j.artist ? iso(j.artist) : '',
    `<bdi dir="ltr" class="job-status">${statusText(j)}</bdi>`,
  ].filter(Boolean);
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
      case 'remove': await removeJob(job); break;
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
    e.ffmpeg === 'system' ? 'Using the copy already on this computer' : e.ffmpeg === 'managed' ? 'Managed by SlothDJ' : '…';
  const js = e.js || '';
  $('#s-js').textContent = js.startsWith('node')
    ? 'Uses Node.js, already on this computer'
    : js.startsWith('deno (system')
      ? 'Uses Deno, already on this computer'
      : js.startsWith('deno')
        ? 'Uses Deno, downloaded by SlothDJ'
        : 'Set up automatically on your first YouTube download';
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
  if (!$('#format').dataset.touched) $('#format').value = s.format;
  $('#s-format').value = s.format;
  $('#s-art').checked = s.embedArtwork;
  $('#s-tray').checked = s.closeToTray;
  $('#s-notify').checked = s.notify;
  $('#s-autostart').checked = s.startWithSystem;
  segment('#s-parallel', String(s.maxConcurrent));
  segment('#s-theme', s.theme);
  segment('#s-remove', s.removeAction || 'ask');
  $('#s-remove-hint').textContent = {
    ask: 'Asks each time whether to delete the file',
    list: 'Removes it from the list and keeps the file',
    file: `Also moves the file to the ${trashName()}`,
  }[s.removeAction || 'ask'];
  document.querySelectorAll('[data-action="remove"]').forEach((b) => {
    b.title = removeTitle();
    b.setAttribute('aria-label', removeTitle());
  });
  applyTheme();
  renderExtension();
}
async function saveSettings(patch) {
  state.settings = await invoke('save_settings', { settings: { ...state.settings, ...patch } });
  renderSettings();
}
function renderPort(port) {
  state.port = port;
  $('#s-port').textContent = port ? `Listening on 127.0.0.1:${port}` : 'Not listening';
  renderExtension();
}

const EXT_FRESH_MS = 30 * 24 * 60 * 60 * 1000;
/** Newer-than check for "1.2.0"-style versions. */
const olderThan = (a, b) => {
  const pa = String(a || '0').split('.').map(Number);
  const pb = String(b || '0').split('.').map(Number);
  for (let i = 0; i < 3; i++) if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) < (pb[i] || 0);
  return false;
};
const ago = (ms) => {
  const m = Math.round((Date.now() - ms) / 60000);
  return m < 1 ? 'just now' : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.round(m / 60)} h ago` : `${Math.round(m / 1440)} days ago`;
};

/** Once the extension has checked in, stop suggesting it; suggest updating it if it's older than the app. */
function renderExtension() {
  const s = state.settings;
  if (!s) return;
  const installed = s.extensionSeen && Date.now() - s.extensionSeen < EXT_FRESH_MS;
  const outdated = installed && olderThan(s.extensionVersion, state.version);
  $('#get-ext').hidden = !!installed;
  $('#empty-text').innerHTML = installed
    ? 'Paste a SoundCloud or YouTube link above, type a song name to search, or use the <b>SlothDJ</b> button on SoundCloud and YouTube.'
    : 'Paste a SoundCloud or YouTube link above, type a song name to search, or hit the <b>SlothDJ</b> button in your browser.';
  $('#ext-status span:last-child').textContent = installed
    ? 'Extension connected'
    : state.port ? `Extension · port ${state.port}` : 'Extension API offline';
  $('#s-ext-label').textContent = installed
    ? `Installed · ${s.extensionVersion ? `v${s.extensionVersion}` : 'older version'}${outdated ? ' · update available' : ''}`
    : 'Not installed yet';
  $('#s-ext').hidden = installed && !outdated;
  $('#s-ext-btn').textContent = outdated ? 'Update' : 'Set up';
  renderGuideStatus();
  $('#s-port').textContent = installed
    ? `Last seen ${ago(s.extensionSeen)} · port ${state.port ?? '…'}`
    : state.port ? `Listening on 127.0.0.1:${state.port}` : 'Not listening';
}

$('#s-format').addEventListener('change', (e) => {
  delete $('#format').dataset.touched;
  saveSettings({ format: e.target.value });
});
$('#s-art').addEventListener('change', (e) => saveSettings({ embedArtwork: e.target.checked }));
$('#s-tray').addEventListener('change', (e) => saveSettings({ closeToTray: e.target.checked }));
$('#s-notify').addEventListener('change', (e) => saveSettings({ notify: e.target.checked }));
$('#s-autostart').addEventListener('change', async (e) => {
  try {
    state.settings = await invoke('set_autostart', { enabled: e.target.checked });
  } catch (err) {
    toast(String(err), true);
  }
  renderSettings();
});
$('#s-parallel').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) saveSettings({ maxConcurrent: Number(b.dataset.v) });
});
$('#s-remove').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) saveSettings({ removeAction: b.dataset.v });
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
// ---------- Browser extension setup guide ----------
const guide = $('#ext-guide');
function closeGuide() {
  guide.hidden = true;
}
async function openGuide() {
  const s = state.settings;
  const outdated = s.extensionSeen && olderThan(s.extensionVersion, state.version);
  $('#guide-title').textContent = outdated ? 'Update the SlothDJ extension' : 'Add SlothDJ to your browser';
  $('#guide-remove').hidden = !outdated;
  $('#guide-old-note').hidden = !!outdated; // the dedicated step covers it when we know
  $('#guide-path').textContent = 'Preparing…';
  $('#guide-browsers').innerHTML = '';
  guide.hidden = false;
  renderGuideStatus();
  try {
    const setup = await invoke('extension_setup');
    $('#guide-path').textContent = setup.folder;
    $('#guide-path').title = setup.folder;
    $('#guide-browsers').innerHTML = setup.browsers
      .map((b) => `<button class="ghost-btn small" data-browser="${b.id}">${icon('external')}Open in ${esc(b.name)}</button>`)
      .join('');
    $('#guide-browsers-hint').innerHTML = setup.browsers.length
      ? 'Or type <code>chrome://extensions</code> in the address bar (<code>edge://extensions</code> in Edge).'
      : 'Type <code>chrome://extensions</code> in your browser’s address bar (<code>edge://extensions</code> in Edge).';
    toast('Folder path copied. Paste it in step 3.');
  } catch (e) {
    $('#guide-path').textContent = String(e);
  }
}
function renderGuideStatus() {
  if (guide.hidden) return;
  const s = state.settings;
  const ok = s.extensionSeen && Date.now() - s.extensionSeen < EXT_FRESH_MS && !olderThan(s.extensionVersion, state.version);
  const box = $('#guide-status');
  box.classList.toggle('ok', !!ok);
  box.querySelector('.dot').className = `dot ${ok ? 'ok' : 'busy'}`;
  box.querySelector('span').textContent = ok
    ? `Connected! The SlothDJ button is ready on SoundCloud and YouTube${s.extensionVersion ? ` (extension v${s.extensionVersion})` : ''}.`
    : 'Waiting for the extension to say hello…';
}
guide.addEventListener('click', async (e) => {
  if (e.target.closest('[data-close-guide]')) return closeGuide();
  const b = e.target.closest('[data-browser]');
  if (b) invoke('open_extensions_page', { browser: b.dataset.browser }).catch((err) => toast(String(err), true));
});
$('#guide-copy').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText($('#guide-path').textContent);
    toast('Folder path copied.');
  } catch {
    toast('Select the path and press Ctrl+C to copy it.', true);
  }
});
$('#guide-show').addEventListener('click', () => invoke('reveal_extension_folder').catch((err) => toast(String(err), true)));
$('#s-ext').addEventListener('click', () => {
  closeSettings();
  openGuide();
});
$('#get-ext').addEventListener('click', openGuide);
$('#s-github').addEventListener('click', (e) => {
  e.preventDefault();
  invoke('open_url', { url: REPO });
});
$('#folder-btn').addEventListener('click', () => invoke('open_output_dir').catch((err) => toast(String(err), true)));

// ---------- App updates ----------
function showUpdate(info) {
  $('#update-title').textContent = `SlothDJ ${info.version} is ready`;
  $('#update-sub').textContent = 'Installs in a few seconds and restarts SlothDJ.';
  $('#update-banner').hidden = false;
  $('#s-app-update').textContent = `Version ${info.version} is available`;
}
async function checkAppUpdate() {
  const label = $('#s-app-update');
  label.textContent = 'Checking…';
  try {
    const info = await invoke('check_update');
    if (info) showUpdate(info);
    else label.textContent = 'You have the latest version.';
  } catch (err) {
    label.textContent = String(err);
  }
}
$('#s-check-app').addEventListener('click', checkAppUpdate);
$('#update-dismiss').addEventListener('click', () => ($('#update-banner').hidden = true));
$('#update-now').addEventListener('click', async () => {
  const running = [...state.jobs.values()].filter((j) => RUNNING.includes(j.status)).length;
  if (running && !confirm(`${running} download${running > 1 ? 's are' : ' is'} still running and will stop. Update now?`)) return;
  $('#update-now').disabled = true;
  $('#update-sub').textContent = 'Downloading the update…';
  $('#update-banner .bar').hidden = false;
  try {
    await invoke('install_update'); // restarts SlothDJ when done
  } catch (err) {
    $('#update-now').disabled = false;
    $('#update-banner .bar').hidden = true;
    $('#update-sub').textContent = String(err);
  }
});

// ---------- Window ----------
$('#win-min').addEventListener('click', () => appWindow.minimize());
$('#win-max').addEventListener('click', () => appWindow.toggleMaximize());
$('#win-close').addEventListener('click', () => {
  // With "keep running in the tray" on, closing just hides the window and downloads carry on.
  if (!state.settings.closeToTray) {
    const running = [...state.jobs.values()].filter((j) => RUNNING.includes(j.status)).length;
    if (running && !confirm(`${running} download${running > 1 ? 's are' : ' is'} still running. Quit anyway?`)) return;
  }
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
  if (e.key === 'Escape') {
    if (!$('#remove-dialog').hidden) closeRemove();
    else if (!$('#player').hidden) closePlayer();
    else if (!guide.hidden) closeGuide();
    else if (!drawer.hidden) closeSettings();
    else if (!results.hidden) closeResults();
  } else if (mod && key === ',') {
    e.preventDefault();
    openSettings();
  } else if (mod && (key === 'l' || key === 'f')) {
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
  state.platform = s.platform;
  if (s.platform === 'macos') document.body.classList.add('mac');
  $('#s-autostart-label').textContent =
    s.platform === 'macos' ? 'Open at login' : s.platform === 'windows' ? 'Start with Windows' : 'Start when you log in';
  state.formats = s.formats;
  state.settings = s.settings;
  $('#format').innerHTML = formatOptions();
  $('#s-format').innerHTML = state.formats
    .filter((f) => !f.video)
    .map((f) => `<option value="${f.id}">${esc(f.label)}</option>`)
    .join('');
  $('#s-version').textContent = `v${s.version}`;
  state.version = s.version;
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
  await listen('update-available', showUpdate);
  await listen('update-progress', ([got, total]) => {
    if (total) $('#update-fill').style.width = `${Math.min(100, (got / total) * 100)}%`;
  });

  input.focus();
  checkClipboard();
}

init().catch((e) => {
  console.error(e);
  toast(`Something went wrong: ${e}`, true);
});
