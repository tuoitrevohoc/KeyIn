// Timeline for the KeyIn 30s promo. Shared by film.js (browser), trace.mjs and score.mjs (node).
// Scripts are typed through the real engine; '|' is a one-beat hold (not a key).
globalThis.EDL = (() => {
  const KPS = 12;                    // keys per second: sixteenth-note triplets at 120 BPM
  const scenes = {
    hook1: { t0: 0.0,  ml: 1, script: 'tieengs Vieejt' },
    hook2: { t0: 0.0,  ml: 1, script: 'hello' },
    hook3: { t0: 0.0,  ml: 1, script: 'nihao' },
    mac:   { t0: 3.5,  ml: 1, script: 'Hoom nay tooi hojc tieengs Trung voiws my friend. Thaafy giaos nois nihao|||  nghiax laf hello.' },
    win:   { t0: 14.0, ml: 0, script: 'Cuoois tuaanf women| yiqi| qu chifan|  nhes!' },
    tab:   { t0: 20.0, ml: 1, script: 'ma||\t||\t||\t' },
  };
  // Film time of every real key in a scene (holds removed), on the 1/12 s grid.
  for (const s of Object.values(scenes)) {
    s.keys = []; let t = s.t0;
    for (const ch of s.script) {
      if (ch === '|') { t += 0.5; continue; }
      s.keys.push({ ch, t: +t.toFixed(4) });
      t += 1 / KPS;
    }
    s.t1 = t;
  }
  // Hook words start on beats; their keys are faster so each lands in half a bar.
  const fast = (s, t0, kps) => s.keys.forEach((k, i) => { k.t = +(t0 + i / kps).toFixed(4); });
  fast(scenes.hook1, 0.0, 24); fast(scenes.hook2, 0.75, 24); fast(scenes.hook3, 1.5, 24);
  return {
    fps: 30, duration: 30, bpm: 120, size: [1920, 1080], KPS, scenes,
    // Section boundaries.
    cut: { mac: 3.0, zoom: 9.75, win: 13.5, tab: 19.5, local: 24.0, end: 26.0 },
  };
})();
if (typeof module !== 'undefined') module.exports = globalThis.EDL;
