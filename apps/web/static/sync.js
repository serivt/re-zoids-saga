// Keeps the browser's saves and the cloud's alike, slot by slot. A save
// changed in one place only since the last sync goes to the other; one
// changed in both (or found in both before any sync) is a conflict the
// player settles, seeing what each holds; the achievements never conflict:
// the two lists are joined. Each slot remembers its last sync: the cloud's
// time and the browser's.
import * as cloud from './cloud.js';
import * as saves from './saves.js';

const PREFIX = 're-zoids-saga/';
const SYNCED = '#synced';
const STORED = '#stored';
const ACHIEVEMENTS = 'achievements';

function local(key) {
  return {
    data: localStorage.getItem(PREFIX + key),
    stored: localStorage.getItem(PREFIX + key + STORED),
  };
}

function lastSync(key) {
  try {
    return JSON.parse(localStorage.getItem(PREFIX + key + SYNCED) ?? 'null');
  } catch {
    return null;
  }
}

function remember(key, cloudTime) {
  const { stored } = local(key);
  localStorage.setItem(PREFIX + key + SYNCED, JSON.stringify({ cloud: cloudTime, local: stored }));
}

function takeCloud(key, row) {
  saves.writeText(key, row.data);
  remember(key, row.updated_at);
}

// The keys of the achievements' text `data` (Base64), one a line.
function achievementKeys(data) {
  if (!data) return [];
  return new TextDecoder().decode(saves.fromBase64(data)).split('\n').filter((line) => line && !line.startsWith('#'));
}

// Syncs every slot of the ROM `rom` (its bytes) and the achievements;
// returns the conflicts left to settle, by slot: what each side holds.
export async function syncAll(rom, { uploadsOnly = false } = {}) {
  const id = await cloud.romId(rom);
  const rows = await cloud.list(id);
  const conflicts = new Map();
  for (const { key } of saves.SAVES) {
    const conflict = await syncSlot(rom, id, key, rows.get(key), uploadsOnly);
    if (conflict) conflicts.set(key, conflict);
  }
  await syncAchievements(id, rows.get(ACHIEVEMENTS), uploadsOnly);
  return conflicts;
}

async function syncSlot(rom, id, key, row, uploadsOnly) {
  const mine = local(key);
  const synced = lastSync(key);
  const upload = async () => {
    const summary = saves.summary(rom, key);
    const stored = await cloud.upload(id, key, mine.data, summary);
    remember(key, stored.updated_at);
  };
  if (!mine.data && !row) return null;
  if (mine.data && !row) return upload();
  if (!mine.data) return uploadsOnly ? null : takeCloud(key, row);
  if (mine.data === row.data) return remember(key, row.updated_at);
  const localChanged = !synced || synced.local !== mine.stored;
  const cloudChanged = !synced || synced.cloud !== row.updated_at;
  if (localChanged && !cloudChanged) return upload();
  if (!localChanged && cloudChanged) return uploadsOnly ? null : takeCloud(key, row);
  return { browser: saves.summary(rom, key), cloud: row.summary ?? 'a save', row };
}

async function syncAchievements(id, row, uploadsOnly) {
  const mine = local(ACHIEVEMENTS).data;
  const joined = [...new Set([...achievementKeys(mine), ...achievementKeys(row?.data)])];
  if (joined.length === 0) return;
  const data = saves.toBase64(new TextEncoder().encode(`# re-zoids-saga achievements\n${joined.join('\n')}\n`));
  const unchanged = (text) => text && achievementKeys(text).length === joined.length;
  if (!uploadsOnly && !unchanged(mine)) saves.writeText(ACHIEVEMENTS, data);
  if (!unchanged(row?.data)) {
    const stored = await cloud.upload(id, ACHIEVEMENTS, data, `${joined.length} unlocked`);
    remember(ACHIEVEMENTS, stored.updated_at);
  }
}

// Settles slot `key`'s conflict: keeps the browser's save (sent to the
// cloud) or takes the cloud's (the browser's kept as a backup).
export async function settle(rom, key, conflict, keepBrowser) {
  if (keepBrowser) {
    const id = await cloud.romId(rom);
    const stored = await cloud.upload(id, key, local(key).data, saves.summary(rom, key));
    remember(key, stored.updated_at);
  } else {
    takeCloud(key, conflict.row);
  }
}
