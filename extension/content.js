// Adds a slothDJ button on SoundCloud (next to Like/Repost/Share) and YouTube (under the video), plus a small progress toast.
(() => {
  if (window.__slothdj) return;
  window.__slothdj = true;

  const svg = (body) =>
    `<svg viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
  const ICON = {
    idle: svg('<path d="M8 1.8v8M4.7 6.6 8 9.9l3.3-3.3"/><path d="M2.6 11.4v1.3c0 .7.6 1.3 1.3 1.3h8.2c.7 0 1.3-.6 1.3-1.3v-1.3"/>'),
    busy: svg('<circle cx="8" cy="8" r="5.6" stroke-opacity=".25"/><path d="M13.6 8A5.6 5.6 0 0 0 8 2.4"/>'),
    done: svg('<path d="M3 8.6 6.4 12 13 4.6"/>'),
    error: svg('<circle cx="8" cy="8" r="6"/><path d="M8 4.9v3.5M8 11.1v.05"/>'),
  };
  const send = (msg) => chrome.runtime.sendMessage(msg).catch(() => ({ offline: true }));
  const keyOf = (url) => SDJ.classify(url)?.url;
  const jobs = new Map(); // sound key -> latest job (or a pseudo-state while sending)
  const polling = new Map(); // job id -> interval
  let root; // toast shadow root
  let toastJobId = null;
  let hideTimer;

  // ---------- Buttons ----------
  const onYouTube = /(^|\.)youtube\.com$/.test(location.hostname);

  function makeButton(className, url, labelClass) {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = className;
    b.innerHTML = `<div class="sdj-ico"></div><span class="${labelClass} sdj-label"></span>`;
    b.addEventListener('click', onClick);
    point(b, url);
    return b;
  }

  /** Aim a button at a (new) link, e.g. after YouTube navigates to another video. */
  function point(b, url) {
    b.dataset.url = url;
    b.dataset.key = keyOf(url);
    paint(b, jobs.get(b.dataset.key));
  }

  // SoundCloud: a button in every Like/Repost/Share bar (track page and every list item).
  function soundUrl(group) {
    if (group.closest('.listenEngagement, .l-listen-hero, .fullHero')) return location.origin + location.pathname;
    const item = group.closest('.trackItem, .sound, .soundBadge, .searchItem, .soundList__item, .compactTrackListItem');
    const a = item?.querySelector('a.trackItem__trackTitle, a.soundTitle__title, a.sc-link-primary[href]');
    return a ? new URL(a.getAttribute('href'), location.origin).href : null;
  }

  function scanSoundCloud() {
    document.querySelectorAll('.soundActions .sc-button-group:not(:has(> .sdj-btn))').forEach((group) => {
      const url = soundUrl(group);
      if (!url || !SDJ.classify(url)) return;
      const size = group.classList.contains('sc-button-group-small') ? 'small' : 'medium';
      const cls = `sc-button-secondary sc-button sc-button-${size} sc-button-icon sc-button-responsive sdj-btn`;
      group.appendChild(makeButton(cls, url, 'sc-button-label'));
    });
  }

  // YouTube: one button next to Like and Share under the video. YouTube is a single-page
  // app, so the same button is re-aimed whenever you open another video.
  function scanYouTube() {
    const target = SDJ.classify(location.href);
    const bar = document.querySelector('ytd-watch-metadata #top-level-buttons-computed');
    let b = document.querySelector('.sdj-yt-btn');
    if (!target || target.kind !== 'track' || !bar) return;
    if (!b || b.parentElement !== bar) {
      b?.remove();
      b = makeButton('sdj-btn sdj-yt-btn', target.url, '');
      bar.appendChild(b);
    } else if (b.dataset.key !== target.url) {
      point(b, target.url);
    }
  }

  const scan = onYouTube ? scanYouTube : scanSoundCloud;
  let scanTimer;
  new MutationObserver(() => {
    clearTimeout(scanTimer);
    scanTimer = setTimeout(scan, 250);
  }).observe(document.body, { childList: true, subtree: true });
  document.addEventListener('yt-navigate-finish', () => setTimeout(scan, 50));
  scan();

  // ---------- Job tracking ----------
  function paint(b, job) {
    let state = 'idle';
    let label = '';
    let title = 'Download with slothDJ';
    if (job) {
      switch (job.status) {
        case 'sending': state = 'busy'; label = 'Sending'; title = 'Sending to slothDJ…'; break;
        case 'queued':
        case 'starting': state = 'busy'; label = 'Queued'; title = 'Queued in slothDJ'; break;
        case 'downloading': state = 'busy'; label = `${Math.round(job.progress * 100)}%`; title = `Downloading: ${job.title}`; break;
        case 'converting': state = 'busy'; label = job.format?.startsWith('mp4_') ? 'Merging…' : 'MP3…'; title = 'Finishing up…'; break;
        case 'done': state = 'done'; title = `Saved by slothDJ: ${job.title}`; break;
        case 'failed': state = 'error'; title = job.error || 'Download failed'; break;
        default: break;
      }
    }
    if (b.dataset.state !== state) {
      b.dataset.state = state;
      b.querySelector('.sdj-ico').innerHTML = ICON[state];
    }
    const l = b.querySelector('.sdj-label');
    if (b.classList.contains('sdj-yt-btn')) {
      // YouTube's buttons carry text labels, so ours always does too.
      l.textContent = label || { done: 'Saved', error: 'Failed' }[state] || 'slothDJ';
    } else {
      l.textContent = label || 'slothDJ';
      l.classList.toggle('sc-visuallyhidden', !label);
      b.classList.toggle('sdj-wide', !!label);
    }
    b.title = title;
    b.setAttribute('aria-label', title);
  }

  function setJob(key, job) {
    if (job) jobs.set(key, job);
    else jobs.delete(key);
    document.querySelectorAll(`.sdj-btn[data-key="${CSS.escape(key)}"]`).forEach((b) => paint(b, job));
    if (job?.id && toastJobId === job.id) toastForJob(job);
  }

  function follow(key, job) {
    setJob(key, job);
    if (polling.has(job.id)) return;
    let misses = 0;
    const timer = setInterval(async () => {
      const res = await send({ type: 'job', id: job.id });
      if (!res?.ok) {
        if (++misses > 5) stop();
        return;
      }
      setJob(key, res.job);
      if (['done', 'failed', 'canceled'].includes(res.job.status)) stop();
    }, 1000);
    const stop = () => {
      clearInterval(timer);
      polling.delete(job.id);
    };
    polling.set(job.id, timer);
  }

  function handleResult(url, res) {
    const key = keyOf(url);
    if (res?.ok && res.job) {
      toastJobId = res.job.id;
      follow(key, res.job);
      toastForJob(res.job);
    } else if (res?.offline) {
      setJob(key, null);
      toastOffline(url);
    } else {
      setJob(key, { status: 'failed', error: res?.error });
      toast({ tone: 'err', title: 'Couldn’t add that', sub: res?.error || 'Something went wrong.' });
    }
  }

  async function onClick(e) {
    e.preventDefault();
    e.stopPropagation();
    const b = e.currentTarget;
    const current = jobs.get(b.dataset.key);
    if (current && ['sending', 'queued', 'starting', 'downloading', 'converting'].includes(current.status)) {
      toastJobId = current.id;
      return toastForJob(current);
    }
    setJob(b.dataset.key, { status: 'sending' });
    handleResult(b.dataset.url, await send({ type: 'download', url: b.dataset.url }));
  }

  // Results of the right-click menu, so the page shows feedback too.
  chrome.runtime.onMessage.addListener((msg) => {
    if (msg?.type === 'sent') handleResult(msg.url, msg.res);
  });

  // ---------- Toast (shadow DOM, so SoundCloud's CSS can't touch it) ----------
  function host() {
    if (root) return root;
    const el = document.createElement('div');
    el.id = 'slothdj-toast-host';
    root = el.attachShadow({ mode: 'open' });
    root.innerHTML = `
      <style>
        :host { all: initial; }
        .card { position: fixed; right: 20px; bottom: 84px; z-index: 2147483647; width: 320px; box-sizing: border-box;
          display: grid; grid-template-columns: 34px 1fr 22px; gap: 10px 12px; align-items: start;
          padding: 14px; border-radius: 14px; background: #18131f; color: #f4f1fa;
          font: 13px/1.4 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
          border: 1px solid rgba(255,255,255,.1); box-shadow: 0 18px 40px -12px rgba(0,0,0,.6);
          transform: translateY(8px); opacity: 0; transition: opacity .18s, transform .18s; pointer-events: none; }
        .card.show { opacity: 1; transform: none; pointer-events: auto; }
        .card::before { content: ""; position: absolute; inset: 0 0 auto 0; height: 2px; border-radius: 14px 14px 0 0;
          background: linear-gradient(90deg, #8b5cf6, #ec4899); }
        .card.err::before { background: #f87171; }
        img { width: 34px; height: 34px; border-radius: 9px; }
        .t { font-weight: 650; }
        .s { color: #a39cb5; font-size: 12px; overflow: hidden; text-overflow: ellipsis; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; }
        .x { all: unset; cursor: pointer; color: #6f6884; font-size: 18px; line-height: 1; text-align: center; }
        .x:hover { color: #f4f1fa; }
        .bar { grid-column: 2 / 4; height: 4px; border-radius: 99px; background: rgba(255,255,255,.08); overflow: hidden; }
        .fill { height: 100%; width: 0; background: linear-gradient(90deg, #8b5cf6, #ec4899); border-radius: 99px; transition: width .3s; }
        .acts { grid-column: 2 / 4; display: flex; gap: 8px; }
        .acts button { all: unset; cursor: pointer; padding: 6px 12px; border-radius: 9px; font-weight: 600; font-size: 12.5px;
          border: 1px solid rgba(255,255,255,.14); color: #f4f1fa; }
        .acts button.primary { border: 0; color: #fff; background: linear-gradient(135deg, #8b5cf6, #ec4899); }
        .acts button:hover { filter: brightness(1.1); }
        [hidden] { display: none !important; }
      </style>
      <div class="card" role="status">
        <img alt="" src="${chrome.runtime.getURL('icons/logo.svg')}">
        <div><div class="t"></div><div class="s"></div></div>
        <button class="x" aria-label="Close">×</button>
        <div class="bar" hidden><div class="fill"></div></div>
        <div class="acts" hidden></div>
      </div>`;
    root.querySelector('.x').addEventListener('click', hideToast);
    document.documentElement.appendChild(el);
    return root;
  }

  function toast({ tone = 'ok', title, sub = '', progress = null, actions = [], autoHide = 0 }) {
    const r = host();
    const card = r.querySelector('.card');
    card.classList.toggle('err', tone === 'err');
    r.querySelector('.t').textContent = title;
    r.querySelector('.s').textContent = sub;
    r.querySelector('.bar').hidden = progress == null;
    if (progress != null) r.querySelector('.fill').style.width = `${Math.round(progress * 100)}%`;
    const acts = r.querySelector('.acts');
    acts.hidden = !actions.length;
    acts.replaceChildren(
      ...actions.map((a) => {
        const b = document.createElement('button');
        b.textContent = a.label;
        if (a.primary) b.className = 'primary';
        b.addEventListener('click', a.onClick);
        return b;
      }),
    );
    requestAnimationFrame(() => card.classList.add('show'));
    clearTimeout(hideTimer);
    if (autoHide) hideTimer = setTimeout(hideToast, autoHide);
  }

  function hideToast() {
    root?.querySelector('.card').classList.remove('show');
    toastJobId = null;
  }

  function toastForJob(job) {
    const name = [job.title, job.artist].filter(Boolean).join(' · ');
    switch (job.status) {
      case 'done':
        return toast({ title: 'Saved ✓', sub: name, autoHide: 3500, actions: [{ label: 'Show in slothDJ', onClick: () => send({ type: 'show' }) }] });
      case 'failed':
        return toast({ tone: 'err', title: 'Download failed', sub: job.error || name, autoHide: 6000 });
      case 'canceled':
        return toast({ title: 'Canceled', sub: name, autoHide: 2500 });
      case 'downloading':
        return toast({
          title: job.kind !== 'track' && job.total ? `Downloading ${job.index} of ${job.total}` : 'Downloading…',
          sub: name,
          progress: job.progress,
          actions: [cancelAction(job)],
        });
      case 'converting':
        return toast({ title: 'Converting…', sub: name, progress: job.progress, actions: [cancelAction(job)] });
      default:
        return toast({ title: 'Sent to slothDJ', sub: name, progress: 0, actions: [cancelAction(job)] });
    }
  }

  function cancelAction(job) {
    return { label: 'Cancel', onClick: () => send({ type: 'cancel', id: job.id }) };
  }

  async function toastOffline(url) {
    const key = SDJ.formatKey(SDJ.classify(url)?.site);
    const format = (await chrome.storage.sync.get(key))[key];
    toast({
      tone: 'err',
      title: 'slothDJ isn’t running',
      sub: 'Open the app and this download starts right away.',
      actions: [
        {
          label: 'Open slothDJ',
          primary: true,
          onClick: () => {
            location.href = SDJ.deepLink(url, format);
            toast({ title: 'Opening slothDJ…', sub: 'Your download starts in the app.', autoHide: 4000 });
          },
        },
        { label: 'Get the app', onClick: () => window.open(`${SDJ.REPO}/releases/latest`, '_blank', 'noopener') },
      ],
    });
  }
})();
