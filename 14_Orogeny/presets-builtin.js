/* ══════════════════════════════════════════════════════════════════════════
   presets-builtin.js — presets compiled into the tool.

   These need no folder, no permission grant and no server: they are plain JS,
   so they work from file:// like the rest of the experiment. The folder store
   in preset-store.js remains for presets you author yourself; these are the
   ones that ship.

   A saved preset with the same id OVERRIDES the built-in of that id, so
   loading one in the designer, changing it and saving replaces it rather than
   producing a confusing duplicate. Delete your copy and the built-in returns.
   ════════════════════════════════════════════════════════════════════════ */
(function(){
'use strict';

const noise = [
  {
    id: 'builtin-mountains',
    name: 'Mountains',
    builtin: true,
    /* Selected into the OROGEN slot on first load, not the terrain slot. This
       is fold displacement baked into the high-pressure region — it belongs to
       the orogen, not to the whole planet. */
    defaultFor: 'orogen',
    mode: 'composable',
    /* Cellular F1xF2 on a Chebyshev metric — square-ish cells whose product
       field peaks along the cell walls — run through Billow, domain-warped,
       then folded about a pivot just above zero. The warp is what turns the
       cell lattice into something that reads as ranges rather than as a grid;
       the fold puts the crease on the walls.

       Resolution note, since freq 43 is close to the ceiling: a great circle is
       4 x 512 = 2048 texels at the default grid. The first octave lays down
       2*pi*43 ~ 270 cells around, so ~7.6 texels per cell — comfortably
       resolved. The second octave at lacunarity 3 is freq 129, ~810 cells,
       ~2.5 texels per cell. That is at the sampling limit: it contributes real
       detail at 31 % amplitude but will also alias. Drop to one octave if the
       shimmer bothers you. */
    np: {
      base: 4, combinator: 4, seed: [0,0,0],
      freq: 43, octaves: 2, lacunarity: 3, gain: 0.31,
      metric: 2, fIndex: 6, jitter: 0.77, minkowskiP: 2,
      crackleThr: 0.08, smoothK: 8,
      waveN: 4, waveAngle: 0, kernelBW: 0.4, kernelOrient: 0,
      warpOn: 1, warpIter: 1, warpAmt: 1.33,
      curlOn: 0, curlAmt: 0,
      aniOn: 0, aniAngle: 0, aniStretch: 2.8,
      terraceSteps: 6,
      foldOn: 1, foldPivot: 0.05, foldGain: 2.4, foldSign: 1,
      remapMode: 0, rFreq: 1, rAmt: 1, rThresh: 0, rSoft: 0.15,
    },
  },
];

/* ── planet presets ─────────────────────────────────────────────────────────
   Patterns for the gas layer and surfaces for rock, each with a gradient map so
   it can also colour the planet. `for` says where the planet tool lists it:
   'gas' → the Gas module's Pattern, 'terrain' → Terrain noise; every preset
   with a gradient also appears under Colours. Presets you save yourself carry
   no `for` and are listed everywhere.

   Gas bands use the STRATA remap: fract(q.y·rFreq + v·rAmt), and on the unit
   sphere q.y is freq·sin(latitude) — so the bands are latitude bands, the fBm
   term (v·rAmt) wobbles their edges and the domain warp curls them. One strata
   cycle runs the whole gradient, so a gas gradient is CYCLIC: it starts and
   ends on the same colour, with belts and zones in between. Band count across
   the sphere is 2·freq·rFreq. These are first passes, tuned by reasoning, not by
   eye — open them in the designer's sphere view and adjust. */
const NP_DEFAULTS = {
  base:1, combinator:1, seed:[0,0,0], freq:2.2,
  octaves:5, lacunarity:2.03, gain:0.5,
  metric:0, fIndex:0, jitter:0.9, minkowskiP:2,
  crackleThr:0.08, smoothK:8,
  waveN:4, waveAngle:0, kernelBW:0.4, kernelOrient:0,
  warpOn:0, warpIter:1, warpAmt:0.9,
  curlOn:0, curlAmt:0.6,
  aniOn:0, aniAngle:0, aniStretch:2.5,
  terraceSteps:6,
  foldOn:0, foldPivot:0, foldGain:2, foldSign:1,
  remapMode:0, rFreq:1, rAmt:1, rThresh:0, rSoft:0.15,
};
const np = (o) => Object.assign({}, NP_DEFAULTS, o);
const grad = (stops, range) => ({ stops: stops.map(([t, c]) => ({ t, color: c })), range: range || [-1, 1] });

noise.push(
  {
    id: 'builtin-jupiter', name: 'Jupiter bands', builtin: true, mode: 'composable', for: ['gas'],
    // 2·2.2·1.8 ≈ 8 strata cycles, each a belt–zone pair: ~16 bands
    np: np({ base:3, combinator:1, freq:2.2, octaves:6, warpOn:1, warpAmt:0.55, remapMode:3, rFreq:1.8, rAmt:0.22, seed:[3.1,0,7.7] }),
    gradient: grad([
      [0.00, [0.55, 0.36, 0.24]], [0.16, [0.72, 0.52, 0.36]], [0.34, [0.93, 0.88, 0.78]],
      [0.52, [0.86, 0.75, 0.60]], [0.68, [0.70, 0.45, 0.30]], [0.84, [0.91, 0.85, 0.73]],
      [1.00, [0.55, 0.36, 0.24]],
    ]),
  },
  {
    id: 'builtin-saturn', name: 'Saturn bands', builtin: true, mode: 'composable', for: ['gas'],
    np: np({ base:3, combinator:1, freq:2.0, octaves:5, warpOn:1, warpAmt:0.25, remapMode:3, rFreq:3.0, rAmt:0.08, seed:[1.4,2.2,0] }),
    gradient: grad([
      [0.00, [0.78, 0.66, 0.46]], [0.30, [0.88, 0.80, 0.62]], [0.55, [0.94, 0.89, 0.76]],
      [0.80, [0.84, 0.74, 0.54]], [1.00, [0.78, 0.66, 0.46]],
    ]),
  },
  {
    id: 'builtin-neptune', name: 'Neptune bands', builtin: true, mode: 'composable', for: ['gas'],
    np: np({ base:3, combinator:1, freq:2.5, octaves:6, warpOn:1, warpAmt:0.8, remapMode:3, rFreq:0.9, rAmt:0.35, seed:[5,1,2] }),
    gradient: grad([
      [0.00, [0.16, 0.27, 0.66]], [0.40, [0.25, 0.42, 0.85]], [0.70, [0.38, 0.56, 0.92]],
      [0.80, [0.88, 0.92, 0.98]], [0.86, [0.38, 0.56, 0.92]], [1.00, [0.16, 0.27, 0.66]],
    ]),
  },
  {
    id: 'builtin-venus-deck', name: 'Venus cloud deck', builtin: true, mode: 'composable', for: ['gas'],
    // few broad, strongly warped bands: the Y-shaped chevrons of the UV images, roughly
    np: np({ base:3, combinator:1, freq:1.6, octaves:6, warpOn:1, warpIter:2, warpAmt:1.2, remapMode:3, rFreq:0.6, rAmt:0.6, seed:[9,4,1] }),
    gradient: grad([
      [0.00, [0.80, 0.66, 0.40]], [0.35, [0.90, 0.80, 0.56]], [0.65, [0.95, 0.88, 0.68]],
      [1.00, [0.80, 0.66, 0.40]],
    ]),
  },
  {
    id: 'builtin-continents', name: 'Continents', builtin: true, mode: 'composable', for: ['terrain'],
    np: np({ base:2, combinator:1, freq:1.8, octaves:6, warpOn:1, warpAmt:0.8, seed:[2,7,1] }),
    gradient: grad([
      [0.00, [0.03, 0.08, 0.20]], [0.44, [0.13, 0.34, 0.55]], [0.50, [0.80, 0.74, 0.54]],
      [0.56, [0.32, 0.50, 0.23]], [0.75, [0.45, 0.40, 0.28]], [0.90, [0.55, 0.52, 0.50]],
      [1.00, [0.95, 0.96, 0.98]],
    ]),
  },
  {
    id: 'builtin-dunes', name: 'Dune sea', builtin: true, mode: 'composable', for: ['terrain'],
    // ridged and stretched: long crests in one direction
    np: np({ base:3, combinator:3, freq:4.0, octaves:4, aniOn:1, aniAngle:0.6, aniStretch:3.2, seed:[4,4,4] }),
    gradient: grad([
      [0.00, [0.55, 0.38, 0.22]], [0.50, [0.78, 0.60, 0.36]], [1.00, [0.93, 0.80, 0.55]],
    ]),
  },
  {
    id: 'builtin-lava-crust', name: 'Lava crust', builtin: true, mode: 'composable', for: ['terrain'],
    // cell walls: plates of cooled crust; the walls are the cracks, so they take the hot end of the ramp
    np: np({ base:4, fIndex:2, combinator:1, octaves:3, freq:6.0, jitter:0.9, seed:[8,1,3] }),
    gradient: grad([
      [0.00, [1.00, 0.85, 0.30]], [0.12, [1.00, 0.35, 0.05]], [0.28, [0.30, 0.06, 0.02]],
      [0.50, [0.10, 0.08, 0.08]], [1.00, [0.05, 0.04, 0.04]],
    ]),
  },
  {
    id: 'builtin-ice-lineae', name: 'Ice lineae', builtin: true, mode: 'composable', for: ['terrain'],
    // crack network over clean ice; the cracks take the rust-brown end, as on Europa
    np: np({ base:4, fIndex:2, combinator:1, octaves:3, freq:3.0, jitter:1.0, warpOn:1, warpAmt:0.4, seed:[6,0,6] }),
    gradient: grad([
      [0.00, [0.50, 0.32, 0.20]], [0.10, [0.72, 0.58, 0.46]], [0.25, [0.86, 0.88, 0.90]],
      [1.00, [0.94, 0.96, 0.98]],
    ]),
  },
  {
    id: 'builtin-maria', name: 'Lunar maria', builtin: true, mode: 'composable', for: ['terrain'],
    // broad low basins (dark maria) against bright highlands
    np: np({ base:2, combinator:4, freq:1.4, octaves:5, warpOn:1, warpAmt:0.5, seed:[1,9,5] }),
    gradient: grad([
      [0.00, [0.20, 0.20, 0.21]], [0.40, [0.28, 0.28, 0.29]], [0.55, [0.48, 0.47, 0.45]],
      [1.00, [0.62, 0.61, 0.59]],
    ]),
  },
);

/* ── base noises: one starting terrain per planet type ───────────────────────
   Named by planet-presets.js (`terrain:`), so choosing a planet starts it on its
   own relief instead of the generic fBm. The planet scales the value by its
   nAmp and, with plates, still lays the continental split on top. First passes
   by reasoning — tune them in the designer's sphere view. */
noise.push(
  {
    id: 'builtin-orogen-ridges', name: 'Orogen ridges', builtin: true, mode: 'composable', for: ['terrain'],
    // ridged multifractal, warped: sinuous crest lines rather than blobs
    np: np({ base:3, combinator:3, freq:2.4, octaves:7, warpOn:1, warpAmt:0.7, seed:[1.2,3.4,5.6] }),
  },
  {
    id: 'builtin-supercontinent', name: 'Supercontinent', builtin: true, mode: 'composable', for: ['terrain'],
    // very low base frequency and heavy warp: one or two large masses, ragged coasts from the high octaves
    np: np({ base:2, combinator:1, freq:0.9, octaves:8, warpOn:1, warpIter:2, warpAmt:1.1, seed:[7,2,9] }),
  },
  {
    id: 'builtin-islands', name: 'Island scatter', builtin: true, mode: 'composable', for: ['terrain'],
    // billow: rounded highs separated by creases — scattered islands once the sea rises over the creases
    np: np({ base:3, combinator:4, freq:3.2, octaves:6, warpOn:1, warpAmt:0.5, seed:[4,8,1] }),
  },
  {
    id: 'builtin-mars-dichotomy', name: 'Mars highlands', builtin: true, mode: 'composable', for: ['terrain'],
    // hybrid multifractal: rough highlands, smoother lowlands — a stand-in for the crustal dichotomy
    np: np({ base:2, combinator:5, freq:1.6, octaves:7, warpOn:1, warpAmt:0.6, seed:[3,3,8] }),
  },
  {
    id: 'builtin-venus-plains', name: 'Volcanic plains', builtin: true, mode: 'composable', for: ['terrain'],
    // multifractal: broad smooth plains with rare rough patches (tesserae, roughly)
    np: np({ base:3, combinator:6, freq:1.3, octaves:6, warpOn:1, warpAmt:0.9, seed:[6,1,4] }),
  },
  {
    id: 'builtin-calderas', name: 'Caldera pits', builtin: true, mode: 'composable', for: ['terrain'],
    // smooth F1 cells with a few fBm octaves: rounded depressions, Io's paterae roughly
    np: np({ base:4, fIndex:11, combinator:1, octaves:3, freq:5.0, smoothK:10, seed:[5,5,0] }),
  },
  {
    id: 'builtin-haze-lowlands', name: 'Soft lowlands', builtin: true, mode: 'composable', for: ['terrain'],
    // gentle warped fBm with low gain: rounded relief, few sharp features — Titan's eroded ice bedrock
    np: np({ base:2, combinator:1, freq:1.6, octaves:6, gain:0.42, warpOn:1, warpAmt:1.2, seed:[2,9,2] }),
  },
);

const impact = [];

/* Merge built-ins with whatever the store returned. Saved entries win on id. */
function merge(builtins, saved){
  const m = new Map();
  builtins.forEach(p => m.set(p.id, p));
  (saved || []).forEach(p => m.set(p.id, p));
  return [...m.values()];
}

window.OrogenyBuiltinPresets = { noise, impact, merge };
})();
