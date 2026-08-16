/* ══════════════════════════════════════════════════════════════════════════
   preset-store.js — preset persistence for № 14 Orogeny.

   One store, two backends:

     FOLDER  — a real directory you pick, one .json file per preset, via the
               File System Access API. Presets become files you can version,
               diff, copy between machines and hand-edit.
     LOCAL   — localStorage, the original behaviour. Used when no folder has
               been chosen, or when the API is unavailable.

   Requirements for the folder backend, stated plainly because they are easy to
   trip over:
     · SECURE CONTEXT. showDirectoryPicker does not exist on file:// — the page
       must be served over http://localhost or https. There is a static server
       config in .claude/launch.json on port 8731 for exactly this.
     · Chrome / Edge / Opera. Firefox and Safari have no implementation, so
       supported === false and the store stays on localStorage silently.
     · A USER GESTURE to pick the folder, and usually one click per session to
       re-grant permission to a remembered folder. The handle itself survives in
       IndexedDB; the permission does not.

   ── Cross-tab notification ────────────────────────────────────────────────
   The designer is a separate page from the main tool, so a save in one has to
   reach the other. The filesystem gives us no change events, so writes also
   bump a counter in localStorage. That fires a `storage` event in every OTHER
   tab on the origin, which is precisely the case we need, and those tabs then
   re-read the folder. Files stay the source of truth; localStorage is only the
   doorbell. Focus is a second trigger, for when a tab was discarded.
   ════════════════════════════════════════════════════════════════════════ */
