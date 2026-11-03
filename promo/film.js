/* KeyIn — 30s promo. Pure function of time: window.seek(t) paints frame t. */
(() => {
const stage = document.getElementById('stage');
const EDL = globalThis.EDL, TR = globalThis.TRACE, SC = EDL.scenes, CUT = EDL.cut;
const ACC = '#007AFF', INK = '#141414';
// Layouts: 'h' is the 1920x1080 cut, 'v' the 1080x1920 cut for TikTok/Reels (content kept
// clear of the top 200px and bottom 380px that the apps cover).
const V = new URLSearchParams(location.search).get('fmt') === 'v';
const LY = V ? {
  W: 1080, H: 1920, head: { x: 70, y: 300, size: 76, w: 940 }, band: 570,
  hook: { ys: [540, 870, 1200], size: 140, tagBelow: true },
  S: 1.75, WX: 70, WY: 660, winW: 540, macH: 270, wndH: 230, strip: { y: 1300, n: 15 }, zoomC: [560, 920],
  tab: { key: [70, 1230], word: [70, 660], panel: [70, 960, 1.4] },
  local: { x: 70, y: 560, dy: 290, size: 100, w: 940 },
  end: { mark: [60, 450, 300], word: [370, 472, 200], sub: [76, 840, 66, 930], chips: [76, 1050], url: [76, 1160, 112], gh: [80, 1310, 36] },
} : {
  W: 1920, H: 1080, head: { x: 120, y: 62, size: 64, w: 0 }, band: 180,
  hook: { ys: [205, 455, 705], size: 150, tagBelow: false },
  S: 2.5, WX: 185, WY: 196, winW: 620, macH: 215, wndH: 230, strip: { y: 815, n: 22 }, zoomC: [860, 420],
  tab: { key: [110, 540], word: [560, 350], panel: [560, 650, 1.95] },
  local: { x: 150, y: 250, dy: 200, size: 116, w: 0 },
  end: { mark: [150, 200, 330], word: [510, 228, 230], sub: [156, 600, 64, 0], chips: [156, 722], url: [156, 836, 84], gh: [160, 950, 34] },
};
stage.style.width = LY.W + 'px'; stage.style.height = LY.H + 'px';
const wrapCss = w => w ? `white-space:normal;width:${w}px;` : '';
const LANG = { VI: { flag: '🇻🇳', name: 'Tiếng Việt', color: '#1FA2B8' }, EN: { flag: '🇺🇸', name: 'English', color: '#007AFF' },
  '简中': { flag: '🇨🇳', name: '中文', color: '#FF8A00' } };

// ---------- math ----------
const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
const lerp = (a, b, u) => a + (b - a) * u;
const E = {
  lin: u => u, out: u => 1 - Math.pow(1 - u, 3), in: u => u * u * u,
  inOut: u => u < .5 ? 4 * u * u * u : 1 - Math.pow(-2 * u + 2, 3) / 2,
  outBack: u => { const c1 = 1.7, c3 = c1 + 1; return 1 + c3 * Math.pow(u - 1, 3) + c1 * Math.pow(u - 1, 2); },
  outQuint: u => 1 - Math.pow(1 - u, 5),
};
const seg = (t, t0, t1, e = E.lin) => e(clamp((t - t0) / (t1 - t0)));
// Instant kick that rings down, for hits on the beat.
const punch = (t, t0, amp, d = 9, f = 14) => (t < t0 ? 0 : amp * Math.exp(-d * (t - t0)) * Math.cos(f * (t - t0)));

// ---------- dom ----------
function el(tag, cls, parent, css, html) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (css) e.style.cssText = css;
  if (html != null) e.innerHTML = html;
  (parent || stage).appendChild(e);
  return e;
}
const show = (e, on) => { const d = on ? '' : 'none'; if (e.style.display !== d) e.style.display = d; };
const setHTML = (e, h) => { if (e._h !== h) { e.innerHTML = h; e._h = h; } };
const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
const xf = (e, s) => { if (e._t !== s) { e.style.transform = s; e._t = s; } };
const op = (e, o) => { const v = String(+o.toFixed(3)); if (e.style.opacity !== v) e.style.opacity = v; };

// Engine state of a scene at film time t: the trace line after the last key pressed.
function state(id, t) {
  const keys = SC[id].keys;
  let k = 0;
  while (k < keys.length && keys[k].t <= t + 1e-6) k++;
  const s = k ? TR[id][k - 1] : { doc: '', pre: '', lang: 'VI', hints: [], items: [], pages: 0 };
  return { ...s, k, last: k ? keys[k - 1].t : -9, key: k ? keys[k - 1].ch : null };
}

// ---------- headlines: words pop up on their beat, leave upward ----------
const HEADS = [];
function head(t0, t1, words, x = LY.head.x, y = LY.head.y, size = LY.head.size, w = LY.head.w) {
  const h = el('div', 'hd', stage, `left:${x}px;top:${y}px;font-size:${size}px;${wrapCss(w)}`);
  const ws = words.map(w => typeof w === 'string'
    ? el('span', 'w' + (w.startsWith('*') ? ' acc' : ''), h, '', esc(w.replace(/^\*/, '')))
    : el('span', 'w', h, 'vertical-align:12%', `<span class="chip on" style="position:relative;font-size:28px;padding:8px 22px 10px">${w.chip}</span>`));
  HEADS.push({ h, ws, t0, t1 });
}
head(0.0, 2.85, ['Một', 'bộ', 'gõ.', '*Ba', '*thứ', '*tiếng.']);
head(3.05, 6.4, [{ chip: 'macOS' }, 'Tự', 'nhận', 'tiếng', '*từng', '*chữ.']);
head(6.5, 9.6, [{ chip: 'macOS' }, 'Telex', 'ra', 'tiếng', 'Việt,', '*English', '*vẫn', '*là', '*English.']);
head(9.75, 13.35, ['Gõ', 'Pinyin', 'ra', 'chữ', 'Hán,', '*kèm', '*nghĩa.']);
head(13.6, 19.35, [{ chip: 'Windows' }, 'Cũng', 'gõ', 'được', '*trên', '*Windows.']);
head(19.6, 23.85, ['Lỡ', 'sai', 'tiếng?', 'Nhấn', '*Tab.']);
function paintHeads(t) {
  for (const { h, ws, t0, t1 } of HEADS) {
    const on = t >= t0 - 0.01 && t < t1 + 0.3;
    show(h, on);
    if (!on) continue;
    const out = seg(t, t1, t1 + 0.25, E.in);
    ws.forEach((w, i) => {
      const a = t0 + i * 0.083, u = seg(t, a, a + 0.32, E.outBack);
      xf(w, `translateY(${(1 - u) * 70 - out * 90}px) rotate(${(1 - u) * 4}deg)`);
      op(w, clamp((t - a) * 8) * (1 - out));
    });
  }
}

// ---------- hook: three words typed live through the engine ----------
const HOOK = [
  { id: 'hook1', y: LY.hook.ys[0], commit: 0.75, lang: 'VI' },
  { id: 'hook2', y: LY.hook.ys[1], commit: 1.25, lang: 'EN' },
  { id: 'hook3', y: LY.hook.ys[2], commit: 2.0, lang: '简中' },
].map(h => {
  const box = el('div', 'hk', stage, `top:${h.y}px;left:${LY.head.x + 30}px`);
  box.style.fontSize = '1px';
  const parts = { txt: el('span', 'txt', box, `font-size:${LY.hook.size}px`), raw: el('div', 'raw', box), tag: el('div', 'tag', box) };
  // Portrait: the language tag sits under the word, the raw keys under the tag.
  if (LY.hook.tagBelow) { parts.tag.style.top = (LY.hook.size + 18) + 'px'; parts.raw.style.top = (LY.hook.size + 108) + 'px'; }
  return { ...h, box, ...parts };
});
function paintHook(t) {
  HOOK.forEach((h, i) => {
    const s = state(h.id, t), on = t < 3.4 && s.k > 0;
    show(h.box, on);
    if (!on) return;
    const done = t >= h.commit;
    const final = h.lang === '简中' ? TR[h.id].at(-1).items[0][0] : s.doc + s.pre;
    const caret = !done ? '<span class="caret"></span>' : '';
    setHTML(h.txt, done ? esc(final) : esc(s.doc) + `<span class="mk">${esc(s.pre)}</span>` + caret);
    // Raw keys sit under the word once it lands, so the trick is readable.
    setHTML(h.raw, 'gõ  ' + esc(SC[h.id].script));
    op(h.raw, done ? clamp((t - h.commit) * 5) : 0);
    const L = LANG[h.lang];
    setHTML(h.tag, `${L.flag} ${L.name}`);
    h.tag.style.background = L.color;
    h.tag.style.left = LY.hook.tagBelow ? '0px' : (h.txt.offsetWidth + 44) + 'px';
    const u = seg(t, h.commit, h.commit + 0.3, E.outBack);
    show(h.tag, done);
    xf(h.tag, `scale(${0.4 + 0.6 * u}) rotate(${(1 - u) * -8}deg)`);
    xf(h.txt, `translateY(${punch(t, h.commit, -14)}px)`);
    h.txt.style.display = 'inline-block';
    const out = seg(t, 2.8 + i * 0.06, 3.15 + i * 0.06, E.in);
    xf(h.box, `translateY(${-out * LY.H}px)`);
  });
}

// ---------- windows ----------
const { S, WX, WY } = LY;
const cam = el('div', 'cam', stage, `width:${LY.W}px;height:${LY.H}px`);
const band = el('div', 'abs', stage, `width:${LY.W}px;height:${LY.band}px;background:linear-gradient(#F4F2ED 78%, rgba(244,242,237,0));z-index:4`);
function macWindow() {
  const w = el('div', 'win mac', cam, `width:${LY.winW}px;height:${LY.macH}px`);
  const bar = el('div', 'bar', w);
  [['#FF5F57', 12], ['#FEBC2E', 32], ['#28C840', 52]].forEach(([c, x]) => el('div', 'tl', bar, `left:${x}px;background:${c}`));
  el('div', 'title', bar, '', 'Không tên — Đã sửa');
  return { w, doc: el('div', 'doc', w), panel: el('div', 'mp', w), docX: 26, docY: 52, line: 42 };
}
function winWindow() {
  const w = el('div', 'win wnd', cam, `width:${LY.winW}px;height:${LY.wndH}px`);
  const bar = el('div', 'bar', w);
  el('div', 'tab', bar, '', 'Không tên<b>✕</b>');
  [['─', LY.winW - 102], ['☐', LY.winW - 64], ['✕', LY.winW - 28]].forEach(([g, x]) => el('div', 'ctl', bar, `left:${x}px`, g));
  el('div', 'menu', w, '', 'Tệp Sửa Xem');
  el('div', 'status', w, '', 'UTF-8 &nbsp; Windows (CRLF) &nbsp; 100%');
  return { w, doc: el('div', 'doc', w), panel: el('div', 'wp', w), docX: 16, docY: 80, line: 38 };
}
const MAC = macWindow(), WIN = winWindow();

const macHints = hs => hs.map(h => h.startsWith('↑↓') ? h.replace(/EN/g, '🇺🇸').replace(/VI/g, '🇻🇳').replace(/简中/g, '🇨🇳') : h);
function macPanel(s, showMeanings) {
  const rows = showMeanings && s.lang === '简中' && s.items.some(i => i[1]);
  const L = LANG[s.lang];
  const items = s.items.map(([c, m], i) => `<div class="it${i ? '' : ' sel'}"><span class="n">${i + 1}</span>${esc(c)}${rows && m ? `<span class="m">— ${esc(m)}</span>` : ''}</div>`).join('')
    + (s.pages > 1 && !rows ? '<div class="chev">›</div>' : '');
  const ft = `<div class="ft"><span class="bd" style="color:${L.color}">${L.flag} ${s.lang}</span>${macHints(s.hints).map(h => `<span style="margin-left:18px">${esc(h)}</span>`).join('')}</div>`;
  return { cls: 'mp' + (rows ? ' rows' : '') + (s.items.length ? '' : ' empty'), html: (s.items.length ? `<div class="items">${items}</div>` : '') + ft };
}
function winPanel(s) {
  const L = LANG[s.lang];
  const items = s.items.map(([c], i) => `<div class="it${i ? '' : ' sel'}">${i + 1} ${esc(c)}</div>`).join('');
  const ft = `<div class="ft"><span class="bd" style="color:${L.color}">${s.lang}</span>&nbsp;&nbsp;&nbsp;${s.hints.map(esc).join('&nbsp;&nbsp;&nbsp;')}</div>`;
  return { cls: 'wp' + (s.items.length ? '' : ' empty'), html: (s.items.length ? `<div class="items">${items}</div>` : '') + ft };
}
// Fills a window's document and panel for scene `id`; returns the panel's anchor in window points.
function paintDoc(W, id, t, panelOf) {
  const s = state(id, t);
  const idle = t - s.last;
  const caretOn = idle < 0.45 || (idle % 1) < 0.5;
  setHTML(W.doc, esc(s.doc) + '<span class="anc"></span>' + (s.pre ? `<span class="mk">${esc(s.pre)}</span>` : '')
    + `<span class="caret" style="opacity:${caretOn ? 1 : 0}"></span>`);
  show(W.panel, !!s.pre);
  if (!s.pre) return null;
  const p = panelOf(s);
  if (W.panel.className !== p.cls) W.panel.className = p.cls;
  setHTML(W.panel, p.html);
  const anc = W.doc.querySelector('.anc');
  let x = W.docX + anc.offsetLeft - 8, y = W.docY + anc.offsetTop + W.line + 2;
  x = Math.max(-WX / S + 4, Math.min(x, LY.winW + 40 - W.panel.offsetWidth));
  W.panel.style.left = x + 'px'; W.panel.style.top = y + 'px';
  return { x, y, w: W.panel.offsetWidth, h: W.panel.offsetHeight };
}

// ---------- key strip: the physical keys, so Telex and Pinyin stay readable ----------
const strip = el('div', 'keys', stage, `left:${WX}px;top:${LY.strip.y}px`);
const caps = Array.from({ length: LY.strip.n }, () => el('div', 'kc', strip));
function paintKeys(id, t, alpha) {
  show(strip, alpha > 0);
  if (alpha <= 0) return;
  const s = state(id, t), keys = SC[id].keys.slice(Math.max(0, s.k - caps.length), s.k);
  caps.forEach((c, i) => {
    const k = keys[i];
    show(c, !!k);
    if (!k) return;
    const label = k.ch === ' ' ? 'space' : k.ch === '\t' ? 'Tab' : k.ch;
    setHTML(c, esc(label));
    const cls = 'kc' + (label.length > 1 ? ' wide' : '') + (i === keys.length - 1 && t - k.t < 0.14 ? ' hot' : '');
    if (c.className !== cls) c.className = cls;
    xf(c, `translateY(${i === keys.length - 1 ? 5 * (1 - seg(t, k.t, k.t + 0.12)) : 0}px)`);
  });
  op(strip, alpha);
}

// ---------- tab close-up ----------
const TAB = (() => {
  const box = el('div', 'abs', stage, `width:${LY.W}px;height:${LY.H}px`);
  const key = el('div', 'big-tab', box, `left:${LY.tab.key[0]}px;top:${LY.tab.key[1]}px`, 'Tab<span>⇥</span>');
  const word = el('div', 'hd', box, `left:${LY.tab.word[0]}px;top:${LY.tab.word[1]}px;font-size:190px;font-weight:700`);
  const tag = el('div', 'tag', box, `top:${LY.tab.word[1] + 66}px;` + 'position:absolute;font:700 48px SFP;padding:10px 26px 12px;border-radius:999px;color:#fff;white-space:nowrap');
  const pw = el('div', 'abs', box, `left:${LY.tab.panel[0]}px;top:${LY.tab.panel[1]}px;transform-origin:0 0;transform:scale(${LY.tab.panel[2]})`);
  const panel = el('div', 'mp', pw, 'position:relative');
  return { box, key, word, tag, panel };
})();
function paintTab(t) {
  const on = t >= CUT.tab && t < CUT.local;
  show(TAB.box, on);
  if (!on) return;
  const s = state('tab', t), tabs = SC.tab.keys.filter(k => k.ch === '\t');
  const inU = seg(t, CUT.tab, CUT.tab + 0.4, E.outQuint), out = seg(t, CUT.local - 0.25, CUT.local, E.in);
  xf(TAB.box, `translateY(${(1 - inU) * 300 - out * LY.H}px)`);
  op(TAB.box, inU);
  const press = tabs.find(k => t >= k.t && t - k.t < 0.18);
  TAB.key.className = 'big-tab' + (press ? ' hot' : '');
  const lastTab = tabs.filter(k => t >= k.t).pop();
  xf(TAB.key, `translateY(${press ? 10 : 0}px) rotate(-4deg)`);
  setHTML(TAB.word, s.pre ? `<span style="text-decoration:underline;text-decoration-thickness:6px;text-underline-offset:18px;text-decoration-color:#9a9a9a">${esc(s.pre)}</span>` : '');
  const L = LANG[s.lang];
  setHTML(TAB.tag, `${L.flag} ${L.name}`);
  TAB.tag.style.background = L.color;
  TAB.tag.style.left = (LY.tab.word[0] + TAB.word.offsetWidth + 50) + 'px';
  const u = lastTab ? seg(t, lastTab.t, lastTab.t + 0.3, E.outBack) : seg(t, SC.tab.keys[0].t, SC.tab.keys[0].t + 0.3, E.outBack);
  show(TAB.tag, !!s.pre);
  xf(TAB.tag, `scale(${0.5 + 0.5 * u}) rotate(${(1 - u) * -8}deg)`);
  const p = macPanel(s, false);
  show(TAB.panel, !!s.pre);
  if (TAB.panel.className !== p.cls) TAB.panel.className = p.cls;
  setHTML(TAB.panel, p.html);
}

// ---------- on-device statements ----------
const LOCAL = [['Nhanh,', '*chạy', '*ngay', '*trên', '*máy.'], ['Không', 'cần', '*mạng.'], ['Không', 'gửi', 'dữ', 'liệu', '*đi', '*đâu.']]
  .map((ws, i) => { const n = HEADS.length; head(CUT.local + i * 0.5, CUT.end - 0.15, ws, LY.local.x, LY.local.y + i * LY.local.dy, LY.local.size, LY.local.w); return HEADS[n]; });

// ---------- end card ----------
const END = (() => {
  const N = LY.end, box = el('div', 'abs', stage, `width:${LY.W}px;height:${LY.H}px`);
  const mark = el('div', 'abs', box, `left:${N.mark[0]}px;top:${N.mark[1]}px`, `<svg viewBox="80 60 350 390" width="${N.mark[2]}" height="${N.mark[2]}">
    <rect id="kb" x="114" y="86" width="56" height="340" fill="${INK}"/>
    <polygon id="kt" points="328,86 398,86 328,165 243,256 328,350 398,426 328,426" fill="${ACC}"/></svg>`);
  const word = el('div', 'hd', box, `left:${N.word[0]}px;top:${N.word[1]}px;font-size:${N.word[2]}px;letter-spacing:-0.04em`, 'KeyIn');
  const sub = el('div', 'hd', box, `left:${N.sub[0]}px;top:${N.sub[1]}px;font-size:${N.sub[2]}px;font-weight:700;${wrapCss(N.sub[3])}`, 'Bộ gõ <span class="acc">Anh · Việt · Trung</span> trong một.');
  const chips = el('div', 'abs', box, `left:${N.chips[0]}px;top:${N.chips[1]}px`);
  const c1 = el('div', 'chip on', chips, 'position:relative;display:inline-block;margin-right:18px', 'macOS');
  const c2 = el('div', 'chip on', chips, 'position:relative;display:inline-block;margin-right:18px', 'Windows');
  const c3 = el('div', 'chip', chips, 'position:relative;display:inline-block', 'Miễn phí · Mã nguồn mở');
  const url = el('div', 'hd', box, `left:${N.url[0]}px;top:${N.url[1]}px;font-size:${N.url[2]}px;color:#007AFF`, 'getkeyin.app');
  const gh = el('div', 'hd', box, `left:${N.gh[0]}px;top:${N.gh[1]}px;font-size:${N.gh[2]}px;font-weight:600;color:#6d6a63`, 'github.com/<span style="color:#141414">tuoitrevohoc/KeyIn</span>');
  return { box, kb: mark.querySelector('#kb'), kt: mark.querySelector('#kt'), word, sub, c: [c1, c2, c3], url, gh };
})();
function paintEnd(t) {
  const on = t >= CUT.end;
  show(END.box, on);
  if (!on) return;
  const t0 = CUT.end;
  const b = seg(t, t0, t0 + 0.35, E.outQuint);
  END.kb.setAttribute('transform', `translate(0 ${(1 - b) * 360})`);
  const k = seg(t, t0 + 0.25, t0 + 0.6, E.outBack);
  END.kt.setAttribute('transform', `translate(328 256) scale(${k}) translate(-328 -256)`);
  const w = seg(t, t0 + 0.5, t0 + 0.85, E.outBack);
  xf(END.word, `translateX(${(1 - w) * -120}px) scale(1)`);
  op(END.word, clamp((t - t0 - 0.5) * 6));
  const s = seg(t, t0 + 1.0, t0 + 1.35, E.outBack);
  xf(END.sub, `translateY(${(1 - s) * 60}px)`); op(END.sub, clamp((t - t0 - 1.0) * 6));
  END.c.forEach((c, i) => { const a = t0 + 1.5 + i * 0.25, u = seg(t, a, a + 0.3, E.outBack); xf(c, `scale(${u})`); op(c, clamp((t - a) * 8)); });
  const r = seg(t, t0 + 2.0, t0 + 2.35, E.outBack);
  xf(END.url, `translateY(${(1 - r) * 50}px) scale(${1 + punch(t, t0 + 2.0, 0.05)})`); op(END.url, clamp((t - t0 - 2.0) * 6));
  const g = seg(t, t0 + 2.5, t0 + 2.85, E.outBack);
  xf(END.gh, `translateY(${(1 - g) * 40}px)`); op(END.gh, clamp((t - t0 - 2.5) * 6));
}

// ---------- scenes with windows ----------
function paintWindows(t) {
  // Mac window: rises in at the cut, leaves left when Windows arrives.
  const macOn = t >= CUT.mac && t < CUT.win + 0.5;
  const winOn = t >= CUT.win && t < CUT.tab + 0.5;
  show(MAC.w, macOn); show(WIN.w, winOn);
  let anchor = null;
  if (macOn) {
    const up = seg(t, CUT.mac, CUT.mac + 0.5, E.outQuint), out = seg(t, CUT.win, CUT.win + 0.45, E.in);
    xf(MAC.w, `translate(${WX - out * 1900}px,${WY + (1 - up) * LY.H * 0.88}px) rotate(${(1 - up) * 3}deg) scale(${S})`);
    anchor = paintDoc(MAC, 'mac', t, s => macPanel(s, true));
  }
  if (winOn) {
    const inn = seg(t, CUT.win, CUT.win + 0.5, E.outQuint), out = seg(t, CUT.tab, CUT.tab + 0.45, E.in);
    xf(WIN.w, `translate(${WX + (1 - inn) * 1900 - out * 1900}px,${WY}px) scale(${S})`);
    paintDoc(WIN, 'win', t, winPanel);
  }
  // Camera: push in on the Chinese candidates and their meanings, then pull back for the commit.
  const z = seg(t, CUT.zoom, CUT.zoom + 0.5, E.inOut) * (1 - seg(t, 10.95, 11.4, E.inOut));
  if (z > 0 && anchor) {
    const fx = WX + (anchor.x + 150) * S, fy = WY + anchor.y * S, Z = 1 + 0.45 * z;
    const cx = lerp(fx, LY.zoomC[0], z), cy = lerp(fy, LY.zoomC[1], z);
    xf(cam, `translate(${cx - fx * Z}px,${cy - fy * Z}px) scale(${Z})`);
  } else xf(cam, 'none');
  // Paper band keeps the headline readable over the zoomed window.
  show(band, z > 0); op(band, z);
  const keyScene = t < CUT.win + 0.2 ? 'mac' : 'win';
  const a = t < CUT.mac + 0.4 ? seg(t, CUT.mac + 0.2, CUT.mac + 0.5) : t > CUT.tab - 0.1 ? 1 - seg(t, CUT.tab - 0.1, CUT.tab + 0.1) : (z > 0 ? 1 - z : 1);
  paintKeys(keyScene, t, macOn || winOn ? a : 0);
}

window.seek = t => {
  paintHeads(t);
  paintHook(t);
  paintWindows(t);
  paintTab(t);
  paintEnd(t);
};
window.ready = document.fonts.ready.then(() => window.seek(0));
})();
