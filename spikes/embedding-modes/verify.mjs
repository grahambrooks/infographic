// Checks a built infographic in real browser engines:
//  1. it animates when shown as <img> (the GitHub README case);
//  2. at every storyboard frame's time, the browser shows exactly the
//     elements the timeline says are visible, with tokens where the timeline
//     puts them.
//
// Usage: node verify.mjs <out-dir> <slug>
//   (after `infographic build <spec> --frames`)
import { chromium, firefox, webkit } from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

const [dir, slug] = process.argv.slice(2);
if (!dir || !slug) {
  console.error('usage: node verify.mjs <out-dir> <slug>');
  process.exit(2);
}
const hash = b => crypto.createHash('sha1').update(b).digest('hex');
const TOKEN_TOLERANCE = 1.5; // px

let failures = 0;
for (const theme of ['light', 'dark']) {
  const svgFile = path.resolve(dir, theme === 'light' ? `${slug}.svg` : `${slug}-${theme}.svg`);
  const svgText = fs.readFileSync(svgFile, 'utf8');
  const [, w, h] = svgText.match(/width="([\d.]+)" height="([\d.]+)"/);
  const frameDir = path.resolve(dir, `${slug}.frames`);
  const frames = fs.readdirSync(frameDir).filter(f => f.startsWith(`${theme}-`)).sort().map(f => {
    const text = fs.readFileSync(path.join(frameDir, f), 'utf8');
    const t = Number(text.match(/data-t="([\d.]+)"/)[1]);
    const ids = new Set([...text.matchAll(/ id="([^"]+)"/g)].map(m => m[1]));
    const tokens = Object.fromEntries(
      [...text.matchAll(/<circle id="([^"]+)" data-role="token" cx="([-\d.]+)" cy="([-\d.]+)"/g)].map(m => [m[1], [Number(m[2]), Number(m[3])]]));
    return { name: f, t, ids, tokens };
  });

  for (const [engine, bt] of Object.entries({ chromium, firefox, webkit })) {
    const browser = await bt.launch();
    const page = await browser.newPage({ viewport: { width: Math.ceil(+w), height: Math.ceil(+h) } });

    // 1. Plays inside <img>: poll until the picture changes, for up to one
    // cycle, so a spec that opens with a still phase is not reported frozen.
    const cycle = Number(svgText.match(/data-cycle="([\d.]+)"/)?.[1] ?? 10);
    const html = path.join(dir, `.verify-${theme}-${engine}-${process.pid}.html`);
    fs.writeFileSync(html, `<!doctype html><body style="margin:0"><img src="${path.basename(svgFile)}"></body>`);
    await page.goto('file://' + path.resolve(html));
    await page.waitForTimeout(300);
    const first = hash(await page.screenshot());
    let plays = false;
    for (let waited = 0; waited < cycle * 1000 && !plays; waited += 400) {
      await page.waitForTimeout(400);
      plays = hash(await page.screenshot()) !== first;
    }
    fs.rmSync(html, { force: true });

    // 2. Seek inline SVG to each frame time and compare with the timeline.
    await page.setContent(`<!doctype html><body style="margin:0">${svgText}</body>`);
    const mismatches = [];
    for (const fr of frames) {
      const seen = await page.evaluate(t => {
        const svg = document.querySelector('svg');
        svg.pauseAnimations();
        svg.setCurrentTime(t);
        const out = { visible: {}, tokens: {} };
        for (const an of svg.querySelectorAll('animate[attributeName="visibility"]')) {
          const el = an.parentElement;
          out.visible[el.id] = getComputedStyle(el).visibility === 'visible';
        }
        for (const c of svg.querySelectorAll('circle[data-role="token"]')) {
          if (getComputedStyle(c).visibility !== 'visible') continue;
          const r = c.getBoundingClientRect();
          out.tokens[c.id] = [r.x + r.width / 2, r.y + r.height / 2];
        }
        return out;
      }, fr.t);
      for (const [id, vis] of Object.entries(seen.visible)) {
        if (vis !== fr.ids.has(id)) mismatches.push(`${fr.name} t=${fr.t}: ${id} is ${vis ? 'visible' : 'hidden'}, timeline says ${vis ? 'hidden' : 'visible'}`);
      }
      for (const [id, [x, y]] of Object.entries(fr.tokens)) {
        const got = seen.tokens[id];
        if (!got) continue; // visibility mismatch already reported
        const d = Math.hypot(got[0] - x, got[1] - y);
        if (d > TOKEN_TOLERANCE) mismatches.push(`${fr.name} t=${fr.t}: token ${id} is ${d.toFixed(1)}px from where the timeline puts it`);
      }
    }
    await browser.close();
    const ok = plays && mismatches.length === 0;
    if (!ok) failures++;
    console.log(`${theme.padEnd(5)} ${engine.padEnd(8)} playsInImg=${plays} frames=${frames.length} mismatches=${mismatches.length}`);
    for (const m of mismatches.slice(0, 10)) console.log('   ', m);
  }
}
process.exit(failures ? 1 : 0);
