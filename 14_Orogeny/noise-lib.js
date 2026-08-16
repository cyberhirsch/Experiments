/* ══════════════════════════════════════════════════════════════════════════
   noise-lib.js — shared procedural-noise library for № 14 Orogeny.

   Loaded by both Orogeny_v003.html (the main tool) and noise-designer.html
   (the standalone preset designer), so a preset built in one renders
   identically in the other — there is exactly one copy of this math.

   Exports on window.OrogenyNoise:
     glslFor(base, comb, opts)  — GLSL specialized to one base + combinator
     glslForParams(np) / glslKey(np)
     BASE_VARIANTS, BASE_KINDS, COMBINATORS, MODIFIERS, REMAP_MODES,
     SPECIAL_KINDS, PARAM_SCHEMA, defaultParams, uploadNoiseParams,
     variantFor

   ── Why specialization, not runtime dispatch ──────────────────────────────
   The first version branched on base and combinator with runtime ifs inside
   one mega-shader. GLSL compilers unroll bounded loops and inline
   aggressively, so every branch was generated whether reachable or not:
   evalBaseUnit appeared 83 times statically, and each instance inlined the
   27-cell x 8-impulse kernelSum three times — order 5e4 kernel bodies, each
   with a hash and a transcendental. That crashed the GPU process outright:
   the program link threw, and every later WebGL2 context in the same browser
   process failed with it.

   So base, cellular output index, combinator, modifiers and the remap mode
   are all PREPROCESSOR constants. Exactly one of each survives, and the heavy
   generators are themselves guarded so an unselected base is never compiled.
   Call glslFor() per combination and cache the program.

   ── Structure ─────────────────────────────────────────────────────────────
   BASE (raw field) x COMBINATOR (octave recombination) x MODIFIERS
   (warp / curl / anisotropy / fold) x REMAP (final tone stage).

   Named techniques from the literature are mostly compositions rather than
   separate generators: marble is a sine remap over turbulence, wood is a ring
   remap, crackle is a threshold on the cellular F2-F1 output. See
   NOISE_CATALOG.md for the full map of names to compositions.

   Three families do not fit a per-pixel eval(p) signature at all (reaction-
   diffusion, diamond-square, blue-noise); they are in SPECIAL_KINDS and get
   their own preview mode in the designer, not a GLSL entry here.

   base==value with combinator==fbm/ridged and no modifiers routes through the
   ORIGINAL fbm()/ridged() verbatim, so the main tool's existing terrain init
   reproduces bit-identically rather than approximately.
   ════════════════════════════════════════════════════════════════════════ */
