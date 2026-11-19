// Writes latest.json (what installed copies of SlothDJ check for updates) once all
// platform builds have uploaded their signed update files to the draft release.
// Done in one place so parallel builds can't trip over each other updating it.
//
// Env: GITHUB_REPOSITORY, GITHUB_REF_NAME (the tag, e.g. v1.3.0), RELEASE_ID, GH_TOKEN
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const repo = process.env.GITHUB_REPOSITORY;
const tag = process.env.GITHUB_REF_NAME;
const id = process.env.RELEASE_ID;
const version = tag.replace(/^v/, '');
const gh = (...args) => execFileSync('gh', args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });

const assets = JSON.parse(gh('api', '--paginate', `repos/${repo}/releases/${id}/assets`));
const named = (re) => assets.find((a) => re.test(a.name));
const signatureOf = (asset) => {
  const sig = assets.find((a) => a.name === `${asset.name}.sig`);
  if (!sig) throw new Error(`No signature uploaded for ${asset.name}`);
  return gh('api', '-H', 'Accept: application/octet-stream', `repos/${repo}/releases/assets/${sig.id}`).trim();
};

// The updater looks for "<os>-<arch>-<installer>" first, then "<os>-<arch>".
const wanted = [
  { keys: ['windows-x86_64', 'windows-x86_64-nsis'], file: /_x64-setup\.exe$/ },
  { keys: ['darwin-aarch64', 'darwin-aarch64-app'], file: /_aarch64\.app\.tar\.gz$/ },
  { keys: ['darwin-x86_64', 'darwin-x86_64-app'], file: /_x64\.app\.tar\.gz$/ },
  { keys: ['linux-x86_64', 'linux-x86_64-appimage'], file: /_amd64\.AppImage$/ },
  { keys: ['linux-x86_64-deb'], file: /_amd64\.deb$/ },
];

const platforms = {};
for (const { keys, file } of wanted) {
  const asset = named(file);
  if (!asset) throw new Error(`Missing build for ${keys[0]} (${file})`);
  // Download links are only final once the release is published under its tag.
  const entry = {
    signature: signatureOf(asset),
    url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(asset.name)}`,
  };
  for (const k of keys) platforms[k] = entry;
}

const latest = { version, notes: `SlothDJ ${version}`, pub_date: new Date().toISOString(), platforms };
writeFileSync('latest.json', JSON.stringify(latest, null, 2));
console.log(`latest.json for ${version}: ${Object.keys(platforms).join(', ')}`);

// Replace any latest.json a previous attempt left on this release (DRY_RUN=1 only writes the file).
for (const old of process.env.DRY_RUN ? [] : assets.filter((a) => a.name === 'latest.json')) {
  gh('api', '-X', 'DELETE', `repos/${repo}/releases/assets/${old.id}`);
}
