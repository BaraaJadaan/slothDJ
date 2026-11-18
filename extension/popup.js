'use strict';
/* global SDJ */
const $ = (s) => document.querySelector(s);
const send = (msg) => chrome.runtime.sendMessage(msg).catch(() => ({ offline: true }));
const FALLBACK_FORMATS = [
  { id: 'mp3_320', label: 'MP3 · 320 kbps' },
  { id: 'mp3_192', label: 'MP3 · 192 kbps' },
  { id: 'mp3_128', label: 'MP3 · 128 kbps' },
  { id: 'original', label: 'Original (no re-encode)' },
];

let tab = null;
let target = null;
let status = { online: false };

async function init() {
  [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  target = tab?.url ? SDJ.classify(tab.url) : null;
  status = await send({ type: 'status' });

  renderConnection();
  await renderFormats();
  renderTarget();
  renderRecent();
  setInterval(renderRecent, 1000);
}

function renderConnection() {
  const pill = $('#conn');
  pill.className = `pill ${status.online ? 'on' : 'off'}`;
  pill.querySelector('span').textContent = status.online ? 'App connected' : 'App offline';
  pill.title = status.online ? `slothDJ ${status.version} · click to open` : 'Start the slothDJ desktop app';
  $('#offline').hidden = status.online;
}

async function renderFormats() {
  const { format = '' } = await chrome.storage.sync.get('format');
  const formats = status.formats?.length ? status.formats : FALLBACK_FORMATS;
  const def = formats.find((f) => f.id === status.defaultFormat);
  const sel = $('#format');
  sel.innerHTML =
    `<option value="">${def ? `App default (${def.label.replace('MP3 · ', 'MP3 ')})` : 'App default'}</option>` +
    formats.map((f) => `<option value="${f.id}">${f.label}</option>`).join('');
  sel.value = format;
  sel.addEventListener('change', () => chrome.storage.sync.set({ format: sel.value }));
}

async function renderTarget() {
  if (!target) {
    $('#not-sc').hidden = false;
    return;
  }
  $('#current').hidden = false;
  $('#controls').hidden = false;
  $('#kind').textContent = SDJ.KIND_LABEL[target.kind];
  $('#title').textContent = target.url.replace('https://soundcloud.com/', '');
  const btn = $('#dl');
  btn.querySelector('span').textContent = status.online ? 'Download' : 'Open slothDJ & download';

  try {
    const q = new URLSearchParams({ format: 'json', url: target.url });
    const res = await fetch(`https://soundcloud.com/oembed?${q}`);
    if (res.ok) {
      const o = await res.json();
      const artist = o.author_name || '';
      $('#title').textContent = (o.title || '').replace(new RegExp(` by ${artist.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`), '') || 'Untitled';
      $('#artist').textContent = artist;
      if (o.thumbnail_url) $('#art').src = o.thumbnail_url.replace('-t500x500.', '-t300x300.');
    }
  } catch {
    /* the preview is a nicety */
  }
}

$('#dl').addEventListener('click', async () => {
  const btn = $('#dl');
  const label = btn.querySelector('span');
  const format = $('#format').value || null;
  $('#msg').hidden = true;

  if (!status.online) {
    // Launch the app through its slothdj:// link; it queues the download itself.
    chrome.tabs.update(tab.id, { url: SDJ.deepLink(target.url, format) });
    label.textContent = 'Opening slothDJ…';
    setTimeout(() => window.close(), 1200);
    return;
  }

  btn.disabled = true;
  label.textContent = 'Sending…';
  const res = await send({ type: 'download', url: target.url, format });
  if (res.ok) {
    btn.classList.add('sent');
    label.textContent = 'Sent to slothDJ ✓';
    renderRecent();
  } else {
    btn.disabled = false;
    label.textContent = 'Download';
    $('#msg').textContent = res.offline ? 'slothDJ stopped responding. Is it still open?' : res.error || 'Something went wrong.';
    $('#msg').hidden = false;
  }
});

$('#conn').addEventListener('click', () => status.online && send({ type: 'show' }));
$('#open-sc').addEventListener('click', () => chrome.tabs.create({ url: 'https://soundcloud.com/discover' }));
$('#get-app').addEventListener('click', () => chrome.tabs.create({ url: `${SDJ.REPO}/releases/latest` }));

const STATUS_TEXT = {
  queued: 'Queued',
  starting: 'Starting',
  converting: 'Converting',
  done: 'Saved',
  failed: 'Failed',
  canceled: 'Canceled',
};

async function renderRecent() {
  const { recent = [] } = await chrome.storage.local.get('recent');
  const items = recent.slice(0, 5);
  $('#recent-wrap').hidden = items.length === 0;
  if (!items.length) return;

  const jobs = status.online ? await Promise.all(items.map((r) => send({ type: 'job', id: r.id }))) : [];
  const ul = $('#recent');
  ul.replaceChildren(
    ...items.map((r, i) => {
      const job = jobs[i]?.ok ? jobs[i].job : null;
      const li = document.createElement('li');
      const st = job?.status;
      const text = !job
        ? ''
        : st === 'downloading'
          ? job.kind !== 'track' && job.total
            ? `${job.index}/${job.total}`
            : `${Math.round(job.progress * 100)}%`
          : STATUS_TEXT[st] || '';
      li.innerHTML = `<span class="t"></span><span class="s ${st || ''}"></span>`;
      li.querySelector('.t').textContent = job?.title || r.title;
      li.querySelector('.s').textContent = text;
      if (job && !['done', 'failed', 'canceled'].includes(st)) {
        const bar = document.createElement('span');
        bar.className = 'bar';
        bar.innerHTML = `<i style="width:${Math.round(job.progress * 100)}%"></i>`;
        li.appendChild(bar);
      }
      li.title = 'Show in slothDJ';
      li.addEventListener('click', () => send({ type: 'show' }));
      return li;
    }),
  );
}

init();