(function(){
'use strict';

/* base ids:  0 white  1 value  2 perlin  3 simplex  4 cellular
              5 quasicrystal  6 gabor  7 sparseConv  8 phasor
   comb ids:  0 none  1 fbm  2 turbulence  3 ridged  4 billow
              5 hybridMF  6 multifractal  7 terraced
   fIndex:    0 F1  1 F2  2 F2-F1  3 F1+F2  4 F3  5 F4  6 F1*F2  7 F1/F2
              8 cellID  9 edgeDistance  10 crackle  11 smoothF1
   remap:     0 none  1 marble  2 rings  3 strata  4 step  5 smoothstep
              6 posterize                                                   */

/* Split out on purpose. These five are DUPLICATES of what Orogeny_v003.html's
   COMMON block already declares, so injecting this library into one of the
   main tool's shaders would be a redefinition error. glslFor({omitShared:true})
   drops this half and relies on the host's copies — which are byte-identical,
   so the result is the same either way. Standalone pages take both halves. */
const GLSL_SHARED = `
float hash11(float p){ p = fract(p*0.1031); p *= p + 33.33; p *= p + p; return fract(p); }
float hash13(vec3 q){ vec3 p3 = fract(q*0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x+p3.y)*p3.z); }
float vnoise(vec3 x){
  vec3 i = floor(x), f = fract(x); vec3 u = f*f*(3.0-2.0*f);
  float a=hash13(i), b=hash13(i+vec3(1,0,0)), c=hash13(i+vec3(0,1,0)), d=hash13(i+vec3(1,1,0));
  float e=hash13(i+vec3(0,0,1)), g=hash13(i+vec3(1,0,1)), h=hash13(i+vec3(0,1,1)), k=hash13(i+vec3(1,1,1));
  return mix(mix(mix(a,b,u.x), mix(c,d,u.x), u.y), mix(mix(e,g,u.x), mix(h,k,u.x), u.y), u.z);
}
float fbm(vec3 p, int oct){
  float a=0.5, s=0.0, n=0.0;
  for(int i=0;i<8;i++){ if(i>=oct) break; s += a*vnoise(p); n += a; p *= 2.03; a *= 0.5; }
  return s/max(n,1e-5);
}
float ridged(vec3 p, int oct){
  float a=0.5, s=0.0, n=0.0;
  for(int i=0;i<8;i++){ if(i>=oct) break;
    s += a*(1.0 - abs(vnoise(p)*2.0-1.0)); n += a; p *= 2.07; a *= 0.5; }
  return s/max(n,1e-5);
}
`;

const GLSL_BODY = `
/* not in the main tool's COMMON block, so always compiled */
vec3 hash33(vec3 q){
  vec3 p3 = fract(q*vec3(0.1031, 0.1030, 0.0973));
  p3 += dot(p3, p3.yxz + 33.33);
  return fract((p3.xxy + p3.yxx)*p3.zyx);
}

struct NoiseParams {
  int   base, combinator;
  vec3  seed;
  float freq;
  int   octaves;
  float lacunarity, gain;
  int   metric, fIndex;
  float jitter, minkowskiP, crackleThr, smoothK;
  int   waveN;
  float waveAngle;
  float kernelBW, kernelOrient;
  int   warpOn, warpIter;
  float warpAmt;
  int   curlOn;
  float curlAmt;
  int   aniOn;
  float aniAngle, aniStretch;
  int   terraceSteps;
  float foldPivot, foldGain, foldSign;
  int   remapMode;
  float rFreq, rAmt, rThresh, rSoft;
};

/* ═══════════════════ base generators — one survives ═══════════════════ */

#if OG_BASE == 2
float perlinNoise(vec3 p){
  vec3 i = floor(p), f = fract(p);
  vec3 u = f*f*f*(f*(f*6.0-15.0)+10.0);
  float n000 = dot(normalize(hash33(i+vec3(0,0,0))*2.0-1.0), f-vec3(0,0,0));
  float n100 = dot(normalize(hash33(i+vec3(1,0,0))*2.0-1.0), f-vec3(1,0,0));
  float n010 = dot(normalize(hash33(i+vec3(0,1,0))*2.0-1.0), f-vec3(0,1,0));
  float n110 = dot(normalize(hash33(i+vec3(1,1,0))*2.0-1.0), f-vec3(1,1,0));
  float n001 = dot(normalize(hash33(i+vec3(0,0,1))*2.0-1.0), f-vec3(0,0,1));
  float n101 = dot(normalize(hash33(i+vec3(1,0,1))*2.0-1.0), f-vec3(1,0,1));
  float n011 = dot(normalize(hash33(i+vec3(0,1,1))*2.0-1.0), f-vec3(0,1,1));
  float n111 = dot(normalize(hash33(i+vec3(1,1,1))*2.0-1.0), f-vec3(1,1,1));
  float nx00 = mix(n000,n100,u.x), nx10 = mix(n010,n110,u.x);
  float nx01 = mix(n001,n101,u.x), nx11 = mix(n011,n111,u.x);
  return mix(mix(nx00,nx10,u.y), mix(nx01,nx11,u.y), u.z)*1.4;
}
#endif

#if OG_BASE == 3
/* standard public-domain 3D simplex (Gustavson / McEwan) */
vec3 mod289v3(vec3 x){ return x - floor(x/289.0)*289.0; }
vec4 mod289v4(vec4 x){ return x - floor(x/289.0)*289.0; }
vec4 permute289(vec4 x){ return mod289v4(((x*34.0)+1.0)*x); }
vec4 taylorInvSqrt(vec4 r){ return 1.79284291400159 - 0.85373472095314*r; }
float simplex3(vec3 v){
  const vec2 C = vec2(1.0/6.0, 1.0/3.0);
  const vec4 D = vec4(0.0, 0.5, 1.0, 2.0);
  vec3 i  = floor(v + dot(v, C.yyy));
  vec3 x0 = v - i + dot(i, C.xxx);
  vec3 g = step(x0.yzx, x0.xyz);
  vec3 l = 1.0 - g;
  vec3 i1 = min(g.xyz, l.zxy);
  vec3 i2 = max(g.xyz, l.zxy);
  vec3 x1 = x0 - i1 + C.xxx;
  vec3 x2 = x0 - i2 + C.yyy;
  vec3 x3 = x0 - D.yyy;
  i = mod289v3(i);
  vec4 p = permute289(permute289(permute289(
            i.z + vec4(0.0, i1.z, i2.z, 1.0))
          + i.y + vec4(0.0, i1.y, i2.y, 1.0))
          + i.x + vec4(0.0, i1.x, i2.x, 1.0));
  float n_ = 0.142857142857;
  vec3 ns = n_ * D.wyz - D.xzx;
  vec4 j = p - 49.0 * floor(p * ns.z * ns.z);
  vec4 x_ = floor(j * ns.z);
  vec4 y_ = floor(j - 7.0 * x_);
  vec4 x = x_ *ns.x + ns.yyyy;
  vec4 y = y_ *ns.x + ns.yyyy;
  vec4 h = 1.0 - abs(x) - abs(y);
  vec4 b0 = vec4(x.xy, y.xy);
  vec4 b1 = vec4(x.zw, y.zw);
  vec4 s0 = floor(b0)*2.0 + 1.0;
  vec4 s1 = floor(b1)*2.0 + 1.0;
  vec4 sh = -step(h, vec4(0.0));
  vec4 a0 = b0.xzyw + s0.xzyw*sh.xxyy;
  vec4 a1 = b1.xzyw + s1.xzyw*sh.zzww;
  vec3 p0 = vec3(a0.xy, h.x);
  vec3 p1 = vec3(a0.zw, h.y);
  vec3 p2 = vec3(a1.xy, h.z);
  vec3 p3 = vec3(a1.zw, h.w);
  vec4 norm = taylorInvSqrt(vec4(dot(p0,p0), dot(p1,p1), dot(p2,p2), dot(p3,p3)));
  p0 *= norm.x; p1 *= norm.y; p2 *= norm.z; p3 *= norm.w;
  vec4 m = max(0.6 - vec4(dot(x0,x0), dot(x1,x1), dot(x2,x2), dot(x3,x3)), 0.0);
  m = m * m;
  return 42.0 * dot(m*m, vec4(dot(p0,x0), dot(p1,x1), dot(p2,x2), dot(p3,x3)));
}
#endif

#if OG_BASE == 4
/* ─── cellular / Worley / Voronoi ───
   One shader base, twelve outputs. OG_FIDX picks the output at compile time,
   because the outputs need structurally different code: F1/F2 keep two
   running minima, F3/F4 need four, cell ID and edge distance need the
   identity of the nearest seed, and edge distance needs a second pass.

   The per-output normalization constants below were MEASURED, not guessed:
   the same math was reimplemented in JS and sampled 60,000 times to get the
   1st and 99th percentiles, then mapped onto [-1,1]. They are calibrated at
   the defaults (jitter 0.9, Euclidean); other metrics shift the range, and
   the final clamp absorbs the difference rather than the output blowing out. */
float metricDist(vec3 d, NoiseParams np){
  int m = np.metric;
  if (m==1) return abs(d.x)+abs(d.y)+abs(d.z);          // Manhattan  — diamond cells
  if (m==2) return max(abs(d.x), max(abs(d.y), abs(d.z)));// Chebyshev — square cells
  if (m==3) return dot(d,d);                             // Euclidean squared (no sqrt)
  if (m==4){ float pw = max(np.minkowskiP, 0.25);
    return pow(pow(abs(d.x),pw)+pow(abs(d.y),pw)+pow(abs(d.z),pw), 1.0/pw); }
  return length(d);
}
vec3 cellPoint(vec3 nc, NoiseParams np){
  return nc + 0.5 + np.jitter*(hash33(nc + np.seed) - 0.5);
}

float cellular(vec3 p, NoiseParams np){
  vec3 c = floor(p);

#if OG_FIDX == 8 || OG_FIDX == 9
  /* need the nearest seed's identity, not just its distance */
  float best = 1e9; vec3 bestC = c, bestP = p;
  for(int kk=-1; kk<=1; kk++) for(int jj=-1; jj<=1; jj++) for(int ii=-1; ii<=1; ii++){
    vec3 nc = c + vec3(float(ii),float(jj),float(kk));
    vec3 pt = cellPoint(nc, np);
    float d = metricDist(pt - p, np);
    if (d < best){ best = d; bestC = nc; bestP = pt; }
  }
  #if OG_FIDX == 8
    /* flat random value per cell — the shattered / stained-glass look. Not a
       distance field at all, which is why it cannot be had by tuning F1. */
    return hash13(bestC + np.seed)*2.0 - 1.0;
  #else
    /* TRUE distance to the Voronoi edge: the minimum distance from p to the
       perpendicular bisector between the nearest seed and each neighbour.
       F2-F1 only approximates this and is wrong near cell corners. Costs a
       second 27-cell pass. */
    float e = 1e9;
    for(int kk=-1; kk<=1; kk++) for(int jj=-1; jj<=1; jj++) for(int ii=-1; ii<=1; ii++){
      vec3 nc = c + vec3(float(ii),float(jj),float(kk));
      vec3 pt = cellPoint(nc, np);
      vec3 diff = pt - bestP;
      float L = length(diff);
      if (L < 1e-4) continue;
      e = min(e, dot(0.5*(pt + bestP) - p, diff/L));
    }
    return clamp(e*5.1059 - 1.0090, -1.0, 1.0);
  #endif

#elif OG_FIDX == 4 || OG_FIDX == 5
  float f0=1e9, f1=1e9, f2=1e9, f3=1e9;
  for(int kk=-1; kk<=1; kk++) for(int jj=-1; jj<=1; jj++) for(int ii=-1; ii<=1; ii++){
    vec3 nc = c + vec3(float(ii),float(jj),float(kk));
    float d = metricDist(cellPoint(nc, np) - p, np);
    if      (d<f0){ f3=f2; f2=f1; f1=f0; f0=d; }
    else if (d<f1){ f3=f2; f2=f1; f1=d; }
    else if (d<f2){ f3=f2; f2=d; }
    else if (d<f3){ f3=d; }
  }
  #if OG_FIDX == 4
    return clamp(f2*3.5792 - 2.9650, -1.0, 1.0);
  #else
    return clamp(f3*3.8871 - 3.6200, -1.0, 1.0);
  #endif

#elif OG_FIDX == 11
  /* smooth F1 via an exponential soft-min: no derivative discontinuity at the
     cell walls, so it lights and erodes cleanly. */
  float sum = 0.0;
  float k = max(np.smoothK, 0.5);
  for(int kk=-1; kk<=1; kk++) for(int jj=-1; jj<=1; jj++) for(int ii=-1; ii<=1; ii++){
    vec3 nc = c + vec3(float(ii),float(jj),float(kk));
    sum += exp(-k*metricDist(cellPoint(nc, np) - p, np));
  }
  return clamp((-log(max(sum,1e-9))/k)*3.2384 - 1.4136, -1.0, 1.0);

#else
  float f1=1e9, f2=1e9;
  for(int kk=-1; kk<=1; kk++) for(int jj=-1; jj<=1; jj++) for(int ii=-1; ii<=1; ii++){
    vec3 nc = c + vec3(float(ii),float(jj),float(kk));
    float d = metricDist(cellPoint(nc, np) - p, np);
    if (d<f1){ f2=f1; f1=d; } else if (d<f2){ f2=d; }
  }
  #if   OG_FIDX == 0
    return clamp(f1*2.6927 - 1.3557, -1.0, 1.0);
  #elif OG_FIDX == 1
    return clamp(f2*3.1779 - 2.2066, -1.0, 1.0);
  #elif OG_FIDX == 2
    return clamp((f2-f1)*3.0330 - 1.0077, -1.0, 1.0);
  #elif OG_FIDX == 3
    return clamp((f1+f2)*1.6878 - 2.0731, -1.0, 1.0);
  #elif OG_FIDX == 6
    return clamp((f1*f2)*2.6763 - 1.1948, -1.0, 1.0);
  #elif OG_FIDX == 7
    return clamp((f1/max(f2,1e-4))*2.4988 - 1.4892, -1.0, 1.0);
  #elif OG_FIDX == 10
    /* crackle: threshold the wall field — dry lakebed, mud cracks */
    return smoothstep(np.crackleThr + 0.03, np.crackleThr - 0.03, f2-f1)*2.0 - 1.0;
  #else
    return clamp(f1*2.6927 - 1.3557, -1.0, 1.0);
  #endif
#endif
}
#endif

#if OG_BASE == 5
float quasicrystal(vec3 p, NoiseParams np){
  float s = 0.0; int N = clamp(np.waveN, 2, 16);
  for(int i=0;i<16;i++){
    if (i>=N) break;
    float ang = np.waveAngle + float(i)*3.14159265/float(N);
    vec3 dir = normalize(vec3(cos(ang), sin(ang), 0.35*sin(ang*1.7)));
    s += cos(dot(p, dir)*6.2831853);
  }
  return s/float(N);
}
#endif

#if OG_BASE >= 6
/* Shared per-cell kernel-sum scaffold for gabor / sparse-convolution /
   phasor: realtime bounded-impulse approximations, not the exact analytic
   forms. OG_KMODE picks the carrier so only one variant is compiled.
   Impulses are capped at 4 over a 3^3 neighbourhood — 108 unrolled bodies,
   affordable only because this is now instantiated once per octave rather
   than 83 times over. */
#define OG_KMAX 4
float kernelSum(vec3 p, NoiseParams np){
  vec3 c = floor(p);
  float sum = 0.0;
  int M = clamp(np.waveN, 1, OG_KMAX);
  float bw = max(np.kernelBW, 0.05);
  for(int kk=-1; kk<=1; kk++) for(int jj=-1; jj<=1; jj++) for(int ii=-1; ii<=1; ii++){
    vec3 nc = c + vec3(float(ii),float(jj),float(kk));
    for(int m=0; m<OG_KMAX; m++){
      if (m>=M) break;
      vec3 h = hash33(nc*17.0 + float(m)*3.7 + np.seed);
      vec3 d = p - (nc + h);
      if (dot(d,d) > 4.0) continue;
      float g = exp(-3.14159265*bw*bw*dot(d,d));
      #if OG_KMODE == 1
        sum += g*(h.z*2.0-1.0);
      #else
        #if OG_KMODE == 2
          float orient = np.kernelOrient + h.y*6.2831853;
        #else
          float orient = np.kernelOrient;
        #endif
        vec3 dir = vec3(cos(orient), sin(orient), 0.0);
        sum += g*cos(6.2831853*np.freq*dot(d,dir));
      #endif
    }
  }
  return sum;
}
#endif

/* one base, chosen at compile time */
float evalBaseUnit(vec3 p, NoiseParams np){
#if   OG_BASE == 0
  return hash13(p)*2.0-1.0;
#elif OG_BASE == 1
  return vnoise(p)*2.0-1.0;
#elif OG_BASE == 2
  return perlinNoise(p);
#elif OG_BASE == 3
  return simplex3(p);
#elif OG_BASE == 4
  return cellular(p, np);
#elif OG_BASE == 5
  return quasicrystal(p, np);
#else
  return clamp(kernelSum(p, np)*1.6, -1.0, 1.0);
#endif
}

/* ═══════════════════ combinator — one survives ═══════════════════ */

#if OG_COMB == 1 || OG_COMB == 7
float combFbm(vec3 p, NoiseParams np){
  float amp=0.5, s=0.0, n=0.0; vec3 q=p;
  for(int i=0;i<8;i++){ if(i>=np.octaves) break; s += amp*evalBaseUnit(q,np); n += amp; q *= np.lacunarity; amp *= np.gain; }
  return s/max(n,1e-5);
}
#endif
#if OG_COMB == 2
float combTurbulence(vec3 p, NoiseParams np){
  float amp=0.5, s=0.0, n=0.0; vec3 q=p;
  for(int i=0;i<8;i++){ if(i>=np.octaves) break; s += amp*abs(evalBaseUnit(q,np)); n += amp; q *= np.lacunarity; amp *= np.gain; }
  return s/max(n,1e-5)*2.0-1.0;
}
#endif
#if OG_COMB == 3
float combRidged(vec3 p, NoiseParams np){
  float amp=0.5, s=0.0, n=0.0; vec3 q=p;
  for(int i=0;i<8;i++){ if(i>=np.octaves) break; s += amp*(1.0-abs(evalBaseUnit(q,np))); n += amp; q *= np.lacunarity; amp *= np.gain; }
  return s/max(n,1e-5)*2.0-1.0;
}
#endif
#if OG_COMB == 4
float combBillow(vec3 p, NoiseParams np){
  float amp=0.5, s=0.0, n=0.0; vec3 q=p;
  for(int i=0;i<8;i++){ if(i>=np.octaves) break; s += amp*(2.0*abs(evalBaseUnit(q,np))-1.0); n += amp; q *= np.lacunarity; amp *= np.gain; }
  return s/max(n,1e-5);
}
#endif
#if OG_COMB == 5
float combHybridMF(vec3 p, NoiseParams np){
  float offset=0.7, weight=1.0, s=0.0, amp=1.0; vec3 q=p;
  for(int i=0;i<8;i++){
    if(i>=np.octaves) break;
    float sig = (evalBaseUnit(q,np)+offset)*amp;
    s += weight*sig;
    weight = clamp(weight*clamp(sig,0.0,1.0), 0.0, 1.0);
    q *= np.lacunarity; amp *= np.gain;
  }
  return clamp(s*0.5-0.5, -1.0, 1.0);
}
#endif
#if OG_COMB == 6
float combMultifractal(vec3 p, NoiseParams np){
  float s=1.0, amp=1.0; vec3 q=p;
  for(int i=0;i<8;i++){ if(i>=np.octaves) break; s *= (evalBaseUnit(q,np)*amp*0.5+1.0); q *= np.lacunarity; amp *= np.gain; }
  return clamp(s-1.0, -1.0, 1.0);
}
#endif
#if OG_COMB == 7
float combTerraced(vec3 p, NoiseParams np){
  float r = combFbm(p, np);
  float steps = float(max(np.terraceSteps,2));
  return floor((r*0.5+0.5)*steps)/steps*2.0-1.0;
}
#endif

/* ═══════════════════ modifiers ═══════════════════
   The warp field is a fixed cheap value-noise fBm, NOT the selected base.
   Running cellular or a kernel base through the warp cost 18 extra
   instantiations of the heaviest code in the file and changed the result very
   little — a warp wants to be smooth and cheap. */
vec3 applyAniso(vec3 p, float angle, float stretch){
  float c=cos(angle), s=sin(angle);
  vec2 xy = vec2(c*p.x+s*p.y, -s*p.x+c*p.y);
  xy.x *= max(stretch, 0.05);
  return vec3(xy, p.z);
}
#if OG_WARP
vec3 domainWarp(vec3 p, NoiseParams np){
  vec3 q = p;
  int iters = clamp(np.warpIter, 1, 4);
  for(int i=0;i<4;i++){
    if (i>=iters) break;
    vec3 w = vec3(fbm(q*0.7 + vec3( 3.1,  1.7,  9.2), 3),
                  fbm(q*0.7 + vec3(13.7,  5.3,  2.1), 3),
                  fbm(q*0.7 + vec3(29.3, 17.9,  4.4), 3)) - 0.5;
    q = q + w*np.warpAmt;
  }
  return q;
}
#endif
#if OG_CURL
vec3 curlWarp(vec3 p, NoiseParams np){
  float e = 0.05;
  float n1 = fbm(p+vec3(0,e,0),3), n2 = fbm(p-vec3(0,e,0),3);
  float n3 = fbm(p+vec3(0,0,e),3), n4 = fbm(p-vec3(0,0,e),3);
  float n5 = fbm(p+vec3(e,0,0),3), n6 = fbm(p-vec3(e,0,0),3);
  vec3 c = vec3((n1-n2)-(n3-n4), (n3-n4)-(n5-n6), (n5-n6)-(n1-n2)) / (2.0*e);
  return p + c*np.curlAmt;
}
#endif

/* ═══════════════════ fold (solarize) ═══════════════════
   The V. One abs about a pivot, applied ONCE to the finished value — four ALU
   ops total, independent of base, combinator and octave count.

     out = (abs(v - pivot)*gain - 1) * sign

   At v == pivot the result is -1 and it rises either side, so the entire level
   set where the field equals the pivot becomes a crease. sign = -1 flips it to
   a light crest. Not the same as the RIDGED combinator, which folds inside
   every octave and creases self-similarly at all scales; this folds only at
   the final scale — coarser, but free. */
#if OG_FOLD
float applyFold(float v, NoiseParams np){
  return clamp((abs(v - np.foldPivot)*np.foldGain - 1.0)*np.foldSign, -1.0, 1.0);
}
#endif

/* ═══════════════════ remap — the final tone stage ═══════════════════
   Takes the value AND the position, because the classic composites are
   functions of both: marble is a sine of a coordinate displaced by noise, not
   a sine of the noise. This is what turns one fBm into marble, wood, strata
   or crackle for a handful of ALU ops. */
#if OG_REMAP
/* 1 marble/veining · 2 wood rings · 3 strata · 4 hard threshold
   5 soft threshold · 6 posterize                                */
float applyRemap(float v, vec3 q, NoiseParams np){
  #if   OG_REMAP == 1
    return sin((q.x*np.rFreq + v*np.rAmt)*6.2831853);
  #elif OG_REMAP == 2
    return sin((length(q)*np.rFreq + v*np.rAmt)*6.2831853);
  #elif OG_REMAP == 3
    return fract(q.y*np.rFreq + v*np.rAmt)*2.0 - 1.0;
  #elif OG_REMAP == 4
    return step(np.rThresh, v)*2.0 - 1.0;
  #elif OG_REMAP == 5
    return smoothstep(np.rThresh - np.rSoft, np.rThresh + np.rSoft, v)*2.0 - 1.0;
  #else
    float st = float(max(np.terraceSteps, 2));
    return floor((v*0.5+0.5)*st)/st*2.0 - 1.0;
  #endif
}
#endif

/* ═══════════════════ entry point ═══════════════════ */
float evalNoise(vec3 p, NoiseParams np){
  vec3 q = p*np.freq + np.seed;
  float v;

#if OG_BASE == 1 && !OG_WARP && !OG_CURL && !OG_ANISO && (OG_COMB == 1 || OG_COMB == 3)
  /* bit-identical fast path: the original fbm()/ridged() on value noise, so
     the main tool's existing terrain init reproduces exactly. Fold and remap,
     when enabled, are applied after and do not disturb that equivalence. */
  #if OG_COMB == 1
    v = fbm(q, np.octaves)*2.0-1.0;
  #else
    v = ridged(q, np.octaves)*2.0-1.0;
  #endif
#else
  #if OG_ANISO
    q = applyAniso(q, np.aniAngle, np.aniStretch);
  #endif
  #if OG_WARP
    q = domainWarp(q, np);
  #endif
  #if OG_CURL
    q = curlWarp(q, np);
  #endif

  #if   OG_COMB == 1
    v = combFbm(q, np)*2.0-1.0;
  #elif OG_COMB == 2
    v = combTurbulence(q, np);
  #elif OG_COMB == 3
    v = combRidged(q, np)*2.0-1.0;
  #elif OG_COMB == 4
    v = combBillow(q, np);
  #elif OG_COMB == 5
    v = combHybridMF(q, np);
  #elif OG_COMB == 6
    v = combMultifractal(q, np);
  #elif OG_COMB == 7
    v = combTerraced(q, np);
  #else
    v = evalBaseUnit(q, np);
  #endif
#endif

#if OG_FOLD
  v = applyFold(v, np);
#endif
#if OG_REMAP
  v = applyRemap(v, q, np);
#endif
  return v;
}

#if OG_WANT_DERIV
/* Finite-difference gradient. Costs SEVEN evalNoise instantiations, so it is
   off unless explicitly requested — it was a silent 7x on the whole shader. */
vec4 evalNoiseD(vec3 p, NoiseParams np){
  float e = 0.01;
  float v = evalNoise(p, np);
  float dx = (evalNoise(p+vec3(e,0,0),np)-evalNoise(p-vec3(e,0,0),np))/(2.0*e);
  float dy = (evalNoise(p+vec3(0,e,0),np)-evalNoise(p-vec3(0,e,0),np))/(2.0*e);
  float dz = (evalNoise(p+vec3(0,0,e),np)-evalNoise(p-vec3(0,0,e),np))/(2.0*e);
  return vec4(v, dx, dy, dz);
}
#endif
`;

function glslFor(base, comb, opts){
  opts = opts || {};
  const kmode = base === 7 ? 1 : (base === 8 ? 2 : 0);
  return [
    opts.omitShared ? '' : GLSL_SHARED,
    '#define OG_BASE '  + (base|0),
    '#define OG_COMB '  + (comb|0),
    '#define OG_FIDX '  + (opts.fIndex|0),
    '#define OG_KMODE ' + kmode,
    '#define OG_WARP '  + (opts.warp  ? 1 : 0),
    '#define OG_CURL '  + (opts.curl  ? 1 : 0),
    '#define OG_ANISO ' + (opts.aniso ? 1 : 0),
    '#define OG_FOLD '  + (opts.fold  ? 1 : 0),
    '#define OG_REMAP ' + (opts.remap|0),
    '#define OG_WANT_DERIV ' + (opts.deriv ? 1 : 0),
    GLSL_BODY,
  ].join('\n');
}
/* Program cache key. fIndex only participates when the base is cellular, so
   the other bases do not fragment the cache into twelve identical programs. */
function glslKey(np){
  const fi = (np.base === 4) ? (np.fIndex|0) : 0;
  return [np.base, np.combinator, fi, np.warpOn?1:0, np.curlOn?1:0,
          np.aniOn?1:0, np.foldOn?1:0, np.remapMode|0].join('_');
}
function glslForParams(np, opts){
  opts = (typeof opts === 'boolean') ? { deriv: opts } : (opts || {});
  return glslFor(np.base, np.combinator, {
    fIndex: (np.base === 4) ? np.fIndex : 0,
    warp: !!np.warpOn, curl: !!np.curlOn, aniso: !!np.aniOn,
    fold: !!np.foldOn, remap: np.remapMode|0,
    deriv: !!opts.deriv, omitShared: !!opts.omitShared,
  });
}

/* ═══════════════════════════ JS-side catalog ═══════════════════════════ */

/* GLSL-level bases. The UI does NOT list these directly — see BASE_VARIANTS. */
const BASE_KINDS = [
  { id:0, key:'white',        label:'White noise' },
  { id:1, key:'value',        label:'Value' },
  { id:2, key:'perlin',       label:'Perlin (gradient)' },
  { id:3, key:'simplex',      label:'Simplex' },
  { id:4, key:'cellular',     label:'Cellular / Voronoi' },
  { id:5, key:'quasicrystal', label:'Quasicrystal' },
  { id:6, key:'gabor',        label:'Gabor' },
  { id:7, key:'sparseConv',   label:'Sparse convolution' },
  { id:8, key:'phasor',       label:'Phasor' },
];

/* What the dropdown actually shows. The cellular family is twelve first-class
   entries rather than one entry hiding an F-index sub-select — the variants
   already existed, they were simply unreachable without knowing to look. */
const CELL_PARAMS = ['metric','jitter'];
const BASE_VARIANTS = [
  { key:'white',   label:'White noise',        base:0, params:[] },
  { key:'value',   label:'Value',               base:1, params:[] },
  { key:'perlin',  label:'Perlin (gradient)',   base:2, params:[] },
  { key:'simplex', label:'Simplex',             base:3, params:[] },

  { key:'cellF1',    label:'Cellular · F1 (cells)',      base:4, fIndex:0,  params:CELL_PARAMS },
  { key:'cellF2',    label:'Cellular · F2',               base:4, fIndex:1,  params:CELL_PARAMS },
  { key:'cellEdge',  label:'Cellular · F2−F1 (cracks)',   base:4, fIndex:2,  params:CELL_PARAMS },
  { key:'cellSum',   label:'Cellular · F1+F2',            base:4, fIndex:3,  params:CELL_PARAMS },
  { key:'cellF3',    label:'Cellular · F3',               base:4, fIndex:4,  params:CELL_PARAMS },
  { key:'cellF4',    label:'Cellular · F4',               base:4, fIndex:5,  params:CELL_PARAMS },
  { key:'cellMul',   label:'Cellular · F1×F2',            base:4, fIndex:6,  params:CELL_PARAMS },
  { key:'cellDiv',   label:'Cellular · F1/F2',            base:4, fIndex:7,  params:CELL_PARAMS },
  { key:'voronoiId', label:'Voronoi · cell ID (shatter)', base:4, fIndex:8,  params:CELL_PARAMS,
    note:'Flat random value per cell — not a distance field. The stained-glass look.' },
  { key:'voronoiEdge', label:'Voronoi · true edge distance', base:4, fIndex:9, params:CELL_PARAMS,
    note:'Exact distance to the cell bisector. F2−F1 only approximates this and is wrong near corners. Costs a second 27-cell pass.' },
  { key:'crackle',   label:'Cellular · crackle',          base:4, fIndex:10, params:CELL_PARAMS.concat(['crackleThr']),
    note:'Thresholded wall field — mud cracks, dry lakebed.' },
  { key:'cellSmooth',label:'Cellular · smooth F1',        base:4, fIndex:11, params:CELL_PARAMS.concat(['smoothK']),
    note:'Exponential soft-min: no derivative discontinuity at the walls.' },

  { key:'quasicrystal',label:'Quasicrystal',   base:5, params:['waveN','waveAngle'] },
  { key:'gabor',       label:'Gabor',          base:6, params:['waveN','kernelBW','kernelOrient'],
    note:'Bounded-impulse approximation (Lagae et al.), max 4 impulses per cell.' },
  { key:'sparseConv',  label:'Sparse convolution', base:7, params:['waveN','kernelBW'],
    note:'Isotropic weighted-Gaussian impulses, no carrier wave.' },
  { key:'phasor',      label:'Phasor',          base:8, params:['waveN','kernelBW','kernelOrient'],
    note:'Kernel-sum with randomized per-impulse orientation — the spirit of Tricard et al., not the exact formulation.' },
];
/* which dropdown entry a params object corresponds to */
function variantFor(np){
  return BASE_VARIANTS.find(v => v.base === np.base &&
           (v.base !== 4 || (v.fIndex|0) === (np.fIndex|0))) || BASE_VARIANTS[1];
}

const COMBINATORS = [
  { id:0, key:'none',        label:'None (raw base)',       params:[] },
  { id:1, key:'fbm',         label:'fBm',                    params:['octaves','lacunarity','gain'] },
  { id:2, key:'turbulence',  label:'Turbulence',             params:['octaves','lacunarity','gain'] },
  { id:3, key:'ridged',      label:'Ridged multifractal',    params:['octaves','lacunarity','gain'] },
  { id:4, key:'billow',      label:'Billow',                 params:['octaves','lacunarity','gain'] },
  { id:5, key:'hybridMF',    label:'Hybrid multifractal',    params:['octaves','lacunarity','gain'] },
  { id:6, key:'multifractal',label:'Multifractal',           params:['octaves','lacunarity','gain'] },
  { id:7, key:'terraced',    label:'Terraced',                params:['octaves','lacunarity','gain','terraceSteps'] },
];

const MODIFIERS = [
  { key:'warp',  onKey:'warpOn',  label:'Domain warp',  params:['warpAmt','warpIter'] },
  { key:'curl',  onKey:'curlOn',  label:'Curl warp',     params:['curlAmt'] },
  { key:'aniso', onKey:'aniOn',   label:'Anisotropy',    params:['aniAngle','aniStretch'] },
  { key:'fold',  onKey:'foldOn',  label:'Fold / solarize', params:['foldPivot','foldGain','foldSign'],
    note:'One abs about a pivot, 4 ALU ops, applied once to the finished value.' },
];

/* final tone stage — turns one fBm into the classic composites */
const REMAP_MODES = [
  { id:0, key:'none',       label:'None',                 params:[] },
  { id:1, key:'marble',     label:'Marble / veining',      params:['rFreq','rAmt'] },
  { id:2, key:'rings',      label:'Wood rings',            params:['rFreq','rAmt'] },
  { id:3, key:'strata',     label:'Strata / banding',      params:['rFreq','rAmt'] },
  { id:4, key:'step',       label:'Threshold (hard)',      params:['rThresh'] },
  { id:5, key:'smoothstep', label:'Threshold (soft)',      params:['rThresh','rSoft'] },
  { id:6, key:'posterize',  label:'Posterize',             params:['terraceSteps'] },
];

/* don't fit evalNoise(p, np) — own preview mode in the designer only */
const SPECIAL_KINDS = [
  { key:'reactionDiffusion', label:'Reaction-diffusion (Gray-Scott)',
    note:'Simulated: runs a feed/kill ping-pong pass. The preset stores parameters and re-simulates on load, not a snapshot.',
    params:['rdFeed','rdKill','rdDiffA','rdDiffB','rdSteps'] },
  { key:'diamondSquare', label:'Diamond-square',
    note:'Grid precompute on the CPU, not a per-pixel function.', params:['dsSeed','dsRoughness','dsSize'] },
  { key:'blueNoise', label:'Blue noise / Poisson-disk',
    note:'Point scatter via dart-throwing, previewed as a scatter plot rather than a height field.', params:['bnRadius','bnSeed'] },
  { key:'wangTiles', label:'Wang tiles / histogram-preserving stochastic tiling', stub:true,
    note:'Not implemented. Retiles an INPUT bitmap for seamless repetition (Heitz–Neyret); a texture-authoring workflow, not a generator, so it has no closed-form place here. Listed for completeness.' },
];

const PARAM_SCHEMA = {
  metric:       { label:'Distance metric', type:'select', default:0,
                  options:[[0,'Euclidean (round)'],[1,'Manhattan (diamond)'],[2,'Chebyshev (square)'],
                           [3,'Euclidean² (no sqrt)'],[4,'Minkowski p']] },
  minkowskiP:   { label:'Minkowski p',     type:'range', min:0.3, max:6, step:0.05, default:2 },
  jitter:       { label:'Jitter',          type:'range', min:0, max:1, step:0.01, default:0.9 },
  crackleThr:   { label:'Crack width',     type:'range', min:0.01, max:0.5, step:0.005, default:0.08 },
  smoothK:      { label:'Smoothing',       type:'range', min:1, max:24, step:0.5, default:8 },
  waveN:        { label:'Impulses / waves',type:'range', min:1, max:16, step:1, default:4 },
  waveAngle:    { label:'Angle offset',    type:'range', min:0, max:6.283, step:0.01, default:0 },
  kernelBW:     { label:'Bandwidth',       type:'range', min:0.05, max:2, step:0.01, default:0.4 },
  kernelOrient: { label:'Orientation',     type:'range', min:0, max:6.283, step:0.01, default:0 },
  octaves:      { label:'Octaves',         type:'range', min:1, max:8, step:1, default:5 },
  lacunarity:   { label:'Lacunarity',      type:'range', min:1.5, max:3.0, step:0.01, default:2.03 },
  gain:         { label:'Gain',            type:'range', min:0.2, max:0.8, step:0.01, default:0.5 },
  terraceSteps: { label:'Steps',           type:'range', min:2, max:16, step:1, default:6 },
  freq:         { label:'Frequency',       type:'range', min:0.2, max:50, step:0.1, default:2.2 },
  warpAmt:      { label:'Warp amount',     type:'range', min:0, max:3, step:0.01, default:0.9 },
  warpIter:     { label:'Warp iterations', type:'range', min:1, max:4, step:1, default:1 },
  curlAmt:      { label:'Curl warp',       type:'range', min:0, max:2, step:0.01, default:0.6 },
  aniAngle:     { label:'Anisotropy angle',type:'range', min:0, max:6.283, step:0.01, default:0 },
  aniStretch:   { label:'Anisotropy stretch', type:'range', min:0.2, max:5, step:0.01, default:2.5 },
  foldPivot:    { label:'Fold pivot',      type:'range', min:-1, max:1, step:0.01, default:0 },
  foldGain:     { label:'Fold gain',       type:'range', min:0.2, max:6, step:0.05, default:2 },
  foldSign:     { label:'Crease',          type:'select', options:[[1,'Dark (V)'],[-1,'Light (Λ)']], default:1 },
  rFreq:        { label:'Remap frequency', type:'range', min:0.05, max:6, step:0.05, default:1 },
  rAmt:         { label:'Noise influence', type:'range', min:0, max:4, step:0.02, default:1 },
  rThresh:      { label:'Threshold',       type:'range', min:-1, max:1, step:0.01, default:0 },
  rSoft:        { label:'Softness',        type:'range', min:0.01, max:1, step:0.01, default:0.15 },
  rdFeed:       { label:'Feed rate',       type:'range', min:0.01, max:0.09, step:0.001, default:0.037 },
  rdKill:       { label:'Kill rate',       type:'range', min:0.03, max:0.08, step:0.001, default:0.06 },
  rdDiffA:      { label:'Diffusion A',     type:'range', min:0.5, max:1.2, step:0.01, default:1.0 },
  rdDiffB:      { label:'Diffusion B',     type:'range', min:0.2, max:0.8, step:0.01, default:0.5 },
  rdSteps:      { label:'Sim steps',       type:'range', min:100, max:4000, step:50, default:1200 },
  dsSeed:       { label:'Seed',            type:'range', min:0, max:9999, step:1, default:1 },
  dsRoughness:  { label:'Roughness',       type:'range', min:0.1, max:1.2, step:0.01, default:0.55 },
  dsSize:       { label:'Grid size (2^n+1)', type:'select', options:[[65,'65'],[129,'129'],[257,'257']], default:129 },
  bnRadius:     { label:'Min spacing',     type:'range', min:0.01, max:0.15, step:0.005, default:0.04 },
  bnSeed:       { label:'Seed',            type:'range', min:0, max:9999, step:1, default:1 },
};

function defaultParams(){
  const d = k => PARAM_SCHEMA[k].default;
  return {
    base:1, combinator:1, seed:[0,0,0], freq:d('freq'),
    octaves:d('octaves'), lacunarity:d('lacunarity'), gain:d('gain'),
    metric:0, fIndex:0, jitter:d('jitter'), minkowskiP:d('minkowskiP'),
    crackleThr:d('crackleThr'), smoothK:d('smoothK'),
    waveN:d('waveN'), waveAngle:0, kernelBW:d('kernelBW'), kernelOrient:0,
    warpOn:0, warpIter:d('warpIter'), warpAmt:d('warpAmt'),
    curlOn:0, curlAmt:d('curlAmt'),
    aniOn:0, aniAngle:0, aniStretch:d('aniStretch'),
    terraceSteps:d('terraceSteps'),
    foldOn:0, foldPivot:d('foldPivot'), foldGain:d('foldGain'), foldSign:d('foldSign'),
    remapMode:0, rFreq:d('rFreq'), rAmt:d('rAmt'), rThresh:d('rThresh'), rSoft:d('rSoft'),
  };
}

/* Uploads one NoiseParams struct to a `uniform NoiseParams <name>;`.
   Fields the specialized shader does not reference have no location and are
   skipped — expected, not an error. */
function uploadNoiseParams(gl, prog, name, np){
  const p = Object.assign(defaultParams(), np);
  const loc = (f) => gl.getUniformLocation(prog, name + '.' + f);
  const set1i = (f,v) => { const l=loc(f); if(l) gl.uniform1i(l, v|0); };
  const set1f = (f,v) => { const l=loc(f); if(l) gl.uniform1f(l, +v); };
  set1i('base', p.base); set1i('combinator', p.combinator);
  const ls = loc('seed'); if (ls) gl.uniform3f(ls, p.seed[0]||0, p.seed[1]||0, p.seed[2]||0);
  set1f('freq', p.freq); set1i('octaves', p.octaves); set1f('lacunarity', p.lacunarity); set1f('gain', p.gain);
  set1i('metric', p.metric); set1i('fIndex', p.fIndex);
  set1f('jitter', p.jitter); set1f('minkowskiP', p.minkowskiP);
  set1f('crackleThr', p.crackleThr); set1f('smoothK', p.smoothK);
  set1i('waveN', p.waveN); set1f('waveAngle', p.waveAngle);
  set1f('kernelBW', p.kernelBW); set1f('kernelOrient', p.kernelOrient);
  set1i('warpOn', p.warpOn); set1i('warpIter', p.warpIter); set1f('warpAmt', p.warpAmt);
  set1i('curlOn', p.curlOn); set1f('curlAmt', p.curlAmt);
  set1i('aniOn', p.aniOn); set1f('aniAngle', p.aniAngle); set1f('aniStretch', p.aniStretch);
  set1i('terraceSteps', p.terraceSteps);
  set1f('foldPivot', p.foldPivot); set1f('foldGain', p.foldGain); set1f('foldSign', p.foldSign);
  set1i('remapMode', p.remapMode);
  set1f('rFreq', p.rFreq); set1f('rAmt', p.rAmt); set1f('rThresh', p.rThresh); set1f('rSoft', p.rSoft);
}

window.OrogenyNoise = {
  BASE_KINDS, BASE_VARIANTS, COMBINATORS, MODIFIERS, REMAP_MODES, SPECIAL_KINDS,
  PARAM_SCHEMA, defaultParams, uploadNoiseParams,
  glslFor, glslForParams, glslKey, variantFor,
};
})();
