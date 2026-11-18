// Shared by the popup, the background worker and the SoundCloud content script.
/* exported SDJ */
var SDJ = (() => {
  const RESERVED = new Set([
    'discover', 'stream', 'search', 'upload', 'you', 'settings', 'messages', 'notifications', 'charts',
    'pages', 'terms-of-use', 'imprint', 'mobile', 'jobs', 'creators', 'popular', 'feed', 'people', 'tags',
    'connect', 'logout', 'signin', 'signup', 'pro', 'artists', 'go', 'home', 'for-artists',
  ]);
  const PROFILE_TABS = new Set(['tracks', 'albums', 'sets', 'reposts', 'likes', 'popular-tracks', 'spotlight', 'toptracks']);

  /** Is this a SoundCloud page slothDJ can download? Returns { url, kind } or null. */
  function classify(href) {
    let u;
    try {
      u = new URL(href);
    } catch {
      return null;
    }
    const host = u.hostname.replace(/^(www|m)\./, '');
    if (host === 'on.soundcloud.com') return { url: u.origin + u.pathname, kind: 'track' };
    if (host !== 'soundcloud.com') return null;
    const segs = u.pathname.split('/').filter(Boolean);
    if (!segs.length) return null;
    const first = segs[0].toLowerCase();
    if (RESERVED.has(first) && !(first === 'discover' && segs[1] === 'sets' && segs.length >= 3)) return null;
    let kind = 'track';
    if (segs.length === 1 || (segs.length === 2 && PROFILE_TABS.has(segs[1]))) kind = 'profile';
    else if (segs[1] === 'sets') kind = 'set';
    return { url: `https://soundcloud.com/${segs.slice(0, 4).join('/')}`, kind };
  }

  const KIND_LABEL = { track: 'Track', set: 'Set', profile: 'Profile' };
  const REPO = 'https://github.com/BaraaJadaan/slothDJ';

  function deepLink(url, format) {
    const q = new URLSearchParams({ url });
    if (format) q.set('format', format);
    return `slothdj://download?${q}`;
  }

  return { classify, KIND_LABEL, REPO, deepLink };
})();
