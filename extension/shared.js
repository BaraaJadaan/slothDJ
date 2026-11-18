// Shared by the popup, the background worker and the content scripts.
/* exported SDJ */
var SDJ = (() => {
  const RESERVED = new Set([
    'discover', 'stream', 'search', 'upload', 'you', 'settings', 'messages', 'notifications', 'charts',
    'pages', 'terms-of-use', 'imprint', 'mobile', 'jobs', 'creators', 'popular', 'feed', 'people', 'tags',
    'connect', 'logout', 'signin', 'signup', 'pro', 'artists', 'go', 'home', 'for-artists',
  ]);
  const PROFILE_TABS = new Set(['tracks', 'albums', 'sets', 'reposts', 'likes', 'popular-tracks', 'spotlight', 'toptracks']);
  const YT_HOSTS = new Set(['youtube.com', 'music.youtube.com', 'youtu.be', 'youtube-nocookie.com']);

  function soundcloud(u, host) {
    if (host === 'on.soundcloud.com') return { url: u.origin + u.pathname, kind: 'track', site: 'soundcloud' };
    const segs = u.pathname.split('/').filter(Boolean);
    if (!segs.length) return null;
    const first = segs[0].toLowerCase();
    if (RESERVED.has(first) && !(first === 'discover' && segs[1] === 'sets' && segs.length >= 3)) return null;
    let kind = 'track';
    if (segs.length === 1 || (segs.length === 2 && PROFILE_TABS.has(segs[1]))) kind = 'profile';
    else if (segs[1] === 'sets') kind = 'set';
    return { url: `https://soundcloud.com/${segs.slice(0, 4).join('/')}`, kind, site: 'soundcloud' };
  }

  function youtube(u, host) {
    const segs = u.pathname.split('/').filter(Boolean);
    const video = (id) =>
      /^[\w-]{11}$/.test(id || '') ? { url: `https://www.youtube.com/watch?v=${id}`, kind: 'track', site: 'youtube' } : null;
    if (host === 'youtu.be') return video(segs[0]);
    if (segs[0] === 'watch') return video(u.searchParams.get('v'));
    if (['shorts', 'live', 'embed', 'v'].includes(segs[0])) return video(segs[1]);
    if (segs[0] === 'playlist') {
      const list = u.searchParams.get('list');
      if (!list || ['WL', 'LL', 'LM'].includes(list) || !/^[\w-]{1,64}$/.test(list)) return null;
      return { url: `https://www.youtube.com/playlist?list=${list}`, kind: 'set', site: 'youtube' };
    }
    return null;
  }

  /** Is this a page slothDJ can download? Returns { url, kind, site } or null. */
  function classify(href) {
    let u;
    try {
      u = new URL(href);
    } catch {
      return null;
    }
    const host = u.hostname.replace(/^(www|m)\./, '');
    if (YT_HOSTS.has(host)) return youtube(u, host);
    if (host === 'soundcloud.com' || host === 'on.soundcloud.com') return soundcloud(u, host);
    return null;
  }

  function kindLabel(t) {
    if (t.kind === 'set') return t.site === 'youtube' ? 'Playlist' : 'Set';
    if (t.kind === 'profile') return 'Profile';
    return t.site === 'youtube' ? 'Video' : 'Track';
  }

  const SITE_NAME = { soundcloud: 'SoundCloud', youtube: 'YouTube' };
  const REPO = 'https://github.com/BaraaJadaan/slothDJ';
  /** chrome.storage key for the remembered quality: SoundCloud is audio-only, YouTube can be video. */
  const formatKey = (site) => (site === 'youtube' ? 'ytFormat' : 'format');

  function deepLink(url, format) {
    const q = new URLSearchParams({ url });
    if (format) q.set('format', format);
    return `slothdj://download?${q}`;
  }

  return { classify, kindLabel, SITE_NAME, REPO, formatKey, deepLink };
})();
