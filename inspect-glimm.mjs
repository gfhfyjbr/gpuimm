import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
let playwright;
for (const candidate of ['playwright-core', ...process.env.PATH.split(path.delimiter).filter(p => p.endsWith('node_modules/.bin')).map(p => path.join(path.dirname(p), 'playwright-core'))]) {
  try { playwright = require(candidate); break; } catch {}
}
if (!playwright) throw new Error('Run with npm exec --yes --package=playwright-core -- node inspect-glimm.mjs');
const browser = await playwright.chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true, args: ['--enable-experimental-web-platform-features'] });
const routes = ['sheet', 'join', 'invoice', 'upgrade', 'reset', 'publish', 'autopilot', 'theme', 'tasks'];
const page = await browser.newPage({ viewport: { width: 1400, height: 1050 } });
try {
  await page.goto('https://glimm.dev/#demos');
  const iframe = page.locator('#demos iframe');
  const frame = await (await iframe.elementHandle()).contentFrame();
  const audit = [];
  for (const name of routes) {
    await frame.goto(`https://glimm.dev/demos/${name}`);
    await frame.waitForFunction(() => document.fonts.status === 'loaded');
    await page.waitForTimeout(120);
    const initial = await frame.evaluate(() => {
      const rect = e => {
        const r = e.getBoundingClientRect(), s = getComputedStyle(e);
        return { tag: e.tagName, text: e.tagName === 'TEXTAREA' ? e.value : e.innerText?.slice(0, 200), aria: e.getAttribute('aria-label'), x: r.x, y: r.y, w: r.width, h: r.height, display: s.display, position: s.position, fontFamily: s.fontFamily, fontSize: s.fontSize, weight: s.fontWeight, lineHeight: s.lineHeight, color: s.color, bg: s.backgroundColor, radius: s.borderRadius, shadow: s.boxShadow, html: e.outerHTML.slice(0, 2000) };
      };
      const style = document.querySelector('.theme-transit') ? getComputedStyle(document.querySelector('.theme-transit')) : getComputedStyle(document.documentElement);
      const vars = Object.fromEntries([...style].filter(k => k.startsWith('--color-t') || k.startsWith('--radius-t') || k.startsWith('--shadow-t')).map(k => [k, style.getPropertyValue(k)]));
      return { embedded: window.parent !== window, width: innerWidth, height: innerHeight, body: document.body.innerText, controls: [...document.querySelectorAll('button, textarea, input')].map(rect), canvases: [...document.querySelectorAll('canvas')].map(e => ({ ...rect(e), content: e.getAttribute('content'), parent: rect(e.parentElement) })), vars, scripts: [...document.scripts].map(s => s.src).filter(Boolean), stylesheets: [...document.styleSheets].map(s => s.href).filter(Boolean) };
    });
    await iframe.screenshot({ path: `glimm-original-${name}-initial.png` });
    await page.waitForTimeout(1200);
    const after = await frame.evaluate(() => ({ body: document.body.innerText, controls: [...document.querySelectorAll('button')].map(e => ({ text: e.innerText, aria: e.getAttribute('aria-label') })) }));
    await iframe.screenshot({ path: `glimm-original-${name}-auto.png` });
    audit.push({ name, initial, after });
    console.log(name, JSON.stringify({ viewport: [initial.width, initial.height], embedded: initial.embedded, controls: initial.controls.map(c => ({ text: c.text, aria: c.aria })), scopes: initial.canvases.filter(c => c.content === 'drawable').map(c => [c.x, c.y, c.w, c.h]), after: after.body.slice(0, 1100) }));
  }
  fs.writeFileSync('glimm-reference/embedded-audit.json', JSON.stringify(audit, null, 2));
} finally { await browser.close(); }
