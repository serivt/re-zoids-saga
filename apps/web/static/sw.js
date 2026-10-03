// Keeps the page, its script and the game's WebAssembly in the browser so
// the game plays offline once loaded. The ROM and the saves are not here:
// the page keeps them in the browser's own storage. Each build caches
// under its version, and a new one clears the caches of the builds before.
const CACHE = 're-zoids-saga-__VERSION__';
const FILES = [
  './',
  'index.html',
  'main.js',
  'store.js',
  'saves.js',
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

// The page's own files from the cache first; anything else (the
// translations' repository) from the network.
self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);
  if (event.request.method !== 'GET' || url.origin !== self.location.origin) return;
  event.respondWith(
    caches.match(event.request).then((cached) => cached ?? fetch(event.request)),
  );
});
