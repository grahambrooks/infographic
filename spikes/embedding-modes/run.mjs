import { chromium, firefox, webkit } from 'playwright';
import crypto from 'node:crypto';
const h = b => crypto.createHash('sha1').update(b).digest('hex').slice(0,8);
const regions = { css:{x:0,y:0,width:100,height:100}, smil:{x:100,y:0,width:100,height:100},
                  hover:{x:200,y:0,width:100,height:100}, dark:{x:300,y:0,width:100,height:100} };
const shot = (p, r) => p.screenshot({ clip: regions[r] }).then(h);
for (const [name, bt] of Object.entries({ chromium, firefox, webkit })) {
  let browser;
  try { browser = await bt.launch(); } catch (e) { console.log(name, 'LAUNCH FAIL', e.message.split('\n')[0]); continue; }
  for (const page of ['img.html', 'inline.html']) {
    const out = {};
    for (const scheme of ['light', 'dark']) {
      const ctx = await browser.newContext({ colorScheme: scheme, viewport: { width: 400, height: 100 } });
      const p = await ctx.newPage();
      await p.goto('file://' + process.cwd() + '/' + page);
      await p.waitForTimeout(300);
      if (scheme === 'light') {
        const a1 = await shot(p,'css'), s1 = await shot(p,'smil');
        await p.waitForTimeout(700);
        const a2 = await shot(p,'css'), s2 = await shot(p,'smil');
        out.cssAnim = a1 !== a2; out.smilAnim = s1 !== s2;
        const hv0 = await shot(p,'hover');
        await p.mouse.move(250, 50); await p.waitForTimeout(200);
        out.hover = hv0 !== await shot(p,'hover');
      }
      out['dark_' + scheme] = await shot(p,'dark');
      await ctx.close();
    }
    out.darkMode = out.dark_light !== out.dark_dark; delete out.dark_light; delete out.dark_dark;
    console.log(name.padEnd(8), page.padEnd(12), JSON.stringify(out));
  }
  await browser.close();
}
