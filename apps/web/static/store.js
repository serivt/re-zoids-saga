// The browser's own database (IndexedDB) for what is too large for the
// page's storage: the ROM the player chose, and the translation's text.
// Nothing here leaves the browser.

const NAME = 're-zoids-saga';
const STORE = 'files';

function open() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function run(mode, action) {
  const db = await open();
  return new Promise((resolve, reject) => {
    const request = action(db.transaction(STORE, mode).objectStore(STORE));
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

// The value kept under `key`, or undefined.
export function get(key) {
  return run('readonly', (store) => store.get(key)).catch(() => undefined);
}

// Keeps `value` under `key`.
export function put(key, value) {
  return run('readwrite', (store) => store.put(value, key));
}

// Forgets what `key` holds.
export function remove(key) {
  return run('readwrite', (store) => store.delete(key));
}

// Asks the browser not to clear the page's storage when space runs low.
export async function keep() {
  try {
    return await navigator.storage?.persist?.();
  } catch {
    return false;
  }
}
