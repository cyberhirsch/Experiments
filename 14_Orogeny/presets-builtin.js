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
