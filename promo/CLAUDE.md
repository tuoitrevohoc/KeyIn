# KeyIn promo film (motion-studio rules, from CatSort/motion-studio)

## Render contract
- Every film is a pure function of time: `window.seek(t)` paints frame t.
- No CSS transitions, no setTimeout, no requestAnimationFrame in render mode,
  no state carried between frames. Seeded noise only (mulberry32), never Math.random.
- Render with `node render.mjs full`, encode H.264 yuv420p, CRF 16.
- Typed text and candidates come from the real engine: `node trace.mjs` runs
  `examples/trace.rs` and writes data.js. Never hand-edit data.js.

## Look
- Banned defaults: centered title on gradient, everything fading in,
  corner labels and frame borders, glow on UI chrome, generic particle bursts.
- One display face, one UI face. One accent color (macOS system blue).
- Every 2 to 4 seconds something new must happen on screen.

## Sound
- Score and SFX are synthesized in code (score.mjs). 120 BPM; key clicks on the
  sixteenth-triplet grid (12 keys per second). Loudness -14 LUFS.

## Loop before showing anything
1. Render one frame per beat as a contact sheet (`node render.mjs sheet`) and LOOK at it.
2. Score 1-10: hook in first 2s, readability at phone size, motion quality,
   variety, brand accuracy, sound sync.
3. Fix the 3 worst problems. Repeat until every score is 8+. Then full render.
