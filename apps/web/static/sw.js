// Keeps the page, its script and the game's WebAssembly in the browser so
// the game plays offline once loaded. The ROM and the saves are not here:
// the page keeps them in the browser's own storage. Each build caches
// under its version, and a new one clears the caches of the builds before.
// The page asks it which version it keeps, to tell whether it is the
// latest the site serves (version.json, never kept).
const VERSION = '__VERSION__';
const CACHE = `re-zoids-saga-${VERSION}`;
const LATEST = 'version.json';
const FILES = [
  './',
  'index.html',
  'main.js',
  'store.js',
  'saves.js',
  'cloud.js',
  'sync.js',
  'config.js',
  'privacy.html',
  'style.css',
  'manifest.webmanifest',
  'icon.png',
  'pkg/re_zoids_saga_web.js',
  'pkg/re_zoids_saga_web_bg.wasm',
];

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(FILES)));
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))))
      .then(() => self.clients.claim()),
  );
});

// The page's own files from the cache first, and kept there when they had
// to come from the network (as after the page cleared the cache to force a
// fresh load); the latest version's number and anything else (the
// translations' repository, the cloud) from the network alone.
self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);
  if (event.request.method !== 'GET' || url.origin !== self.location.origin) return;
  if (url.pathname.endsWith(`/${LATEST}`)) return;
  event.respondWith(
    caches.match(event.request).then((cached) => cached ?? fetch(event.request).then((response) => {
      if (response.ok) {
        const copy = response.clone();
        caches.open(CACHE).then((cache) => cache.put(event.request, copy));
      }
      return response;
    })),
  );
});

// The page asks which version this worker keeps.
self.addEventListener('message', (event) => {
  if (event.data === 'version') event.ports[0]?.postMessage({ version: VERSION });
});
