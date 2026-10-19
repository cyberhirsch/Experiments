/* CamFluid — cross-origin isolation service worker.
   The HEIC encoder (libheif + x265 in WebAssembly) runs threads on shared memory, which browsers allow
   only on a cross-origin-isolated page. Static hosts (GitHub Pages, a plain localhost server) do not send
   the headers, so this worker adds them to every response: COOP same-origin, COEP credentialless (the
   fonts and the CDN still load without CORP headers). Same idea as coi-serviceworker. Not possible on
   file:// (no service workers there). */
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));
self.addEventListener('fetch', (e) => {
  let r = e.request;
  if (r.cache === 'only-if-cached' && r.mode !== 'same-origin') return;
  if (r.mode === 'no-cors') r = new Request(r, { credentials: 'omit' });
  e.respondWith(fetch(r).then((res) => {
    if (res.status === 0) return res;
    const h = new Headers(res.headers);
    h.set('Cross-Origin-Embedder-Policy', 'credentialless');
    h.set('Cross-Origin-Opener-Policy', 'same-origin');
    return new Response(res.body, { status: res.status, statusText: res.statusText, headers: h });
  }).catch((err) => { console.error(err); return Response.error(); }));
});
