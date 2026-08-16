/* ══════════════════════════════════════════════════════════════════════════
   impact-designer.js — crater-shape popup for № 14 Orogeny.

   Same document as Orogeny_v003.html (loaded via <script src>), so hand-off
   to the main tool is a direct callback, not cross-window messaging. Presets
   still persist to localStorage['orogeny.impactPresets'] so they survive a
   reload, but live sync doesn't need the `storage` event the way the noise
   designer (a separate page/tab) does.

   The 9 params here are exactly the constants that were hardcoded in
   FS_IMPACT (Orogeny_v003.html, the crater shader) and mirrored by hand in
   strike()'s CPU mass-balance integral. Defaults reproduce those original
   literals, so "no preset selected" stays pixel-identical to prior behavior.

   Usage: window.openImpactDesigner({ current, onApply, onStrike })
     current  — params object to preselect (or omitted for defaults)
     onApply  — called with the shape-params object when the user applies
     onStrike — optional, called with the shape-params object to fire a live
                test crater on the globe without closing the popup
   ════════════════════════════════════════════════════════════════════════ */
(function(){
'use strict';

const STORE_KEY = 'orogeny.impactPresets';

const DEFAULTS = {
  bowlExp: 2.0, peakWidthFrac: 0.22, peakHeightFrac: 0.45, peakCutoff: 0.035,
  rimStart: 0.75, rimEnd: 1.0, rimGain: 0.9, ejectaExp: -3.0, rMaxMult: 3.0, depthRadiusRatio: 0.22,
};

const SCHEMA = [
  { key:'bowlExp',         label:'Bowl exponent',        min:1.0, max:4.0,  step:0.05, help:'Parabola power. 2 = classic parabolic bowl; higher flattens the floor and steepens the wall.' },
  { key:'peakWidthFrac',   label:'Peak width (·R)',      min:0.05,max:0.6,  step:0.01, help:'Central-peak Gaussian width as a fraction of crater radius.' },
  { key:'peakHeightFrac',  label:'Peak height (·depth)', min:0.0, max:1.0,  step:0.01, help:'Central-peak height as a fraction of bowl depth.' },
  { key:'peakCutoff',      label:'Peak cutoff radius',   min:0.0, max:0.10, step:0.001,help:'Below this absolute radius, no central peak forms — simple bowl craters vs. complex ones, as in real crater morphology.' },
  { key:'rimStart',        label:'Rim start (·R)',       min:0.4, max:0.95, step:0.01, help:'Where the raised rim begins to rise, as a fraction of R.' },
  { key:'rimEnd',          label:'Rim end (·R)',         min:0.95,max:1.3,  step:0.01, help:'Where the rim reaches full height (≈R = crater edge).' },
  { key:'rimGain',         label:'Rim gain',             min:0.0, max:1.5,  step:0.01, help:'Rim height as a fraction of ejecta height E.' },
  { key:'ejectaExp',       label:'Ejecta falloff power', min:-5.0,max:-1.0, step:0.1,  help:'Power-law exponent for the ejecta blanket outside the rim. More negative = falls off faster.' },
  { key:'rMaxMult',        label:'Ejecta outer radius (·R)', min:1.5,max:6.0, step:0.1, help:'Where the ejecta blanket fully tapers to zero, as a multiple of R.' },
  { key:'depthRadiusRatio',label:'Depth / radius ratio', min:0.05,max:0.4,  step:0.01, help:'Default bowl depth as a fraction of radius, when depth is not given explicitly (auto-bombardment always uses this).' },
];

/* Presets go through the shared store (a folder of .json files when one has
   been picked, localStorage otherwise). The host passes it in; this fallback
   keeps the module usable standalone, with the same async shape so the calling
   code does not care which it got. */
function localFallbackStore(){
  const read = () => { try { const v = JSON.parse(localStorage.getItem(STORE_KEY) || '[]'); return Array.isArray(v) ? v : []; } catch(e){ return []; } };
  const write = l => localStorage.setItem(STORE_KEY, JSON.stringify(l));
  return {
    async list(){ return read(); },
    async save(p){
      const q = Object.assign({}, p);
      if (!q.id) q.id = 'i' + Date.now().toString(36) + Math.random().toString(36).slice(2,7);
      const l = read(); const i = l.findIndex(x => x.id === q.id);
      if (i >= 0) l[i] = q; else l.push(q);
      write(l); return q;
    },
    async remove(id){ write(read().filter(x => x.id !== id)); },
  };
}

/* mirrors FS_IMPACT's dh(a) exactly, normalized to R=1 (x = a/R) */
function smoothstepJs(a,b,x){ const t = Math.min(1, Math.max(0, (x-a)/(b-a))); return t*t*(3-2*t); }
function profile(params, x, D, P, E){
  const p = Object.assign({}, DEFAULTS, params);
  if (x < 1){
    let dh = -D*(1 - Math.pow(x, p.bowlExp));
    const ps = p.peakWidthFrac;                       // in units of R
    dh += P*Math.exp(-(x*x)/(ps*ps));
    dh += E*smoothstepJs(p.rimStart, p.rimEnd, x)*p.rimGain;
    return dh;
  }
  return E*Math.pow(x, p.ejectaExp)*smoothstepJs(p.rMaxMult, 1, x);
}

function ensureStyles(){
  if (document.getElementById('impDesignerStyle')) return;
  const s = document.createElement('style'); s.id = 'impDesignerStyle';
  s.textContent = `
    #impBackdrop{position:fixed;inset:0;z-index:100;background:rgba(2,3,5,0.72);
      display:flex;align-items:center;justify-content:center}
    #impModal{width:min(760px,92vw);max-height:88vh;overflow-y:auto;
      background:rgba(5,7,10,0.96);border:1px solid var(--line,rgba(230,236,242,0.10));
      padding:20px 24px;box-shadow:0 20px 60px rgba(0,0,0,0.6)}
    #impModal h2{font-family:Georgia,'Cormorant Garamond',serif;font-style:italic;font-weight:400;
      font-size:17px;color:var(--accent,#8fc8e8);margin-bottom:4px}
    #impModal .sub{font-size:8.5px;letter-spacing:.2em;text-transform:uppercase;color:var(--dim,#7a8794);margin-bottom:14px}
    #impGrid{display:grid;grid-template-columns:1fr 1fr;gap:0 22px}
    #impGrid .row{margin:9px 0}
    #impGrid label{display:flex;justify-content:space-between;font-size:9.5px;letter-spacing:.1em;
      text-transform:uppercase;color:var(--dim,#7a8794);margin-bottom:4px}
    #impGrid label b{color:var(--ink,#e6ecf2);font-weight:400}
    #impGrid .help{font-size:8px;line-height:1.5;color:var(--dim,#7a8794);opacity:.65;margin:2px 0 0}
    #impPreviewWrap{margin:14px 0}
    #impCanvas{width:100%;height:200px;background:rgba(255,255,255,0.02);border:1px solid var(--line,rgba(230,236,242,0.10))}
    #impPresetRow{display:flex;gap:8px;align-items:center;margin:10px 0}
    #impPresetRow input[type=text]{flex:1;background:rgba(5,7,10,0.9);border:1px solid var(--line,rgba(230,236,242,0.10));
      color:var(--ink,#e6ecf2);font-family:inherit;font-size:10px;padding:6px 8px;outline:none}
    #impPresetList{display:flex;flex-wrap:wrap;gap:6px;margin:8px 0}
    #impPresetList button{flex:0 0 auto;min-width:0}
    #impActions{display:flex;gap:8px;margin-top:16px;justify-content:flex-end}
  `;
  document.head.appendChild(s);
}

function buildRow(container, key, params, onChange){
  const spec = SCHEMA.find(s => s.key === key);
  const row = document.createElement('div'); row.className = 'row';
  const lab = document.createElement('label');
  const b = document.createElement('b'); b.textContent = (+params[key]).toFixed(3);
  lab.appendChild(document.createTextNode(spec.label + ' ')); lab.appendChild(b);
  row.appendChild(lab);
  const inp = document.createElement('input'); inp.type = 'range';
  inp.min = spec.min; inp.max = spec.max; inp.step = spec.step; inp.value = params[key];
  inp.oninput = () => { params[key] = parseFloat(inp.value); b.textContent = params[key].toFixed(3); onChange(); };
  row.appendChild(inp);
  const help = document.createElement('div'); help.className = 'help'; help.textContent = spec.help;
  row.appendChild(help);
  container.appendChild(row);
}

function drawPreview(canvas, params, previewR){
  const ctx = canvas.getContext('2d');
  const dpr = Math.min(window.devicePixelRatio||1, 2);
  const w = canvas.clientWidth, h = canvas.clientHeight;
  canvas.width = w*dpr; canvas.height = h*dpr; ctx.setTransform(dpr,0,0,dpr,0,0);
  ctx.clearRect(0,0,w,h);

  const D = 1.0, P = (previewR > params.peakCutoff) ? D*params.peakHeightFrac : 0.0;
  const E_probe = D*0.5; // ejecta height is normally solved for mass balance; a fixed probe value is enough to shape-preview
  const xMax = params.rMaxMult;
  const N = 240;
  const pts = [];
  let mn = 1e9, mx = -1e9;
  for (let i=0;i<=N;i++){
    const x = xMax*i/N;
    const dh = profile(params, x, D, P, E_probe);
    pts.push([x, dh]); mn = Math.min(mn, dh); mx = Math.max(mx, dh);
  }
  const pad = 18;
  const sx = (x) => pad + (x/xMax)*(w-2*pad);
  const sy = (v) => { const range = Math.max(mx-mn, 1e-6); return h-pad - ((v-mn)/range)*(h-2*pad); };

  ctx.strokeStyle = 'rgba(230,236,242,0.12)'; ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(pad, sy(0)); ctx.lineTo(w-pad, sy(0)); ctx.stroke();
  ctx.beginPath(); ctx.moveTo(sx(1), pad); ctx.lineTo(sx(1), h-pad); ctx.stroke();

  ctx.strokeStyle = '#8fc8e8'; ctx.lineWidth = 1.6; ctx.beginPath();
  pts.forEach(([x,v],i) => { const px=sx(x), py=sy(v); if (i===0) ctx.moveTo(px,py); else ctx.lineTo(px,py); });
  ctx.stroke();

  ctx.fillStyle = 'rgba(122,135,148,0.8)'; ctx.font = '9px monospace';
  ctx.fillText('R', sx(1)-3, h-4);
  ctx.fillText((previewR > params.peakCutoff) ? 'complex (has peak)' : 'simple (no peak)', pad, 12);
}

let modalEl = null;
window.openImpactDesigner = function(opts){
  opts = opts || {};
  const onApply = opts.onApply || function(){};
  const onStrike = opts.onStrike || null;
  const onSaved = opts.onSaved || function(){};
  const store = opts.store || localFallbackStore();
  const params = Object.assign({}, DEFAULTS, opts.current || {});
  let previewR = opts.previewR || 0.06;
  let currentPresetId = null;

  ensureStyles();
  if (modalEl) modalEl.remove();

  const backdrop = document.createElement('div'); backdrop.id = 'impBackdrop';
  const modal = document.createElement('div'); modal.id = 'impModal';
  backdrop.appendChild(modal);
  backdrop.addEventListener('click', (e) => { if (e.target === backdrop) close(); });

  const h2 = document.createElement('h2'); h2.textContent = 'Impact designer';
  const sub = document.createElement('div'); sub.className = 'sub'; sub.textContent = 'crater cross-section · bowl · peak · rim · ejecta';
  modal.appendChild(h2); modal.appendChild(sub);

  const previewWrap = document.createElement('div'); previewWrap.id = 'impPreviewWrap';
  const canvas = document.createElement('canvas'); canvas.id = 'impCanvas';
  previewWrap.appendChild(canvas);
  const rRow = document.createElement('div'); rRow.className = 'row';
  const rLab = document.createElement('label'); const rB = document.createElement('b'); rB.textContent = previewR.toFixed(3);
  rLab.appendChild(document.createTextNode('Preview radius R ')); rLab.appendChild(rB); rRow.appendChild(rLab);
  const rInp = document.createElement('input'); rInp.type = 'range'; rInp.min = 0.01; rInp.max = 0.12; rInp.step = 0.001; rInp.value = previewR;
  rInp.oninput = () => { previewR = parseFloat(rInp.value); rB.textContent = previewR.toFixed(3); redraw(); };
  rRow.appendChild(rInp);
  previewWrap.appendChild(rRow);
  modal.appendChild(previewWrap);

  const grid = document.createElement('div'); grid.id = 'impGrid';
  SCHEMA.forEach(s => buildRow(grid, s.key, params, redraw));
  modal.appendChild(grid);

  function redraw(){ drawPreview(canvas, params, previewR); }

  const presetGrp = document.createElement('div'); presetGrp.className = 'grp';
  presetGrp.style.cssText = 'font-size:8.5px;letter-spacing:.3em;text-transform:uppercase;color:var(--dim,#7a8794);border-top:1px solid var(--line,rgba(230,236,242,0.10));padding-top:11px;margin:16px 0 4px;opacity:.75';
  presetGrp.textContent = 'Presets'; modal.appendChild(presetGrp);

  const presetRow = document.createElement('div'); presetRow.id = 'impPresetRow';
  const nameInp = document.createElement('input'); nameInp.type = 'text'; nameInp.placeholder = 'preset name';
  const saveBtn = document.createElement('button'); saveBtn.textContent = 'Save as new';
  presetRow.appendChild(nameInp); presetRow.appendChild(saveBtn);
  modal.appendChild(presetRow);
  const presetList = document.createElement('div'); presetList.id = 'impPresetList';
  modal.appendChild(presetList);

  async function refreshPresets(){
    const list = await store.list();
    presetList.innerHTML = '';
    list.forEach(p => {
      const b = document.createElement('button'); b.textContent = p.name; b.title = 'Load (shift-click to delete)';
      b.onclick = async (e) => {
        if (e.shiftKey){ await store.remove(p.id); refreshPresets(); onSaved(); return; }
        Object.assign(params, DEFAULTS, p.params);
        currentPresetId = p.id; nameInp.value = p.name;
        grid.innerHTML = ''; SCHEMA.forEach(s => buildRow(grid, s.key, params, redraw));
        redraw();
      };
      presetList.appendChild(b);
    });
    return list;
  }
  saveBtn.onclick = async () => {
    const list = await store.list();
    const name = nameInp.value.trim() || ('crater ' + (list.length + 1));
    // reuse the id when the name matches the loaded preset, so Save updates it
    const id = (currentPresetId && list.some(x => x.id === currentPresetId && x.name === name))
      ? currentPresetId : undefined;
    const saved = await store.save({ id, name, params: Object.assign({}, params) });
    currentPresetId = saved.id;
    refreshPresets();
    onSaved();
  };

  const actions = document.createElement('div'); actions.id = 'impActions';
  if (onStrike){
    const strikeBtn = document.createElement('button'); strikeBtn.textContent = 'Strike test crater';
    strikeBtn.onclick = () => onStrike(Object.assign({}, params));
    actions.appendChild(strikeBtn);
  }
  const applyBtn = document.createElement('button'); applyBtn.textContent = 'Apply';
  applyBtn.onclick = () => { onApply(Object.assign({}, params)); close(); };
  const closeBtn = document.createElement('button'); closeBtn.textContent = 'Close';
  closeBtn.onclick = close;
  actions.appendChild(applyBtn); actions.appendChild(closeBtn);
  modal.appendChild(actions);

  function close(){ backdrop.remove(); if (modalEl === backdrop) modalEl = null; }

  document.body.appendChild(backdrop);
  modalEl = backdrop;
  refreshPresets();
  redraw();
  window.addEventListener('resize', redraw, { once:false });
};
})();
