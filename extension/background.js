// SlothDJ extension service worker: the only part that talks to the desktop app.
importScripts('shared.js');

const PORTS = [37771, 37772, 37773, 37774, 37775];
// Sent with status checks so the app knows the extension is installed (and stops suggesting it).
const EXT_VERSION = chrome.runtime.getManifest().version;
let knownPort = null;

async function call(port, path, init = {}) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 1500);
  try {
    const res = await fetch(`http://127.0.0.1:${port}${path}`, { ...init, signal: ctrl.signal });
    return { status: res.status, data: await res.json() };
  } finally {
    clearTimeout(timer);
  }
}

/** Find the port SlothDJ is listening on (it picks the first free one of five). */
async function findPort() {
  const order = knownPort ? [knownPort, ...PORTS.filter((p) => p !== knownPort)] : PORTS;
  for (const port of order) {
    try {
      const { data } = await call(port, `/api/status?ext=${EXT_VERSION}`);
      if (String(data?.app).toLowerCase() === 'slothdj') {
        knownPort = port;
        return { port, status: data };
      }
    } catch {
      /* not this one */
    }
  }
  knownPort = null;
  return null;
}

async function api(path, init) {
  const found = await findPort();
  if (!found) return { offline: true };
  try {
    const { status, data } = await call(found.port, path, init);
    return { status, ...data };
  } catch {
    knownPort = null;
    return { offline: true };
  }
}

/** The quality remembered for this site (empty = the app's default). */
async function savedFormat(url) {
  const site = SDJ.classify(url)?.site;
  const key = SDJ.formatKey(site);
  const stored = await chrome.storage.sync.get(key);
  return stored[key] || null;
}

async function remember(job) {
  const { recent = [] } = await chrome.storage.local.get('recent');
  const next = [{ id: job.id, title: job.title, url: job.url, at: Date.now() }, ...recent.filter((r) => r.id !== job.id)].slice(0, 8);
  await chrome.storage.local.set({ recent: next });
}

async function download(url, format) {
  const res = await api('/api/download', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url, format: format ?? (await savedFormat(url)) }),
  });
  if (res.ok && res.job) await remember(res.job);
  return res;
}

chrome.runtime.onMessage.addListener((msg, _sender, reply) => {
  (async () => {
    switch (msg.type) {
      case 'status': {
        const found = await findPort();
        return found ? { online: true, ...found.status } : { online: false };
      }
      case 'download':
        return download(msg.url, msg.format);
      case 'job':
        return api(`/api/jobs/${encodeURIComponent(msg.id)}`);
      case 'cancel':
        return api(`/api/jobs/${encodeURIComponent(msg.id)}/cancel`, { method: 'POST' });
      case 'hello':
        // A SoundCloud/YouTube page opened: let the app know we're here (at most every few minutes).
        if (Date.now() - lastHello > 5 * 60 * 1000) {
          lastHello = Date.now();
          await findPort();
        }
        return { ok: true };
      case 'show':
        return api('/api/show', { method: 'POST' });
      default:
        return { error: 'unknown message' };
    }
  })().then(reply, (e) => reply({ error: String(e) }));
  return true; // async reply
});

// Right-click menus. YouTube gets separate audio and video entries.
const SC_LINKS = ['https://soundcloud.com/*', 'https://m.soundcloud.com/*', 'https://on.soundcloud.com/*'];
const SC_PAGES = ['https://soundcloud.com/*', 'https://m.soundcloud.com/*'];
const YT_LINKS = [
  'https://www.youtube.com/watch*', 'https://m.youtube.com/watch*', 'https://music.youtube.com/watch*',
  'https://www.youtube.com/shorts/*', 'https://www.youtube.com/playlist*', 'https://music.youtube.com/playlist*',
  'https://youtu.be/*',
];
const YT_PAGES = ['https://www.youtube.com/watch*', 'https://www.youtube.com/shorts/*', 'https://www.youtube.com/playlist*', 'https://music.youtube.com/*'];

let lastHello = 0;
chrome.runtime.onStartup.addListener(() => findPort());

chrome.runtime.onInstalled.addListener(() => {
  findPort();
  chrome.contextMenus.removeAll(() => {
    const add = (id, title, contexts, patterns) =>
      chrome.contextMenus.create({
        id,
        title,
        contexts,
        ...(contexts[0] === 'link' ? { targetUrlPatterns: patterns } : { documentUrlPatterns: patterns }),
      });
    add('sc-link', 'Download with SlothDJ', ['link'], SC_LINKS);
    add('sc-page', 'Download this page with SlothDJ', ['page'], SC_PAGES);
    add('yt-link-audio', 'Download audio with SlothDJ (MP3)', ['link'], YT_LINKS);
    add('yt-link-video', 'Download video with SlothDJ (MP4)', ['link'], YT_LINKS);
    add('yt-page-audio', 'Download audio with SlothDJ (MP3)', ['page'], YT_PAGES);
    add('yt-page-video', 'Download video with SlothDJ (MP4)', ['page'], YT_PAGES);
  });
});

async function flashBadge(text, color) {
  await chrome.action.setBadgeBackgroundColor({ color });
  await chrome.action.setBadgeText({ text });
  setTimeout(() => chrome.action.setBadgeText({ text: '' }), 2500);
}

/** Video menu entry: the remembered YouTube video quality, else 1080p. Audio: remembered audio, else app default. */
async function formatForMenu(menuId, url) {
  const saved = await savedFormat(url);
  const isVideo = (f) => !!f && f.startsWith('mp4_');
  if (menuId.endsWith('-video')) return isVideo(saved) ? saved : 'mp4_1080';
  // '' (not null) means "the app's default", which is always an audio format.
  if (menuId.endsWith('-audio')) return isVideo(saved) ? '' : saved || '';
  return saved;
}

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  const id = String(info.menuItemId);
  const url = id.includes('-link') ? info.linkUrl : info.pageUrl;
  if (!SDJ.classify(url)) return flashBadge('?', '#6f6884');
  const res = await download(url, await formatForMenu(id, url));
  if (tab?.id) chrome.tabs.sendMessage(tab.id, { type: 'sent', url, res }).catch(() => {});
  if (res.ok) flashBadge('✓', '#34d399');
  else flashBadge(res.offline ? 'off' : '!', '#f87171');
});
