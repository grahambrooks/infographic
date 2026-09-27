// Does a base64 @font-face inside the SVG apply when the SVG is shown via <img>?
// Compares the embedded-font render against the monospace fallback render.
import { chromium, firefox, webkit } from 'playwright';
import crypto from 'node:crypto';
const h = b => crypto.createHash('sha1').update(b).digest('hex').slice(0, 8);

import fs from 'node:fs';
const b64 = fs.readFileSync('../../assets/fonts/IBMPlexSans-SemiBold.ttf').toString('base64');
const svg = face => `<svg xmlns="http://www.w3.org/2000/svg" width="400" height="60" viewBox="0 0 400 60"><style>${face
  ? `@font-face{font-family:'Probe';src:url(data:font/ttf;base64,${b64}) format('truetype')}` : ''}</style>
<rect width="400" height="60" fill="#fff"/><text x="10" y="40" font-family="Probe, monospace" font-size="28">Circuit breaker</text></svg>`;
fs.mkdirSync('font', { recursive: true });
fs.writeFileSync('font/embedded.svg', svg(true));
fs.writeFileSync('font/fallback.svg', svg(false));
for (const n of ['embedded', 'fallback']) {
  fs.writeFileSync(`font/${n}-img.html`, `<!doctype html><body style="margin:0"><img src="${n}.svg" width="400" height="60"></body>`);
  fs.writeFileSync(`font/${n}-obj.html`, `<!doctype html><body style="margin:0"><object data="${n}.svg" type="image/svg+xml" width="400" height="60"></object></body>`);
}
for (const [name, bt] of Object.entries({ chromium, firefox, webkit })) {
  const browser = await bt.launch();
  const p = await browser.newPage({ viewport: { width: 400, height: 60 } });
  const r = {};
  for (const f of ['embedded-img', 'embedded-obj', 'fallback-img']) {
    await p.goto('file://' + process.cwd() + '/font/' + f + '.html');
    await p.waitForTimeout(500);
    r[f] = h(await p.screenshot());
  }
  console.log(name.padEnd(8), JSON.stringify({
    fontAppliesInImg: r['embedded-img'] !== r['fallback-img'],
    imgMatchesObject: r['embedded-img'] === r['embedded-obj'],
  }));
  await browser.close();
}
