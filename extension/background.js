// slothDJ extension service worker: the only part that talks to the desktop app.
importScripts('shared.js');

const PORTS = [37771, 37772, 37773, 37774, 37775];
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

/** Find the port slothDJ is listening on (it picks the first free one of five). */
async function findPort() {
  const order = knownPort ? [knownPort, ...PORTS.filter((p) => p !== knownPort)] : PORTS;
  for (const port of order) {
    try {
      const { data } = await call(port, '/api/status');
      if (data?.app === 'slothDJ') {
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

async function getFormat() {
  const { format } = await chrome.storage.sync.get('format');
  return format || null;
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
    body: JSON.stringify({ url, format: format ?? (await getFormat()) }),
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
      case 'show':
        return api('/api/show', { method: 'POST' });
      default:
        return { error: 'unknown message' };
    }
  })().then(reply, (e) => reply({ error: String(e) }));
  return true; // async reply
});

// Right-click menu on SoundCloud links and pages.
chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: 'sdj-link',
      title: 'Download with slothDJ',
      contexts: ['link'],
      targetUrlPatterns: ['https://soundcloud.com/*', 'https://m.soundcloud.com/*', 'https://on.soundcloud.com/*'],
    });
    chrome.contextMenus.create({
      id: 'sdj-page',
      title: 'Download this page with slothDJ',
      contexts: ['page'],
      documentUrlPatterns: ['https://soundcloud.com/*', 'https://m.soundcloud.com/*'],
    });
  });
});

async function flashBadge(text, color) {
  await chrome.action.setBadgeBackgroundColor({ color });
  await chrome.action.setBadgeText({ text });
  setTimeout(() => chrome.action.setBadgeText({ text: '' }), 2500);
}

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  const url = info.menuItemId === 'sdj-link' ? info.linkUrl : info.pageUrl;
  if (!SDJ.classify(url)) return flashBadge('?', '#6f6884');
  const res = await download(url);
  if (tab?.id) chrome.tabs.sendMessage(tab.id, { type: 'sent', url, res }).catch(() => {});
  if (res.ok) flashBadge('✓', '#34d399');
  else flashBadge(res.offline ? 'off' : '!', '#f87171');
});
