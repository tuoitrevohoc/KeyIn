// node poster.mjs -> out/poster.png: the hook frame with a play badge, for the README link.
import { chromium } from '/Users/banhtieu/Projects/CatSort/motion-studio/node_modules/playwright/index.mjs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
const browser = await chromium.launch({ args: ['--allow-file-access-from-files'] });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
await page.goto(pathToFileURL(join(here, 'film.html')).href);
await page.evaluate(() => window.ready);
await page.evaluate(() => {
  window.seek(2.7);
  const b = document.createElement('div');
  b.style.cssText = 'position:absolute;right:170px;top:400px;width:280px;height:280px;border-radius:50%;background:#007AFF;z-index:9;box-shadow:0 18px 50px rgba(0,60,140,.35)';
  b.innerHTML = '<svg viewBox="0 0 100 100" width="280" height="280"><polygon points="40,30 72,50 40,70" fill="#fff"/></svg>';
  document.getElementById('stage').appendChild(b);
  const l = document.createElement('div');
  l.style.cssText = 'position:absolute;right:120px;top:720px;width:380px;text-align:center;font:700 40px SFP;color:#141414;z-index:9';
  l.textContent = 'Video · 30 giây';
  document.getElementById('stage').appendChild(l);
});
await page.screenshot({ path: join(here, 'out', 'poster.png') });
await browser.close();
