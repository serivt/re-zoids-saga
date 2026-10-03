// The saves kept in the browser's page storage, as the game keeps them
// (Base64 under re-zoids-saga/slot-1 to slot-4 and autosave, with the time
// each was stored), exported and imported as files.
import { cartridge_save, save_summary } from './pkg/re_zoids_saga_web.js';

const PREFIX = 're-zoids-saga/';
const STORED = '#stored';
export const SAVES = [
  { key: 'slot-1', label: 'Slot 1', imports: true },
  { key: 'slot-2', label: 'Slot 2', imports: true },
  { key: 'slot-3', label: 'Slot 3', imports: true },
  { key: 'slot-4', label: 'Slot 4', imports: true },
  { key: 'autosave', label: 'Autosave', imports: false },
];

// The bytes of the save under `key`, or null.
export function read(key) {
  const text = localStorage.getItem(PREFIX + key);
  if (text === null) return null;
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let at = 0; at < binary.length; at++) bytes[at] = binary.charCodeAt(at);
  return bytes;
}

// Keeps `bytes` as the save under `key`, kept first as `key.bak`.
export function write(key, bytes) {
  const before = localStorage.getItem(PREFIX + key);
  if (before !== null) localStorage.setItem(`${PREFIX}${key}.bak`, before);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  localStorage.setItem(PREFIX + key, btoa(binary));
  localStorage.setItem(PREFIX + key + STORED, String(Date.now()));
}

// What the save under `key` holds, for the list.
export function summary(rom, key) {
  const bytes = read(key);
  return bytes ? (save_summary(rom, bytes) ?? 'not a save of the game') : 'empty';
}

// Offers `bytes` to the player as the file `name`.
export function download(name, bytes) {
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }));
  const link = Object.assign(document.createElement('a'), { href: url, download: name });
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// The bytes an export of the save under `key` writes, as `kind` asks:
// `sav`, `srm` or `cartridge`.
export function exported(rom, key, kind) {
  const bytes = read(key);
  if (!bytes) return null;
  return kind === 'cartridge' ? cartridge_save(rom, bytes) : bytes;
}
