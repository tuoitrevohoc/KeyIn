// Synthesize the score + SFX for the KeyIn promo. 120 BPM, A minor.
// node score.mjs  ->  out/score_raw.wav, out/score.wav (-14 LUFS), beats.json
// Instruments and mix bus adapted from CatSort/motion-studio/meowsort/score.mjs.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { execFileSync, spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
new Function(readFileSync(join(here, 'edl.js'), 'utf8'))();
const EDL = globalThis.EDL;
mkdirSync(join(here, 'out'), { recursive: true });

const SR = 48000, DUR = EDL.duration, N = Math.round(SR * DUR);
const BEAT = 60 / EDL.bpm, BAR = 4 * BEAT, E8 = BEAT / 2, E16 = BEAT / 4;
const TAU = Math.PI * 2;
const mtof = m => 440 * Math.pow(2, (m - 69) / 12);
function mulberry32(a) { return () => { a |= 0; a = a + 0x6D2B79F5 | 0; let t = Math.imul(a ^ a >>> 15, 1 | a); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
const rng = mulberry32(2024);
const noise = () => rng() * 2 - 1;

// ---------- buses ----------
const bus = () => [new Float32Array(N), new Float32Array(N)];
const DRY = bus(), VERB = bus(), DRUMS = bus();
function put(target, t, sig, gain = 1, pan = 0) {
  const s0 = Math.round(t * SR), gl = gain * Math.cos((pan + 1) * Math.PI / 4), gr = gain * Math.sin((pan + 1) * Math.PI / 4);
  for (let i = 0; i < sig.length; i++) {
    const k = s0 + i;
    if (k < 0 || k >= N) continue;
    target[0][k] += sig[i] * gl * Math.SQRT2;
    target[1][k] += sig[i] * gr * Math.SQRT2;
  }
}
const send = (t, sig, gain, pan, wet = 0.25) => { put(DRY, t, sig, gain, pan); put(VERB, t, sig, gain * wet, pan); };

// ---------- filters ----------
function biquad(type, f, q) {
  const w = TAU * f / SR, c = Math.cos(w), s = Math.sin(w), a = s / (2 * q);
  let b0, b1, b2, a0 = 1 + a, a1 = -2 * c, a2 = 1 - a;
  if (type === 'lp') { b0 = (1 - c) / 2; b1 = 1 - c; b2 = (1 - c) / 2; }
  else if (type === 'hp') { b0 = (1 + c) / 2; b1 = -(1 + c); b2 = (1 + c) / 2; }
  else { b0 = a; b1 = 0; b2 = -a; } // bandpass, 0 dB peak
  return [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0];
}
function filt(sig, type, fOf, q = 0.707) {
  const out = new Float32Array(sig.length);
  let x1 = 0, x2 = 0, y1 = 0, y2 = 0, co = null;
  for (let i = 0; i < sig.length; i++) {
    if (i % 32 === 0) co = biquad(type, typeof fOf === 'function' ? fOf(i / SR) : fOf, q);
    const x = sig[i], y = co[0] * x + co[1] * x1 + co[2] * x2 - co[3] * y1 - co[4] * y2;
    x2 = x1; x1 = x; y2 = y1; y1 = y; out[i] = y;
  }
  return out;
}
const buf = d => new Float32Array(Math.round(d * SR));

// ---------- instruments ----------
function marimba(m, d = 0.6, bright = 1) {
  const f = mtof(m), o = buf(d);
  for (let i = 0; i < o.length; i++) {
    const t = i / SR, att = Math.min(1, t / 0.002);
    o[i] = att * (Math.sin(TAU * f * t) * Math.exp(-t / 0.42) + 0.32 * bright * Math.sin(TAU * f * 3.93 * t) * Math.exp(-t / 0.07)
      + 0.08 * bright * Math.sin(TAU * f * 9.2 * t) * Math.exp(-t / 0.02));
  }
  return o;
}
function glock(m, d = 1.2) {
  const f = mtof(m), o = buf(d);
  for (let i = 0; i < o.length; i++) {
    const t = i / SR;
    o[i] = Math.min(1, t / 0.001) * (Math.sin(TAU * f * t) * Math.exp(-t / 0.55) + 0.4 * Math.sin(TAU * f * 2.76 * t) * Math.exp(-t / 0.18)
      + 0.2 * Math.sin(TAU * f * 5.4 * t) * Math.exp(-t / 0.06));
  }
  return o;
}
function pluck(m, d = 0.45, damp = 0.993) { // Karplus-Strong ukulele string
  const f = mtof(m), L = Math.max(2, Math.round(SR / f)), ring = new Float32Array(L), o = buf(d);
  let prev = 0;
  for (let i = 0; i < L; i++) { const n = noise(); ring[i] = (n + prev) * 0.5; prev = n; }
  let p = 0;
  for (let i = 0; i < o.length; i++) {
    const a = ring[p], b = ring[(p + 1) % L];
    ring[p] = (a + b) * 0.5 * damp;
    o[i] = a;
    p = (p + 1) % L;
  }
  const tail = Math.round(0.02 * SR);
  for (let i = 0; i < tail; i++) o[o.length - 1 - i] *= i / tail;
  return filt(o, 'lp', 3200);
}
function strum(notes, d = 0.32, gain = 1, down = true) {
  const o = buf(d + 0.06);
  (down ? notes : [...notes].reverse()).forEach((m, j) => {
    const p = pluck(m, d, 0.991), off = Math.round(j * 0.011 * SR);
    for (let i = 0; i < p.length && i + off < o.length; i++) o[i + off] += p[i] * gain * 0.5;
  });
  return o;
}
function bass(m, d) {
  const f = mtof(m), o = buf(d + 0.05);
  for (let i = 0; i < o.length; i++) {
    const t = i / SR, env = Math.min(1, t / 0.006) * Math.exp(-t / 0.5) * (t > d ? Math.exp(-(t - d) / 0.012) : 1);
    o[i] = Math.tanh(1.6 * (Math.sin(TAU * f * t) + 0.35 * Math.sin(TAU * 2 * f * t) + 0.12 * Math.sin(TAU * 3 * f * t))) * env * 0.7;
  }
  return o;
}
function pad(notes, d) {
  const o = buf(d + 0.6);
  for (const m of notes) for (const det of [-0.08, 0.08]) {
    const f = mtof(m + det); let ph = rng();
    for (let i = 0; i < o.length; i++) {
      const t = i / SR; ph += f / SR; ph -= Math.floor(ph);
      const env = Math.min(1, t / 0.35) * (t > d ? Math.exp(-(t - d) / 0.25) : 1);
      o[i] += (2 * ph - 1) * env * 0.12;
    }
  }
  return filt(filt(o, 'lp', 900), 'lp', 1400);
}
function kick(g = 1) {
  const o = buf(0.4); let ph = 0;
  for (let i = 0; i < o.length; i++) {
    const t = i / SR, f = 48 + 110 * Math.exp(-t / 0.035);
    ph += TAU * f / SR;
    o[i] = (Math.sin(ph) * Math.exp(-t / 0.22) + 0.25 * noise() * Math.exp(-t / 0.003)) * g;
  }
  return o;
}
function clap(g = 1) {
  const n = buf(0.3);
  for (let i = 0; i < n.length; i++) {
    const t = i / SR;
    const env = (t < 0.03 ? [0, 0.011, 0.022].reduce((s, b) => s + (t >= b ? Math.exp(-(t - b) / 0.004) : 0), 0) : 0) + 0.8 * Math.exp(-Math.max(0, t - 0.022) / 0.09) * (t >= 0.022);
    n[i] = noise() * env * g;
  }
  return filt(n, 'bp', 1300, 0.9);
}
function hat(g = 1, d = 0.035) {
  const n = buf(0.12);
  for (let i = 0; i < n.length; i++) n[i] = noise() * Math.exp(-(i / SR) / d) * g;
  return filt(n, 'hp', 7000);
}
function wood(f = 1800, g = 1) {
  const o = buf(0.12);
  for (let i = 0; i < o.length; i++) {
    const t = i / SR;
    o[i] = (Math.sin(TAU * f * t) * Math.exp(-t / 0.022) + 0.5 * Math.sin(TAU * f * 0.51 * t) * Math.exp(-t / 0.035) + 0.3 * noise() * Math.exp(-t / 0.002)) * g;
  }
  return o;
}
function pop(m) {
  const f = mtof(m), o = buf(0.16); let ph = 0;
  for (let i = 0; i < o.length; i++) {
    const t = i / SR, fr = f * (1 + 0.6 * Math.exp(-t / 0.012));
    ph += TAU * fr / SR;
    o[i] = (Math.sin(ph) + 0.2 * Math.sin(2 * ph)) * Math.exp(-t / 0.06) * Math.min(1, t / 0.002);
  }
  return o;
}
function sweepNoise(d, f0, f1, q = 1.2) {
  const n = buf(d);
  for (let i = 0; i < n.length; i++) { const u = i / n.length; n[i] = noise() * Math.pow(Math.sin(Math.PI * Math.min(1, u * 1.15)), 1.5); }
  return filt(n, 'bp', t => f0 * Math.pow(f1 / f0, Math.min(1, t / d)), q);
}
function meow(d = 0.62, pitch = 1) {
  const o = buf(d + 0.05); let ph = 0;
  const key = (t, ks) => { for (let i = 1; i < ks.length; i++) if (t <= ks[i][0]) { const [t0, a] = ks[i - 1], [t1, b] = ks[i]; const u = (t - t0) / (t1 - t0); return a.map((x, j) => x + (b[j] - x) * (0.5 - 0.5 * Math.cos(Math.PI * u))); } return ks[ks.length - 1][1]; };
  // [f0, F1, F2, amp] : m -> i -> a -> ow
  const ks = [[0, [430, 320, 1300, 0.3]], [0.07 * d / 0.62, [560, 420, 2300, 0.85]], [0.28 * d / 0.62, [790, 880, 1650, 1]], [0.5 * d / 0.62, [600, 560, 1000, 0.8]], [d, [430, 420, 820, 0]]];
  for (let i = 0; i < o.length; i++) {
    const t = i / SR, [f0r, F1, F2, amp] = key(t, ks), f0 = f0r * pitch * (1 + 0.018 * Math.sin(TAU * 6 * t));
    ph += f0 / SR;
    let s = 0;
    for (let k = 1; k * f0 < 6000; k++) {
      const fk = k * f0;
      const g = 1 / (1 + ((fk - F1) / 160) ** 2) + 0.6 / (1 + ((fk - F2) / 260) ** 2) + 0.08 / (1 + ((fk - 3300) / 500) ** 2);
      s += g * Math.sin(TAU * k * ph) / Math.sqrt(k);
    }
    const env = Math.min(1, t / 0.03) * Math.min(1, Math.max(0, (d - t) / 0.12)) * amp;
    o[i] = s * env * 0.35 + noise() * env * 0.015;
  }
  return o;
}

// ---------- reverb (Freeverb-style) ----------
function reverb([inL, inR], room = 0.8, damp = 0.35) {
  const combs = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617].map(n => Math.round(n * SR / 44100));
  const aps = [556, 441, 341, 225].map(n => Math.round(n * SR / 44100));
  const run = (inp, spread) => {
    const out = new Float32Array(N);
    for (const c of combs) {
      const L = c + spread, b = new Float32Array(L); let p = 0, st = 0;
      for (let i = 0; i < N; i++) { const y = b[p]; st = y * (1 - damp) + st * damp; b[p] = inp[i] + st * room; out[i] += y; p = (p + 1) % L; }
    }
    for (const a of aps) {
      const L = a + spread, b = new Float32Array(L); let p = 0;
      for (let i = 0; i < N; i++) { const bo = b[p], x = out[i]; out[i] = -x + bo; b[p] = x + bo * 0.5; p = (p + 1) % L; }
    }
    for (let i = 0; i < N; i++) out[i] *= 0.06;
    return out;
  };
  return [run(inL, 0), run(inR, 23)];
}

// ---------- keyboard sounds ----------
// A key: short band-passed click plus a small low body. `v` varies pitch per key (seeded).
function keyClick(v = 0, g = 1) {
  const o = buf(0.06), f = 3200 + 900 * v, body = 180 + 60 * v;
  for (let i = 0; i < o.length; i++) {
    const t = i / SR;
    o[i] = (noise() * Math.exp(-t / 0.004) * 0.9 + Math.sin(TAU * body * t) * Math.exp(-t / 0.012) * 0.6) * g;
  }
  return filt(o, 'bp', f, 0.8).map((x, i) => x * 2.2 + Math.sin(TAU * body * i / SR) * Math.exp(-(i / SR) / 0.012) * 0.25 * g);
}
function thock(g = 1) { // space bar / Tab: deeper, longer
  const o = buf(0.12);
  for (let i = 0; i < o.length; i++) {
    const t = i / SR;
    o[i] = (Math.sin(TAU * (110 + 60 * Math.exp(-t / 0.01)) * t) * Math.exp(-t / 0.03) + 0.5 * noise() * Math.exp(-t / 0.006)) * g;
  }
  return filt(o, 'lp', 2400);
}

// ---------- arrangement ----------
const CH = { Am: [45, [57, 60, 64, 69], [57, 60, 64]], F: [41, [57, 60, 65, 69], [53, 57, 60]], C: [36, [55, 60, 64, 67], [52, 55, 60]],
  G: [43, [55, 59, 62, 67], [50, 55, 59]], Dm: [38, [57, 62, 65, 69], [50, 53, 57]], E: [40, [56, 59, 64, 68], [52, 56, 59]] };
const RIFF = { Am: [76, 72, 69, 72, 74], F: [77, 72, 69, 72, 76], C: [76, 72, 67, 72, 74], G: [74, 71, 67, 71, 74], Dm: [74, 69, 65, 69, 72], E: [76, 71, 68, 71, 74] };
const RIFF_POS = [0, 2, 3, 5, 6];
const PROG = ['Am', 'F', 'C', 'G', 'Am', 'F', 'C', 'G', 'Dm', 'F', 'Am', 'G', 'F', 'G', 'Am'];
const chordAt = t => PROG[Math.min(Math.floor(t / BAR), PROG.length - 1)];
const C_ = EDL.cut;
const sec = t => t < C_.mac ? 'hook' : t < C_.win ? 'mac' : t < C_.tab ? 'win' : t < C_.local ? 'tab' : t < C_.end ? 'local' : 'end';

for (let k = 0; k < DUR / E8; k++) {
  const t = k * E8, s = sec(t), ch = chordAt(t), [root, chord, padN] = CH[ch];
  const inBar = k % 8, beat = inBar % 2 === 0, beatN = inBar / 2;
  const full = s === 'hook' || s === 'local' || s === 'tab';
  if (t >= 28.0) continue; // the end chord rings out
  // drums: four on the floor in loud sections, half-time under the typing
  if (beat) {
    if (full || (s === 'end' && beatN === 0) || ((s === 'mac' || s === 'win') && beatN % 2 === 0)) put(DRUMS, t, kick(full ? 1 : 0.8), 0.9);
    if ((full || s === 'win') && beatN % 2 === 1) put(DRUMS, t, clap(), 0.36, 0.05);
  }
  if (s !== 'end') put(DRUMS, t, hat(beat ? 0.5 : 1), full ? 0.1 : 0.06, 0.35);
  // bass on 1, the and of 2, 3
  if (s !== 'end' && [0, 3, 4].includes(inBar)) put(DRY, t, bass(inBar === 3 ? root + 12 : root, E8 * 1.5), full ? 0.5 : 0.4);
  // marimba riff, softer under typing so the clicks read
  const ri = RIFF_POS.indexOf(inBar);
  if (ri >= 0 && s !== 'end') {
    const m = RIFF[ch][ri];
    send(t, marimba(m, 0.7), full ? 0.34 : 0.22, -0.2, 0.3);
    if (s === 'local' || s === 'tab') send(t, glock(m + 12, 0.9), 0.1, 0.3, 0.35);
  }
}
// pads glue the typing sections
for (let b = 0; b < 13; b++) send(b * BAR, pad(CH[PROG[b]][2], BAR), b < 2 ? 0.08 : 0.12, 0, 0.4);

// ---------- SFX: every key from the EDL, plus scene hits ----------
const krng = mulberry32(77);
for (const [id, s] of Object.entries(EDL.scenes)) for (const k of s.keys) {
  const v = krng() * 2 - 1, pan = (krng() - 0.5) * 0.5;
  if (k.ch === ' ') put(DRY, k.t, thock(0.8), 0.32, pan);
  else if (k.ch === '\t') { put(DRY, k.t, thock(1.2), 0.5, 0); }
  else put(DRY, k.t, keyClick(v), id.startsWith('hook') ? 0.3 : 0.2, pan);
}
const pop2 = (t, m, g = 0.36) => send(t, pop(m), g, 0.1, 0.25);
// hook: each word lands with a rising pop + glock
[[0.75, 69], [1.25, 72], [2.0, 76]].forEach(([t, m]) => { pop2(t, m, 0.42); send(t, glock(m + 12, 0.9), 0.2, 0.2, 0.4); });
const whoosh = (t, d, pan = 0) => { const w = sweepNoise(d, 350, 4200, 1.0); put(DRY, t, w, 0.45, pan - 0.4); put(DRY, t + 0.04, w, 0.35, pan + 0.4); };
whoosh(C_.mac - 0.05, 0.6); whoosh(C_.win - 0.1, 0.6); whoosh(C_.local - 0.25, 0.5);
// zoom into the candidates: riser, then a chord when 你好 commits
send(C_.zoom - 0.6, sweepNoise(0.7, 500, 5000, 1.4), 0.22, 0, 0.3);
const commitT = EDL.scenes.mac.keys.find(k => k.afterHold && k.t > C_.zoom).t;
[0, 3, 7, 12].forEach((iv, j) => send(commitT + j * 0.05, glock(81 + iv, 1.0), 0.24, -0.3 + j * 0.2, 0.4));
// Windows: a pop on every Chinese commit (the Space that ends a hold)
for (const k of EDL.scenes.win.keys) if (k.afterHold) pop2(k.t, 79, 0.28);
// Tab: every language switch gets a rising note
Object.values(EDL.scenes).flatMap(s => s.keys).filter(k => k.ch === '\t').forEach((k, i) => { pop2(k.t, [76, 79, 84][i % 3], 0.4); send(k.t, glock([88, 91, 96][i % 3], 0.8), 0.12, 0.2, 0.4); });
// on-device statements and the end card
[C_.local, C_.local + 0.5, C_.local + 1.0].forEach((t, i) => { put(DRUMS, t, kick(1.1), 0.7); pop2(t, [72, 74, 76][i], 0.3); });
put(DRUMS, C_.end, kick(1.3), 1.0); send(C_.end, filt(sweepNoise(1.6, 7000, 5000, 0.5), 'hp', 4000), 0.26, 0, 0.5);
[[C_.end + 0.25, 81], [C_.end + 0.5, 84], [C_.end + 1.0, 88], [C_.end + 1.5, 93]].forEach(([t, m]) => send(t, glock(m, 1.4), 0.2, 0.2, 0.45));
send(C_.end, pad([57, 60, 64, 69], 3.6), 0.2, 0, 0.5);
put(DRY, C_.end, bass(33, 2.0), 0.55);
send(C_.end + 2.0, strum([57, 60, 64, 69, 72], 2.0, 1), 0.4, 0.25, 0.4);
put(DRY, C_.end + 2.0, bass(45, 1.6), 0.4);
// ---------- mix ----------
const [RL, RR] = reverb(VERB);
const L = new Float32Array(N), R = new Float32Array(N);
for (let i = 0; i < N; i++) {
  L[i] = DRY[0][i] + RL[i] + DRUMS[0][i];
  R[i] = DRY[1][i] + RR[i] + DRUMS[1][i];
}
// gentle bus glue, fade the last 0.4s, peak normalize before loudness pass
let peak = 0;
for (let i = 0; i < N; i++) { L[i] = Math.tanh(L[i] * 0.9); R[i] = Math.tanh(R[i] * 0.9); }
const fadeN = Math.round(0.4 * SR);
for (let i = 0; i < fadeN; i++) { const g = i / fadeN; L[N - 1 - i] *= g; R[N - 1 - i] *= g; }
for (let i = 0; i < N; i++) peak = Math.max(peak, Math.abs(L[i]), Math.abs(R[i]));
const norm = 0.89 / peak;

function wav(path, l, r) {
  const b = Buffer.alloc(44 + N * 4);
  b.write('RIFF', 0); b.writeUInt32LE(36 + N * 4, 4); b.write('WAVEfmt ', 8); b.writeUInt32LE(16, 16); b.writeUInt16LE(1, 20); b.writeUInt16LE(2, 22);
  b.writeUInt32LE(SR, 24); b.writeUInt32LE(SR * 4, 28); b.writeUInt16LE(4, 32); b.writeUInt16LE(16, 34); b.write('data', 36); b.writeUInt32LE(N * 4, 40);
  for (let i = 0; i < N; i++) { b.writeInt16LE(Math.round(Math.max(-1, Math.min(1, l[i] * norm)) * 32767), 44 + i * 4); b.writeInt16LE(Math.round(Math.max(-1, Math.min(1, r[i] * norm)) * 32767), 46 + i * 4); }
  writeFileSync(path, b);
}
const raw = join(here, 'out', 'score_raw.wav'), fin = join(here, 'out', 'score.wav');
wav(raw, L, R);

// ---------- measured beat grid (spectral-flux onsets on the drum bus) ----------
const hop = 256, onsets = [];
let prevE = 0; const flux = [];
for (let i = 0; i + hop < N; i += hop) { let e = 0; for (let j = 0; j < hop; j++) e += DRUMS[0][i + j] ** 2 + DRUMS[1][i + j] ** 2; flux.push(Math.max(0, e - prevE)); prevE = e; }
const thr = Math.max(...flux) * 0.08;
for (let i = 1; i < flux.length - 1; i++) if (flux[i] > thr && flux[i] >= flux[i - 1] && flux[i] > flux[i + 1]) onsets.push(i * hop / SR);
const grid = Array.from({ length: Math.round(DUR / BEAT) }, (_, k) => +(k * BEAT).toFixed(3));
const measured = grid.map(g => { let best = null; for (const o of onsets) if (Math.abs(o - g) < 0.06 && (best === null || Math.abs(o - g) < Math.abs(best - g))) best = o; return best === null ? null : +best.toFixed(4); });
const dev = measured.filter(m => m !== null).map((m, i) => Math.abs(m - grid[measured.indexOf(m)]));
const T12 = 1 / 24, keyOff = Object.values(EDL.scenes).flatMap(sc => sc.keys).map(k => Math.round(Math.abs(k.t / T12 - Math.round(k.t / T12)) * T12 * 1000));
writeFileSync(join(here, 'beats.json'), JSON.stringify({ bpm: EDL.bpm, beatSec: BEAT, grid, measured,
  measuredHits: measured.filter(m => m !== null).length, maxDeviationMs: Math.round(Math.max(...dev) * 1000), keysOffGridMaxMs: Math.max(...keyOff) }, null, 1));

// ---------- loudness: two-pass loudnorm to -14 LUFS, -1 dBTP ----------
const probe = spawnSync('ffmpeg', ['-hide_banner', '-i', raw, '-af', 'loudnorm=I=-14:TP=-1:LRA=11:print_format=json', '-f', 'null', '-'], { encoding: 'utf8' }).stderr;
const js = probe.slice(probe.lastIndexOf('{')); const m = JSON.parse(js.slice(0, js.indexOf('}') + 1));
execFileSync('ffmpeg', ['-v', 'error', '-y', '-i', raw, '-af',
  `loudnorm=I=-14:TP=-1:LRA=11:measured_I=${m.input_i}:measured_TP=${m.input_tp}:measured_LRA=${m.input_lra}:measured_thresh=${m.input_thresh}:offset=${m.target_offset}:linear=true`,
  '-ar', '48000', '-ac', '2', fin]);
console.log('score ->', fin, 'input', m.input_i, 'LUFS');
