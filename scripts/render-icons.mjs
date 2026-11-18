// Renders brand/logo.svg into every PNG the app, extension and README need.
// Usage: node scripts/render-icons.mjs   (then `npx tauri icon brand/icon-1024.png` for app icons)
import { Resvg } from '@resvg/resvg-js';
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const svg = readFileSync(join(root, 'brand/logo.svg'), 'utf8');

function render(size, out) {
  const png = new Resvg(svg, { fitTo: { mode: 'width', value: size } }).render().asPng();
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, png);
  console.log('wrote', out.replace(root, '.'));
}

render(1024, join(root, 'brand/icon-1024.png'));
render(256, join(root, 'brand/icon-256.png'));
for (const s of [16, 32, 48, 128]) render(s, join(root, `extension/icons/icon-${s}.png`));

// The UI and extension popup use the vector logo directly.
copyFileSync(join(root, 'brand/logo.svg'), join(root, 'app/ui/logo.svg'));
copyFileSync(join(root, 'brand/logo.svg'), join(root, 'extension/icons/logo.svg'));