(function(){
'use strict';

const GEN_KEY = 'orogeny.presetGeneration';
const DB_NAME = 'orogeny-presets', DB_STORE = 'handles';
const supported = typeof window.showDirectoryPicker === 'function' && window.isSecureContext;

/* ─── a minimal promise wrapper over IndexedDB, which is the only place a
       directory handle can be persisted (it is not JSON-serializable) ─── */
function openDb(){
  return new Promise((res, rej) => {
    const r = indexedDB.open(DB_NAME, 1);
    r.onupgradeneeded = () => r.result.createObjectStore(DB_STORE);
    r.onsuccess = () => res(r.result);
    r.onerror   = () => rej(r.error);
  });
}
async function idbGet(key){
  const db = await openDb();
  return new Promise((res, rej) => {
    const q = db.transaction(DB_STORE, 'readonly').objectStore(DB_STORE).get(key);
    q.onsuccess = () => res(q.result);
    q.onerror   = () => rej(q.error);
  });
}
async function idbPut(key, val){
  const db = await openDb();
  return new Promise((res, rej) => {
    const tx = db.transaction(DB_STORE, 'readwrite');
    tx.objectStore(DB_STORE).put(val, key);
    tx.oncomplete = () => res();
    tx.onerror    = () => rej(tx.error);
  });
}
async function idbDel(key){
  const db = await openDb();
  return new Promise((res, rej) => {
    const tx = db.transaction(DB_STORE, 'readwrite');
    tx.objectStore(DB_STORE).delete(key);
    tx.oncomplete = () => res();
    tx.onerror    = () => rej(tx.error);
  });
}

const slug = s => (String(s || 'preset').toLowerCase()
  .replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '') || 'preset').slice(0, 60);
const newId = () => 'p' + Date.now().toString(36) + Math.random().toString(36).slice(2, 7);

/* ══════════════════════════════════════════════════════════════════════════
   create({ kind, lsKey, onChange })
     kind     stable key for the remembered folder (one folder per kind)
     lsKey    localStorage key — the fallback backend AND the legacy data to
              migrate from
     onChange called when another tab writes
   ════════════════════════════════════════════════════════════════════════ */
function create(opts){
  const kind = opts.kind, lsKey = opts.lsKey;
  const onChange = opts.onChange || function(){};
  let dir = null;                 // FileSystemDirectoryHandle when in folder mode
  let fileById = new Map();       // preset id -> filename, refreshed by list()

  const state = {
    supported,
    mode: 'local',                // 'local' | 'folder'
    folderName: null,
    needsGrant: false,            // a folder is remembered but not yet permitted
    lastError: null,
  };

  function readLocal(){
    try { const v = JSON.parse(localStorage.getItem(lsKey) || '[]'); return Array.isArray(v) ? v : []; }
    catch(e){ return []; }
  }
  function writeLocal(list){ localStorage.setItem(lsKey, JSON.stringify(list)); }
  function ping(){
    // doorbell for other tabs; the value only has to change
    try { localStorage.setItem(GEN_KEY, kind + ':' + Date.now()); } catch(e){}
  }

  async function perm(handle, request){
    const o = { mode: 'readwrite' };
    if ((await handle.queryPermission(o)) === 'granted') return true;
    if (!request) return false;
    return (await handle.requestPermission(o)) === 'granted';
  }

  /* Restore a remembered folder. Does NOT prompt: permission requests need a
     user gesture, so if the grant has lapsed we only flag needsGrant and let
     the page offer a button. */
  async function ready(){
    if (!supported) return state;
    try {
      const h = await idbGet(kind);
      if (!h) return state;
      state.folderName = h.name;
      if (await perm(h, false)){ dir = h; state.mode = 'folder'; state.needsGrant = false; }
      else { state.needsGrant = true; }
    } catch(e){ state.lastError = String(e); }
    return state;
  }

  /* Call from a click. Re-grants permission on the remembered folder. */
  async function reconnect(){
    if (!supported) return state;
    const h = await idbGet(kind);
    if (!h){ state.needsGrant = false; return state; }
    if (await perm(h, true)){ dir = h; state.mode = 'folder'; state.needsGrant = false; }
    return state;
  }

  /* Call from a click. Picks a folder and remembers it.
     If the folder is empty and localStorage already holds presets, they are
     copied in — so switching backends does not appear to lose your work. */
  async function chooseFolder(){
    if (!supported) throw new Error(
      window.isSecureContext
        ? 'This browser has no File System Access API — Chrome, Edge or Opera required.'
        : 'The File System Access API needs a secure context: serve the page over http://localhost or https, not file://.');
    const h = await window.showDirectoryPicker({ id: 'orogeny-' + kind, mode: 'readwrite' });
    if (!(await perm(h, true))) throw new Error('Permission to write the folder was declined.');
    dir = h; state.mode = 'folder'; state.folderName = h.name; state.needsGrant = false;
    await idbPut(kind, h);
    const existing = await list();
    if (!existing.length){
      const legacy = readLocal();
      for (const p of legacy) await save(p);
      state.migrated = legacy.length;
    } else {
      state.migrated = 0;
    }
    return state;
  }

  async function useLocal(){
    dir = null; state.mode = 'local'; state.needsGrant = false; state.folderName = null;
    await idbDel(kind).catch(()=>{});
    return state;
  }

  async function list(){
    if (state.mode !== 'folder' || !dir) return readLocal();
    const out = [];
    fileById = new Map();
    try {
      for await (const [name, handle] of dir.entries()){
        if (handle.kind !== 'file' || !name.toLowerCase().endsWith('.json')) continue;
        try {
          const txt = await (await handle.getFile()).text();
          const p = JSON.parse(txt);
          if (!p || typeof p !== 'object') continue;
          if (!p.id) p.id = name.replace(/\.json$/i, '');   // hand-made file, no id
          if (!p.name) p.name = name.replace(/\.json$/i, '');
          fileById.set(p.id, name);
          out.push(p);
        } catch(e){ /* a malformed or half-written file must not kill the list */ }
      }
    } catch(e){
      state.lastError = String(e);
      return readLocal();
    }
    out.sort((a,b) => String(a.name).localeCompare(String(b.name)));
    return out;
  }

  async function save(preset){
    const p = Object.assign({}, preset);
    if (!p.id) p.id = newId();
    p.savedAt = Date.now();

    if (state.mode !== 'folder' || !dir){
      const l = readLocal();
      const i = l.findIndex(x => x.id === p.id);
      if (i >= 0) l[i] = p; else l.push(p);
      writeLocal(l);
      ping();
      return p;
    }

    let fname = fileById.get(p.id);
    if (!fname){
      // fresh file; avoid clobbering an unrelated preset that shares a name
      const base = slug(p.name);
      const taken = new Set(fileById.values());
      fname = base + '.json';
      let n = 2;
      while (taken.has(fname)) fname = base + '-' + (n++) + '.json';
    }
    const fh = await dir.getFileHandle(fname, { create: true });
    const w = await fh.createWritable();
    await w.write(JSON.stringify(p, null, 2));
    await w.close();
    fileById.set(p.id, fname);
    ping();
    return p;
  }

  async function remove(id){
    if (state.mode !== 'folder' || !dir){
      writeLocal(readLocal().filter(x => x.id !== id));
      ping();
      return;
    }
    let fname = fileById.get(id);
    if (!fname){ await list(); fname = fileById.get(id); }
    if (fname) await dir.removeEntry(fname).catch(e => { state.lastError = String(e); });
    fileById.delete(id);
    ping();
  }

  /* Another tab wrote. lsKey covers the localStorage backend, GEN_KEY the
     folder one. Note `storage` fires only in OTHER tabs — which is what we
     want; the writing tab already knows. */
  window.addEventListener('storage', e => {
    if (e.key === lsKey || e.key === GEN_KEY) onChange();
  });
  window.addEventListener('focus', () => onChange());

  return { state, ready, reconnect, chooseFolder, useLocal, list, save, remove,
           get mode(){ return state.mode; },
           get folderName(){ return state.folderName; } };
}

window.OrogenyPresets = { supported, create };
})();
