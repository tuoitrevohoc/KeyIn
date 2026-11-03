// Timeline for the KeyIn 30s promo. Shared by film.js (browser), trace.mjs and score.mjs (node).
// Scripts are typed through the real engine; '|' is a one-beat hold (not a key).
globalThis.EDL = (() => {
  const KPS = 12;                    // keys per second: sixteenth-note triplets at 120 BPM
  const scenes = {
    // The same drink in three languages.
    hook1: { t0: 0.0,  ml: 1, script: 'traf suwax' },
    hook2: { t0: 0.0,  ml: 1, script: 'bubble tea' },
    hook3: { t0: 0.0,  ml: 1, script: 'naicha' },
    // Khiem's lines. Auto misreads "lei", "cho" and "mai", so the film fixes them with Tab
    // (and picks 累 with 2), exactly as a user would.
    mac:   { t0: 3.3,  ml: 1, script: "laoban  owi, hoom nay I'm very lei\t|\t||2, cho\t\t em nghir yixia , tomorrow em sex coos gawngs lamf vieecj harder... maybe." },
    win:   { t0: 15.6, ml: 0, script: 'jintianhen|  busy, mai|\t| cungx vaayj, jiuming| !' },
  };
  // Film time of every real key in a scene (holds removed), on the 1/12 s grid.
  for (const s of Object.values(scenes)) {
    s.keys = []; s.holds = []; let t = s.t0, hold = 0;
    for (const ch of s.script) {
      if (ch === '|') { if (!hold) s.holds.push({ t: +t.toFixed(4) }); hold += 0.5; t += 0.5; continue; }
      // afterHold: the key that ends a hold (a Space committing Chinese candidates).
      s.keys.push({ ch, t: +t.toFixed(4), afterHold: hold > 0 });
      if (hold) { s.holds.at(-1).d = hold; hold = 0; }
      t += 1 / KPS;
    }
    s.t1 = t;
  }
  // Hook words start on beats; their keys are faster so each lands in half a bar.
  const fast = (s, t0, kps) => s.keys.forEach((k, i) => { k.t = +(t0 + i / kps).toFixed(4); });
  fast(scenes.hook1, 0.0, 24); fast(scenes.hook2, 0.75, 24); fast(scenes.hook3, 1.5, 24);
  // The camera pushes in on the Mac candidates while the long hold shows them, and stays
  // for the key that picks one.
  const zh = scenes.mac.holds.find(h => h.d >= 1);
  return {
    fps: 30, duration: 30, bpm: 120, size: [1920, 1080], KPS, scenes,
    // Section boundaries.
    cut: { mac: 3.0, zoom: zh.t + 0.1, zoomOut: zh.t + zh.d + 0.4, win: 15.2, tab: 21.5, local: 22.0, end: 24.5 },
  };
})();
if (typeof module !== 'undefined') module.exports = globalThis.EDL;
