// The saves in the cloud, with an account: the player signs in with a link
// sent to their email, and the page keeps each save and the achievements by
// the account, the ROM's SHA-1 (never its bytes) and the slot, through the
// project's Supabase (its Auth and REST API, called directly). Nothing here
// runs unless the build names a Supabase project (config.js).
import { CLOUD } from './config.js';

const SESSION_KEY = 're-zoids-saga/cloud-session';
const REFRESH_MARGIN_MS = 60_000;

export const enabled = Boolean(CLOUD.url && CLOUD.anonKey);

function headers(token) {
  const result = { apikey: CLOUD.anonKey, 'Content-Type': 'application/json' };
  if (token) result.Authorization = `Bearer ${token}`;
  return result;
}

function readSession() {
  try {
    return JSON.parse(localStorage.getItem(SESSION_KEY) ?? 'null');
  } catch {
    return null;
  }
}

function writeSession(session) {
  if (session) localStorage.setItem(SESSION_KEY, JSON.stringify(session));
  else localStorage.removeItem(SESSION_KEY);
}

// The session a sign-in link brought back in the page's address, kept and
// taken out of the address; an error the link brought, if any.
export function takeSignIn() {
  const hash = new URLSearchParams(window.location.hash.slice(1));
  const error = hash.get('error_description');
  const access = hash.get('access_token');
  if (access || error) {
    history.replaceState(null, '', window.location.pathname + window.location.search);
  }
  if (access) {
    const expires = Number(hash.get('expires_in') ?? 3600);
    writeSession({
      access_token: access,
      refresh_token: hash.get('refresh_token'),
      expires_at: Date.now() + expires * 1000,
    });
  }
  return error;
}

// Sends the sign-in link to `email`; it brings the player back to this page.
export async function sendLink(email) {
  const back = encodeURIComponent(window.location.origin + window.location.pathname);
  const response = await fetch(`${CLOUD.url}/auth/v1/otp?redirect_to=${back}`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ email, create_user: true }),
  });
  if (!response.ok) throw new Error((await response.json()).msg ?? response.statusText);
}

// A token for the account, refreshed when it is about to run out; null
// when nobody is signed in.
async function token() {
  const session = readSession();
  if (!session) return null;
  if (session.expires_at - Date.now() > REFRESH_MARGIN_MS) return session.access_token;
  const response = await fetch(`${CLOUD.url}/auth/v1/token?grant_type=refresh_token`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ refresh_token: session.refresh_token }),
  });
  if (!response.ok) {
    writeSession(null);
    return null;
  }
  const fresh = await response.json();
  writeSession({
    ...session,
    access_token: fresh.access_token,
    refresh_token: fresh.refresh_token,
    expires_at: Date.now() + fresh.expires_in * 1000,
  });
  return fresh.access_token;
}

// The signed-in account's email, or null.
export async function account() {
  const bearer = await token();
  if (!bearer) return null;
  const response = await fetch(`${CLOUD.url}/auth/v1/user`, { headers: headers(bearer) });
  if (!response.ok) return null;
  return (await response.json()).email ?? null;
}

export async function signOut() {
  const bearer = await token();
  if (bearer) {
    await fetch(`${CLOUD.url}/auth/v1/logout`, { method: 'POST', headers: headers(bearer) }).catch(() => {});
  }
  writeSession(null);
}

async function rest(path, options = {}) {
  const bearer = await token();
  if (!bearer) throw new Error('not signed in');
  const response = await fetch(`${CLOUD.url}/rest/v1/${path}`, {
    ...options,
    headers: { ...headers(bearer), ...(options.headers ?? {}) },
  });
  if (!response.ok) throw new Error(await response.text());
  const text = await response.text();
  return text ? JSON.parse(text) : null;
}

// The cloud's saves of the ROM whose SHA-1 is `rom`, by slot.
export async function list(rom) {
  const rows = await rest(`saves?rom=eq.${rom}&select=slot,data,summary,updated_at`);
  return new Map(rows.map((row) => [row.slot, row]));
}

// Stores `data` as the cloud's save `slot` of the ROM `rom`; its row back,
// with the server's time.
export async function upload(rom, slot, data, summary) {
  const rows = await rest('saves?on_conflict=user_id,rom,slot', {
    method: 'POST',
    headers: { Prefer: 'resolution=merge-duplicates,return=representation' },
    body: JSON.stringify([{ rom, slot, data, summary }]),
  });
  return rows[0];
}

// Deletes the account, and with it every save in the cloud.
export async function deleteAccount() {
  await rest('rpc/delete_my_account', { method: 'POST', body: '{}' });
  writeSession(null);
}

// The ROM's SHA-1, as the cloud names its saves.
export async function romId(rom) {
  const digest = await crypto.subtle.digest('SHA-1', rom);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}
