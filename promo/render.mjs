// node render.mjs sheet [offset]   one frame per beat -> qa/sheet.png
// node render.mjs at 1.2 3.4 ...    single frames -> qa/at_<t>.png
// node render.mjs full              every frame, muxed with out/score.wav -> out/KeyIn_Promo_1920x1080.mp4
// node render.mjs gif               the full render as a 960px 15 fps loop -> out/KeyIn_Promo.gif (README)
import { chromium } from '/Users/banhtieu/Projects/CatSort/motion-studio/node_modules/playwright/index.mjs';
import { spawn, execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
new Function(readFileSync(join(here, 'edl.js'), 'utf8'))();
const EDL = globalThis.EDL, [W, H] = EDL.size;
const [mode = 'sheet', ...rest] = process.argv.slice(2);
mkdirSync(join(here, 'qa'), { recursive: true });
mkdirSync(join(here, 'out'), { recursive: true });

const browser = await chromium.launch({ args: ['--allow-file-access-from-files'] });
const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
page.on('pageerror', e => console.error('pageerror', e.message));
page.on('console', m => { if (m.type() === 'error') console.error('console', m.text()); });
await page.goto(pathToFileURL(join(here, 'film.html')).href);
await page.evaluate(() => window.ready);
const frame = async t => { await page.evaluate(t => window.seek(t), t); return page.screenshot({ type: 'png' }); };

if (mode === 'sheet') {
  const off = Number(rest[0] ?? 0.12), beats = Math.round(EDL.duration * EDL.bpm / 60);
  const dir = join(here, 'qa', 'beats');
  mkdirSync(dir, { recursive: true });
  for (let k = 0; k < beats; k++) writeFileSync(join(dir, `${String(k).padStart(2, '0')}.png`), await frame(+(k * 60 / EDL.bpm + off).toFixed(3)));
  execFileSync('ffmpeg', ['-v', 'error', '-y', '-framerate', '1', '-i', join(dir, '%02d.png'),
    '-vf', 'scale=316:-2,pad=320:182:2:2:white,tile=6x10', '-frames:v', '1', join(here, 'qa', 'sheet.png')]);
  console.log('sheet -> qa/sheet.png');
} else if (mode === 'at') {
  for (const s of rest) { writeFileSync(join(here, 'qa', `at_${s}.png`), await frame(Number(s))); console.log(`qa/at_${s}.png`); }
} else if (mode === 'gif') {
  execFileSync('ffmpeg', ['-v', 'error', '-y', '-i', join(here, 'out', `KeyIn_Promo_${W}x${H}.mp4`), '-filter_complex',
    'fps=15,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff:max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle',
    '-loop', '0', join(here, 'out', 'KeyIn_Promo.gif')]);
  console.log('gif -> out/KeyIn_Promo.gif');
} else if (mode === 'full') {
  const out = join(here, 'out', 'video_noaudio.mp4');
  const ff = spawn('ffmpeg', ['-v', 'error', '-y', '-f', 'image2pipe', '-framerate', String(EDL.fps), '-i', '-',
    '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-profile:v', 'high', '-pix_fmt', 'yuv420p', '-r', String(EDL.fps), '-movflags', '+faststart', out],
    { stdio: ['pipe', 'inherit', 'inherit'] });
  const n = Math.round(EDL.duration * EDL.fps), t0 = Date.now();
  for (let f = 0; f < n; f++) {
    const png = await frame(f / EDL.fps);
    if (!ff.stdin.write(png)) await new Promise(r => ff.stdin.once('drain', r));
    if (f % 90 === 0) console.log(`frame ${f}/${n}  ${((Date.now() - t0) / 1000).toFixed(0)}s`);
  }
  ff.stdin.end();
  await new Promise(r => ff.on('close', r));
  const final = join(here, 'out', `KeyIn_Promo_${W}x${H}.mp4`);
  execFileSync('ffmpeg', ['-v', 'error', '-y', '-i', out, '-i', join(here, 'out', 'score.wav'), '-map', '0:v', '-map', '1:a',
    '-c:v', 'copy', '-c:a', 'aac', '-b:a', '256k', '-ar', '48000', '-ac', '2', '-t', String(EDL.duration), '-movflags', '+faststart', final]);
  console.log('video ->', final);
}
await browser.close();
