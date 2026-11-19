// SlothDJ page script: a SlothDJ button with a quality picker on SoundCloud and YouTube,
// and a small card that follows the download.
(() => {
  const OWNER = chrome.runtime.id;
  if (window.__sdj3) return;
  window.__sdj3 = true;
  // This copy's mark on everything it adds. After an extension update the new copy runs
  // next to the old one (which is cut off from the extension), and they must tell their
  // buttons apart.
  const INST = Math.random().toString(36).slice(2);
  const RETIRE = `slothdj-retire:${OWNER}`;
  // Ask a cut-off copy from before an update to take its buttons away; this one takes over.
  document.dispatchEvent(new CustomEvent(RETIRE));
  document.addEventListener(RETIRE, () => alive() || teardown());

  const svg = (body, view = 16) =>
    `<svg viewBox="0 0 ${view} ${view}" xmlns="http://www.w3.org/2000/svg" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
  const ICON = {
    idle: svg('<path d="M8 1.8v8M4.7 6.6 8 9.9l3.3-3.3"/><path d="M2.6 11.4v1.3c0 .7.6 1.3 1.3 1.3h8.2c.7 0 1.3-.6 1.3-1.3v-1.3"/>'),
    busy: svg('<circle cx="8" cy="8" r="5.6" stroke-opacity=".25"/><path d="M13.6 8A5.6 5.6 0 0 0 8 2.4"/>'),
    done: svg('<path d="M3 8.6 6.4 12 13 4.6"/>'),
    error: svg('<circle cx="8" cy="8" r="6"/><path d="M8 4.9v3.5M8 11.1v.05"/>'),
  };
  const CARET = svg('<path d="m4.5 6.5 3.5 3.5 3.5-3.5"/>');
  const RUNNING = ['sending', 'queued', 'starting', 'downloading', 'converting'];
  const FINISHED = ['done', 'failed', 'canceled'];
  const site = /(^|\.)youtube\.com$/.test(location.hostname) ? 'youtube' : 'soundcloud';
  const keyOf = (url) => SDJ.classify(url)?.url;
  const jobs = new Map(); // link -> latest job (or a pseudo-state while sending)
  const polling = new Map(); // job id -> interval
  let root; // card shadow root
  let toastJobId = null;
  let hideTimer;
  let appDefault = null;
  let format = 'mp3_320';

  // ---------- Talking to the extension ----------
  /** False once the extension was reloaded or updated: this page's copy of the script is cut off. */
  const alive = () => {
    try {
      return !!chrome.runtime?.id;
    } catch {
      return false;
    }
  };
  async function send(msg) {
    if (!alive()) return { orphaned: true };
    try {
      return (await chrome.runtime.sendMessage(msg)) ?? { offline: true };
    } catch {
      return alive() ? { offline: true } : { orphaned: true };
    }
  }

  // ---------- Quality ----------
  const formatShort = (id) => SDJ.formatInfo(id)?.short ?? 'MP3 320';
  async function refreshFormat() {
    if (!alive()) return;
    format = await SDJ.chosenFormat(site, appDefault);
    document.querySelectorAll(`.sdjx[data-sdj-owner="${OWNER}"]`).forEach((el) => paint(el, jobs.get(el.dataset.key)));
  }
  send({ type: 'status' }).then((s) => {
    if (s?.defaultFormat) appDefault = s.defaultFormat;
    refreshFormat();
  });
  // A quality picked in the popup (or another tab) shows up here right away.
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === 'sync' && changes[SDJ.formatKey(site)]) refreshFormat();
  });

  // ---------- Buttons ----------
  function mark(el) {
    el.dataset.sdjOwner = OWNER;
    el.dataset.sdjInst = INST;
    return el;
  }
  /** A button left by this extension's copy from before an update. */
  const stale = (el) => el.dataset.sdjOwner === OWNER && el.dataset.sdjInst !== INST;

  function stopEvent(e) {
    e.preventDefault();
    e.stopPropagation();
  }

  /** Labelled button with a quality menu: SoundCloud track/set pages and under YouTube videos. */
  function makeSplit(url, variant) {
    const wrap = document.createElement('span');
    wrap.className = `sdjx sdjx-split sdjx-${variant}`;
    mark(wrap);
    const sc = variant === 'sc' ? 'sc-button-secondary sc-button sc-button-medium ' : '';
    wrap.innerHTML = `
      <button type="button" class="${sc}sdjx-main"><span class="sdjx-ico"></span><span class="sdjx-name">SlothDJ</span><span class="sdjx-fmt"></span></button>
      <button type="button" class="${sc}sdjx-caret" title="Choose quality" aria-label="Choose quality" aria-haspopup="menu">${CARET}</button>`;
    wrap.querySelector('.sdjx-main').addEventListener('click', (e) => {
      stopEvent(e);
      start(wrap.dataset.url);
    });
    wrap.querySelector('.sdjx-caret').addEventListener('click', (e) => {
      stopEvent(e);
      toggleMenu(wrap);
    });
    point(wrap, url);
    return wrap;
  }

  /** Compact icon button for every track in SoundCloud lists. */
  function makeMini(url, size) {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = `sc-button-secondary sc-button sc-button-${size} sc-button-icon sc-button-responsive sdjx sdjx-mini`;
    mark(b);
    b.innerHTML = `<div class="sdjx-ico"></div><span class="sc-button-label sdjx-name"></span>`;
    b.addEventListener('click', (e) => {
      stopEvent(e);
      start(b.dataset.url);
    });
    point(b, url);
    return b;
  }

  /** Aim a button at a (new) link, e.g. after YouTube moves to another video. */
  function point(el, url) {
    el.dataset.url = url;
    el.dataset.key = keyOf(url);
    paint(el, jobs.get(el.dataset.key));
  }

  function paint(el, job) {
    let state = 'idle';
    let label = '';
    let title = `Download with SlothDJ as ${SDJ.formatInfo(format)?.label ?? 'MP3'}`;
    if (job) {
      const video = job.format?.startsWith('mp4_');
      switch (job.status) {
        case 'sending': state = 'busy'; label = 'Sending…'; title = 'Sending to SlothDJ…'; break;
        case 'queued':
        case 'starting': state = 'busy'; label = 'Queued'; title = 'Queued in SlothDJ'; break;
        case 'downloading': state = 'busy'; label = `${Math.round((job.progress || 0) * 100)}%`; title = `Downloading: ${job.title}`; break;
        case 'converting': state = 'busy'; label = video ? 'Merging…' : 'Converting…'; title = 'Finishing up…'; break;
        case 'done': state = 'done'; label = 'Saved'; title = `Saved by SlothDJ: ${job.title}`; break;
        case 'failed': state = 'error'; label = 'Failed'; title = job.error || 'Download failed'; break;
        default: break;
      }
    }
    if (el.dataset.state !== state) {
      el.dataset.state = state;
      el.querySelector('.sdjx-ico').innerHTML = ICON[state];
    }
    const name = el.querySelector('.sdjx-name');
    if (el.classList.contains('sdjx-mini')) {
      name.textContent = state === 'busy' ? label : 'SlothDJ';
      name.classList.toggle('sc-visuallyhidden', state !== 'busy');
      el.classList.toggle('sdjx-wide', state === 'busy');
    } else {
      name.textContent = label || 'SlothDJ';
      const fmt = el.querySelector('.sdjx-fmt');
      fmt.textContent = formatShort(format);
      fmt.hidden = state === 'busy';
    }
    el.title = title;
    el.querySelector('.sdjx-main, .sdjx-name')?.setAttribute('aria-label', title);
  }

  // SoundCloud: a labelled button on track/set pages, a compact one on every listed track.
  function soundUrl(group) {
    if (group.closest('.listenEngagement, .l-listen-hero, .fullHero')) return location.origin + location.pathname;
    const item = group.closest('.trackItem, .sound, .soundBadge, .searchItem, .soundList__item, .compactTrackListItem');
    const a = item?.querySelector('a.trackItem__trackTitle, a.soundTitle__title, a.sc-link-primary[href]');
    return a ? new URL(a.getAttribute('href'), location.origin).href : null;
  }

  function scanSoundCloud() {
    document.querySelectorAll('.soundActions .sc-button-group').forEach((group) => {
      const existing = group.querySelector(':scope > .sdjx');
      if (existing && !stale(existing)) return;
      existing?.remove();
      const url = soundUrl(group);
      if (!url || !SDJ.classify(url)) return;
      if (group.closest('.listenEngagement')) {
        group.appendChild(makeSplit(url, 'sc'));
      } else {
        const size = group.classList.contains('sc-button-group-small') ? 'small' : 'medium';
        group.appendChild(makeMini(url, size));
      }
    });
  }

  // YouTube: one button under the video, next to Like and Share. YouTube is a single-page
  // app, so the same button is re-aimed when you open another video.
  function scanYouTube() {
    const target = SDJ.classify(location.href);
    const bar = document.querySelector('ytd-watch-metadata #top-level-buttons-computed');
    if (!target || target.kind !== 'track' || !bar) return;
    let el = bar.querySelector(':scope > .sdjx');
    if (el && stale(el)) {
      el.remove();
      el = null;
    }
    if (!el) {
      document.querySelectorAll(`.sdjx-yt[data-sdj-owner="${OWNER}"]`).forEach((n) => n.remove());
      bar.appendChild(makeSplit(target.url, 'yt'));
    } else if (el.dataset.key !== target.url) {
      point(el, target.url);
      closeMenu();
    }
  }

  let retired = false;
  // A copy cut off by an update adds no new buttons; the ones it has say "reload the page".
  const scan = () => !retired && alive() && (site === 'youtube' ? scanYouTube : scanSoundCloud)();
  let scanTimer;
  const observer = new MutationObserver(() => {
    clearTimeout(scanTimer);
    scanTimer = setTimeout(scan, 250);
  });
  observer.observe(document.body, { childList: true, subtree: true });
  document.addEventListener('yt-navigate-finish', () => setTimeout(scan, 50));
  scan();
  send({ type: 'hello' });

  /** Stand down for the copy that came with an extension update. */
  function teardown() {
    if (retired) return;
    retired = true;
    observer.disconnect();
    clearTimeout(scanTimer);
    polling.forEach((timer) => clearInterval(timer));
    polling.clear();
    closeMenu();
    document.querySelectorAll(`[data-sdj-inst="${INST}"]`).forEach((n) => n.remove());
  }

  // An older SlothDJ (or a copy cut off by an extension update) is also on this page.
  setTimeout(() => {
    const other = document.querySelector(`.sdj-btn, .sdj-yt-btn, #slothdj-toast-host, [data-sdj-owner]:not([data-sdj-owner="${OWNER}"])`);
    if (retired || !other || sessionStorage.getItem('sdj-dup-warned')) return;
    sessionStorage.setItem('sdj-dup-warned', '1');
    toast({
      tone: 'err',
      title: 'Two SlothDJ buttons on this page',
      sub: 'Reload the page. If two stay, remove the older SlothDJ on your browser’s extensions page.',
      actions: [{ label: 'Reload page', primary: true, onClick: () => location.reload() }],
    });
  }, 3000);

  // ---------- Quality menu ----------
  let menu;
  function closeMenu() {
    menu?.host.remove();
    menu = null;
  }
  function toggleMenu(anchor) {
    if (menu?.anchor === anchor) return closeMenu();
    closeMenu();
    const hostEl = mark(document.createElement('div'));
    const sh = hostEl.attachShadow({ mode: 'open' });
    const group = (title, list) =>
      list.length
        ? `<div class="h">${title}</div>${list
            .map((f) => `<button data-id="${f.id}" class="${f.id === format ? 'on' : ''}"><span class="c">${f.id === format ? '✓' : ''}</span>${f.label}</button>`)
            .join('')}`
        : '';
    const formats = SDJ.formatsFor(site);
    sh.innerHTML = `
      <style>
        :host { all: initial; }
        .m { position: fixed; z-index: 2147483647; min-width: 220px; padding: 6px; border-radius: 12px; background: #18131f; color: #f4f1fa;
          font: 13px/1.4 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; border: 1px solid rgba(255,255,255,.12);
          box-shadow: 0 18px 40px -12px rgba(0,0,0,.6); }
        .t { padding: 6px 10px 4px; font-weight: 650; }
        .h { padding: 8px 10px 2px; font-size: 10.5px; letter-spacing: .08em; text-transform: uppercase; color: #6f6884; }
        button { all: unset; box-sizing: border-box; display: flex; align-items: center; gap: 8px; width: 100%; padding: 7px 10px; border-radius: 8px; cursor: pointer; }
        button:hover, button:focus-visible { background: rgba(255,255,255,.08); }
        button.on { color: #f472b6; font-weight: 600; }
        .c { width: 14px; color: #f472b6; }
        .n { padding: 6px 10px 4px; font-size: 11.5px; color: #6f6884; border-top: 1px solid rgba(255,255,255,.08); margin-top: 4px; }
      </style>
      <div class="m" role="menu">
        <div class="t">Download as…</div>
        ${group('Audio', formats.filter((f) => !f.video))}
        ${group('Video', formats.filter((f) => f.video))}
        <div class="n">Remembered for ${SDJ.SITE_NAME[site]}. You can also change it in the SlothDJ popup.</div>
      </div>`;
    document.documentElement.appendChild(hostEl);
    const box = sh.querySelector('.m');
    const r = anchor.getBoundingClientRect();
    const width = box.offsetWidth;
    box.style.left = `${Math.max(8, Math.min(r.right - width, innerWidth - width - 8))}px`;
    const below = r.bottom + 6;
    box.style.top = below + box.offsetHeight < innerHeight ? `${below}px` : `${Math.max(8, r.top - box.offsetHeight - 6)}px`;
    sh.querySelectorAll('button[data-id]').forEach((b) =>
      b.addEventListener('click', async (e) => {
        stopEvent(e);
        const id = b.dataset.id;
        closeMenu();
        if (!alive()) return toastReload();
        await SDJ.rememberFormat(site, id);
        format = id;
        start(anchor.dataset.url, id);
      }),
    );
    menu = { host: hostEl, anchor };
  }
  document.addEventListener('click', (e) => menu && !menu.host.contains(e.target) && closeMenu(), true);
  document.addEventListener('keydown', (e) => e.key === 'Escape' && closeMenu());
  addEventListener('scroll', closeMenu, { passive: true });
  addEventListener('resize', closeMenu);

  // ---------- Downloads ----------
  function setJob(key, job) {
    if (job) jobs.set(key, job);
    else jobs.delete(key);
    document.querySelectorAll(`.sdjx[data-sdj-owner="${OWNER}"][data-key="${CSS.escape(key)}"]`).forEach((el) => paint(el, job));
    if (job?.id && toastJobId === job.id) toastForJob(job);
  }

  async function start(url, fmt = format) {
    const key = keyOf(url);
    const current = jobs.get(key);
    if (current && RUNNING.includes(current.status) && current.id) {
      toastJobId = current.id;
      return toastForJob(current);
    }
    setJob(key, { status: 'sending', format: fmt });
    handleResult(url, await send({ type: 'download', url, format: fmt }), fmt);
  }

  function follow(key, job) {
    setJob(key, job);
    if (polling.has(job.id)) return;
    let misses = 0;
    const stop = () => {
      clearInterval(polling.get(job.id));
      polling.delete(job.id);
    };
    polling.set(
      job.id,
      setInterval(async () => {
        const res = await send({ type: 'job', id: job.id });
        if (res?.orphaned) {
          stop();
          setJob(key, null);
          if (toastJobId === job.id) toastReload();
          return;
        }
        if (res?.status === 404) {
          // Removed in the app: nothing left to follow.
          stop();
          setJob(key, null);
          if (toastJobId === job.id) hideToast();
          return;
        }
        if (!res?.ok) {
          if (++misses >= 5) {
            stop();
            setJob(key, null);
            if (toastJobId === job.id) toast({ tone: 'err', title: 'Lost contact with SlothDJ', sub: 'Check this download in the app.', autoHide: 5000 });
          }
          return;
        }
        misses = 0;
        setJob(key, res.job);
        if (FINISHED.includes(res.job.status)) stop();
      }, 1000),
    );
  }

  function handleResult(url, res, fmt) {
    const key = keyOf(url);
    if (res?.ok && res.job) {
      toastJobId = res.job.id;
      follow(key, res.job);
      toastForJob(res.job);
    } else if (res?.orphaned) {
      setJob(key, null);
      toastReload();
    } else if (res?.offline) {
      setJob(key, null);
      toastOffline(url, fmt);
    } else {
      setJob(key, { status: 'failed', error: res?.error });
      toast({ tone: 'err', title: 'Couldn’t add that', sub: res?.error || 'Something went wrong.', autoHide: 6000 });
    }
  }

  // Results of the right-click menu, so the page shows feedback too.
  chrome.runtime.onMessage.addListener((msg) => {
    if (msg?.type === 'sent') handleResult(msg.url, msg.res);
  });

  // ---------- Card (shadow DOM, so the site's CSS can't touch it) ----------
  function host() {
    if (root) return root;
    const el = mark(document.createElement('div'));
    el.id = 'slothdj-card';
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
        .x { all: unset; cursor: pointer; color: #a39cb5; font-size: 20px; line-height: 1; text-align: center; border-radius: 6px; }
        .x:hover { color: #f4f1fa; background: rgba(255,255,255,.08); }
        .bar { grid-column: 2 / 4; height: 4px; border-radius: 99px; background: rgba(255,255,255,.08); overflow: hidden; }
        .fill { height: 100%; width: 0; background: linear-gradient(90deg, #8b5cf6, #ec4899); border-radius: 99px; transition: width .3s; }
        .acts { grid-column: 2 / 4; display: flex; gap: 8px; }
        .acts button { all: unset; cursor: pointer; padding: 6px 12px; border-radius: 9px; font-weight: 600; font-size: 12.5px;
          border: 1px solid rgba(255,255,255,.14); color: #f4f1fa; }
        .acts button.primary { border: 0; color: #fff; background: linear-gradient(135deg, #8b5cf6, #ec4899); }
        .acts button:hover { filter: brightness(1.1); }
        .acts button:disabled { opacity: .5; cursor: default; }
        [hidden] { display: none !important; }
      </style>
      <div class="card" role="status">
        <img alt="" src="${chrome.runtime.getURL('icons/logo.svg')}">
        <div><div class="t" dir="auto"></div><div class="s" dir="auto"></div></div>
        <button class="x" aria-label="Close" title="Close">×</button>
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

  /** Closing the card stops it from popping up again for that download. */
  function hideToast() {
    root?.querySelector('.card').classList.remove('show');
    toastJobId = null;
  }

  function toastForJob(job) {
    const name = [job.title, job.artist].filter(Boolean).join(' · ');
    const as = job.format ? ` · ${formatShort(job.format)}` : '';
    switch (job.status) {
      case 'done':
        return toast({ title: 'Saved ✓', sub: name, autoHide: 3500, actions: [{ label: 'Show in SlothDJ', onClick: () => send({ type: 'show' }) }] });
      case 'failed':
        return toast({ tone: 'err', title: 'Download failed', sub: job.error || name, autoHide: 6000 });
      case 'canceled':
        return toast({ title: 'Canceled', sub: name, autoHide: 2500 });
      case 'downloading':
        return toast({
          title: (job.kind !== 'track' && job.total ? `Downloading ${job.index} of ${job.total}` : 'Downloading…') + as,
          sub: name,
          progress: job.progress,
          actions: [cancelAction(job)],
        });
      case 'converting':
        return toast({ title: `${job.format?.startsWith('mp4_') ? 'Merging video and sound…' : 'Converting…'}`, sub: name, progress: job.progress, actions: [cancelAction(job)] });
      default:
        return toast({ title: `Sent to SlothDJ${as}`, sub: name, progress: 0, actions: [cancelAction(job)] });
    }
  }

  function cancelAction(job) {
    return {
      label: 'Cancel',
      onClick: async (e) => {
        e.currentTarget.disabled = true;
        e.currentTarget.textContent = 'Canceling…';
        const res = await send({ type: 'cancel', id: job.id });
        if (res?.orphaned) toastReload();
        else if (!res?.ok) toast({ tone: 'err', title: 'Couldn’t reach SlothDJ', sub: 'Cancel it in the app instead.', autoHide: 5000 });
        // Otherwise the next status check shows "Canceled".
      },
    };
  }

  function toastReload() {
    toast({
      tone: 'err',
      title: 'SlothDJ was updated',
      sub: 'Reload this page to keep using the SlothDJ button. Your downloads continue in the app.',
      actions: [{ label: 'Reload page', primary: true, onClick: () => location.reload() }],
    });
  }

  async function toastOffline(url, fmt) {
    toast({
      tone: 'err',
      title: 'SlothDJ isn’t running',
      sub: 'Open the app and this download starts right away.',
      actions: [
        {
          label: 'Open SlothDJ',
          primary: true,
          onClick: () => {
            location.href = SDJ.deepLink(url, fmt || format);
            toast({ title: 'Opening SlothDJ…', sub: 'Your download starts in the app.', autoHide: 4000 });
          },
        },
        { label: 'Get the app', onClick: () => window.open(`${SDJ.REPO}/releases/latest`, '_blank', 'noopener') },
      ],
    });
  }
})();
