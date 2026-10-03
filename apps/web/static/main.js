// The page around the game: the ROM (read here, kept in this browser and
// never sent), the translation (downloaded from the translations'
// repository or chosen as a file, and kept), the options, the saves, the
// cloud, then the game, which places its screen and its on-screen pad in
// the window itself, with a pause menu over it.
import init, { check_rom, save_details, start } from './pkg/re_zoids_saga_web.js';
import * as store from './store.js';
import * as saves from './saves.js';
import * as cloud from './cloud.js';
import * as sync from './sync.js';

const OPTIONS_KEY = 're-zoids-saga/page-options';
const TAB_KEY = 're-zoids-saga/page-tab';
const PICTURE_OPEN_KEY = 're-zoids-saga/page-picture-open';
const TRANSLATIONS = 'https://raw.githubusercontent.com/serivt/re-zoids-saga-translations/main/';
const INDEX = 'po/languages.json';
const SYNC_EVERY_MS = 60_000;
// The display presets, as the launcher's: a screen the game was played on
// (its grid, colors and trail) or a way of showing it today.
const PRESETS = {
  gba: { filter: 'lcd', scaling: 'sharp', upscale: 'none', color: 'gba', trail: 'fade' },
  'gba-sp-frontlit': { filter: 'lcd-soft', scaling: 'sharp', upscale: 'none', color: 'gba-sp-frontlit', trail: '1' },
  'gba-sp': { filter: 'lcd-soft', scaling: 'sharp', upscale: 'none', color: 'gba-sp', trail: '1' },
  micro: { filter: 'lcd-fine', scaling: 'sharp', upscale: 'none', color: 'micro', trail: '0' },
  ds: { filter: 'off', scaling: 'fill', upscale: 'none', color: 'ds', trail: '0' },
  player: { filter: 'scanlines', scaling: 'sharp', upscale: 'none', color: 'original', trail: '0' },
  modern: { filter: 'off', scaling: 'fill', upscale: 'none', color: 'original', trail: '0' },
  'smooth-pixel-art': { filter: 'off', scaling: 'fill', upscale: 'scale3x', color: 'original', trail: '0' },
};
// The enhanced mode's settings, by the launcher's names.
const ENHANCEMENTS = [
  'battle-animations', 'damage-numbers', 'auto-text', 'autosave', 'weapon-reach', 'fast-forward',
];
// The options the page keeps, by the form fields' names; `muted` is the
// Mute button's.
const OPTIONS = [
  'language', 'mode', 'scaling', 'color', 'trail', 'upscale', 'filter', 'volume',
  'touch', 'touch-size', 'touch-opacity', ...ENHANCEMENTS,
];

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

// An element: `h('p', { class: 'x', onclick }, 'text', child)`.
function h(tag, attributes = {}, ...children) {
  const element = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) {
    if (value == null || value === false) continue;
    if (name.startsWith('on')) element.addEventListener(name.slice(2), value);
    else element.setAttribute(name, value === true ? '' : value);
  }
  for (const child of children.flat()) {
    if (child != null) element.append(child.nodeType ? child : String(child));
  }
  return element;
}

// One of the page's icons.
function icon(id, size = 16) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('width', size);
  svg.setAttribute('height', size);
  svg.setAttribute('aria-hidden', 'true');
  svg.classList.add('icon');
  const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
  use.setAttribute('href', `#${id}`);
  svg.append(use);
  return svg;
}

// Says `text` on `element`, as `kind` (`ok`, `error`, `info` or none).
function say(element, text, kind = '') {
  element.textContent = text;
  element.dataset.kind = kind;
}

function readJson(key, fallback) {
  try {
    return JSON.parse(localStorage.getItem(key) ?? 'null') ?? fallback;
  } catch {
    return fallback;
  }
}

function writeJson(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // A browser that keeps nothing for the page plays with the defaults.
  }
}

const launcher = $('#launcher');
const form = $('#options');
const play = $('#play');

let rom = null;
let romName = 'Zoids Saga';
let translation = null;
let conflicts = new Map();
let signedIn = false;
let session = null;

await init();

// Tabs -------------------------------------------------------------------

const visibleTabs = () => $$('[role="tab"]').filter((tab) => !tab.hidden);

function selectTab(tab, focus = false) {
  for (const each of $$('[role="tab"]')) {
    const on = each === tab;
    each.setAttribute('aria-selected', on);
    each.tabIndex = on ? 0 : -1;
    $(`#${each.getAttribute('aria-controls')}`).hidden = !on;
  }
  if (focus) tab.focus();
  writeJson(TAB_KEY, tab.id);
}

$('[role="tablist"]').addEventListener('click', (event) => {
  const tab = event.target.closest('[role="tab"]');
  if (tab) selectTab(tab);
});
$('[role="tablist"]').addEventListener('keydown', (event) => {
  const tabs = visibleTabs();
  const at = tabs.indexOf(document.activeElement);
  if (at < 0) return;
  const next = { ArrowRight: at + 1, ArrowLeft: at - 1, Home: 0, End: tabs.length - 1 }[event.key];
  if (next === undefined) return;
  event.preventDefault();
  selectTab(tabs[(next + tabs.length) % tabs.length], true);
});
document.addEventListener('click', (event) => {
  const link = event.target.closest('[data-goto]');
  if (!link) return;
  event.preventDefault();
  const tab = $(`#${link.dataset.goto}`);
  selectTab(tab, true);
  tab.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
});

// The ROM ----------------------------------------------------------------

const drop = $('#rom-drop');
const romStatus = $('#rom-status');
const romDetail = $('#rom-detail');
const ROM_STATES = {
  empty: ['Drop your ROM here', 'A .gba file · Japan, Rev 1', 'Choose a file…'],
  drag: ['Release to check it', 'Zoids Saga, Japan, Rev 1', 'Choose a file…'],
  checking: ['Checking the ROM…', 'In this browser; nothing is sent.', 'Choose a file…'],
  ok: ['', '', 'Choose another…'],
  remembered: ['', 'Remembered from your last visit.', 'Choose another…'],
  forgotten: ['Drop your ROM here', 'The ROM is forgotten; the saves stay.', 'Choose a file…'],
  error: ['', 'You need Zoids Saga, Japan, Rev 1.', 'Try another file…'],
};
let romState = 'empty';
let beforeDrag = null;

// Shows the ROM's `state`, with `title` and `detail` in place of the
// state's own.
function setRom(state, title, detail) {
  romState = state;
  const [ownTitle, ownDetail, choose] = ROM_STATES[state];
  drop.dataset.state = state === 'forgotten' ? 'empty' : state;
  romStatus.textContent = title || ownTitle;
  romDetail.textContent = detail || ownDetail;
  $('#rom-choose').textContent = choose;
  const ready = Boolean(rom) && (state === 'ok' || state === 'remembered');
  $('#rom-forget').hidden = !ready;
  play.disabled = !ready;
  $('#play-hint').hidden = ready;
  for (const id of ['#start-intro', '#rom-faq', '#step2']) $(id).hidden = ready;
  document.body.dataset.romReady = ready;
  renderSaves();
}

async function takeRom(file) {
  if (!file) return;
  setRom('checking');
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (useRom(bytes, file.name, false)) {
      await store.put('rom', { name: file.name, bytes });
      await store.keep();
      if (signedIn) await runSync();
    }
  } catch {
    rom = null;
    setRom('error', 'The file could not be read.', 'Try choosing it again.');
  }
}

// Takes `bytes` as the ROM when the port plays it; says what it is.
function useRom(bytes, name, remembered) {
  const check = check_rom(bytes);
  rom = check.playable ? bytes : null;
  if (rom) {
    romName = name.replace(/\.[^.]*$/, '') || romName;
    setRom(remembered ? 'remembered' : 'ok', check.message, remembered ? `Remembered from your last visit: ${name}.` : name);
  } else {
    setRom('error', check.message);
  }
  return check.playable;
}

async function restoreRom() {
  const kept = await store.get('rom');
  if (kept?.bytes) useRom(new Uint8Array(kept.bytes), kept.name, true);
  else setRom('empty');
}

async function forgetRom() {
  await store.remove('rom');
  rom = null;
  setRom('forgotten');
  $('#rom-file').focus();
}

$('#rom-file').addEventListener('change', (event) => {
  takeRom(event.target.files[0]);
  event.target.value = '';
});
$('#rom-forget').addEventListener('click', forgetRom);

// A file dropped anywhere on the launcher; the zone shows it coming.
let dragDepth = 0;
const carriesFiles = (event) => [...(event.dataTransfer?.types ?? [])].includes('Files');
addEventListener('dragenter', (event) => {
  if (!carriesFiles(event) || launcher.hidden) return;
  if (dragDepth++ === 0) {
    beforeDrag = [romState, romStatus.textContent, romDetail.textContent];
    drop.dataset.state = 'drag';
    romStatus.textContent = ROM_STATES.drag[0];
    romDetail.textContent = ROM_STATES.drag[1];
  }
});
addEventListener('dragleave', () => {
  if (dragDepth && --dragDepth === 0 && beforeDrag) setRom(...beforeDrag);
});
addEventListener('dragover', (event) => {
  if (carriesFiles(event)) event.preventDefault();
});
addEventListener('drop', (event) => {
  if (!carriesFiles(event)) return;
  event.preventDefault();
  dragDepth = 0;
  if (!launcher.hidden) takeRom(event.dataTransfer.files[0]);
});

// The options --------------------------------------------------------------

const language = $('#language');
const translationStatus = $('#translation-status');
const volume = $('#volume');

function option(name) {
  const field = form.elements[name];
  if (field?.type === 'checkbox') return field.checked ? '1' : '0';
  return field?.value;
}

function setOption(name, value) {
  const field = form.elements[name];
  if (!field || value === undefined || value === null) return;
  if (field.type === 'checkbox') {
    field.checked = value === '1';
    return;
  }
  if (field.tagName === 'SELECT' && ![...field.options].some((each) => each.value === value)) return;
  if (field instanceof RadioNodeList && ![...field].some((each) => each.value === value)) return;
  field.value = value;
}

function muted() {
  return $('#mute').getAttribute('aria-pressed') === 'true';
}

function setMuted(on) {
  $('#mute').setAttribute('aria-pressed', on);
  $('#mute span').textContent = on ? 'Muted' : 'Mute';
}

// What the options choose, in a line, and kept.
function saveOptions() {
  const kept = Object.fromEntries(OPTIONS.map((name) => [name, option(name)]));
  kept.muted = muted();
  writeJson(OPTIONS_KEY, kept);
  const picked = (name) => $(`input[name="${name}"]:checked + span`, form)?.textContent;
  const lang = language.value === '' ? 'Japanese' : language.selectedOptions[0]?.textContent;
  const colors = $('#color').selectedOptions[0].textContent;
  const line = `${lang} · ${picked('mode')} · ${picked('scaling')} · ${colors === 'Original' ? 'Original colors' : `${colors} colors`}`;
  $('#setup-summary').textContent = `${line}.`;
  $('#playbar-summary').textContent = line;
  const trail = picked('trail');
  const upscaler = picked('upscale');
  const preset = currentPreset();
  for (const chip of $$('[data-preset]')) chip.setAttribute('aria-pressed', chip.dataset.preset === preset);
  const filter = $('#filter').selectedOptions[0].textContent;
  $('#picture-summary').textContent = preset
    ? `${$(`[data-preset="${preset}"]`).textContent} preset`
    : [
      picked('scaling'),
      colors === 'Original' ? 'Original colors' : `${colors} colors`,
      trail === 'Off' ? 'No trail' : `${trail} trail`,
      upscaler === 'Off' ? 'No upscaler' : upscaler,
      filter === 'Off' ? 'No filter' : filter,
    ].join(' · ');
}

// The preset the picture's options match, if any.
function currentPreset() {
  return Object.keys(PRESETS).find((name) => Object.entries(PRESETS[name]).every(([key, value]) => option(key) === value));
}

for (const chip of $$('[data-preset]')) {
  chip.addEventListener('click', () => {
    for (const [key, value] of Object.entries(PRESETS[chip.dataset.preset])) setOption(key, value);
    saveOptions();
  });
}

function restoreOptions() {
  const kept = readJson(OPTIONS_KEY, {});
  for (const name of OPTIONS) {
    if (name !== 'language') setOption(name, kept[name]);
  }
  setMuted(Boolean(kept.muted));
  $('#volume-out').value = `${volume.value}%`;
  showEnhanced();
}

// The picture's options fold away; the page remembers whether they were
// open.
const picture = $('#picture');
picture.open = readJson(PICTURE_OPEN_KEY, false);
picture.addEventListener('toggle', () => writeJson(PICTURE_OPEN_KEY, picture.open));

// A tip on what the enhanced mode adds: over the info button while the
// pointer or the focus is on it, and held open by a tap or a click.
const info = $('#enhanced-info');
const tip = $('#enhanced-tip');
let tipHeld = false;

function showTip(show) {
  tip.hidden = !show;
  info.setAttribute('aria-expanded', show);
}

info.addEventListener('pointerenter', (event) => {
  if (event.pointerType === 'mouse') showTip(true);
});
info.addEventListener('pointerleave', (event) => {
  if (event.pointerType === 'mouse' && !tipHeld) showTip(false);
});
info.addEventListener('focus', () => showTip(true));
info.addEventListener('blur', () => {
  tipHeld = false;
  showTip(false);
});
info.addEventListener('click', (event) => {
  event.preventDefault();
  tipHeld = !tipHeld;
  showTip(tipHeld);
});
info.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && !tip.hidden) {
    event.stopPropagation();
    tipHeld = false;
    showTip(false);
  }
});
document.addEventListener('pointerdown', (event) => {
  if (!tip.hidden && event.target !== info && !info.contains(event.target)) {
    tipHeld = false;
    showTip(false);
  }
});

// The enhanced mode's settings, shown only while it is chosen.
function showEnhanced() {
  $('#enhanced-group').hidden = option('mode') !== 'enhanced';
}

form.addEventListener('submit', (event) => event.preventDefault());
form.addEventListener('change', (event) => {
  if (event.target.name === 'mode') showEnhanced();
  if (event.target !== language) saveOptions();
});
volume.addEventListener('input', () => {
  $('#volume-out').value = `${volume.value}%`;
  session?.set_volume(Number(volume.value));
});
$('#mute').addEventListener('click', () => {
  setMuted(!muted());
  saveOptions();
});

// The translation ------------------------------------------------------------

// Lists the languages the translations' repository offers, when online.
async function offerLanguages() {
  try {
    const index = await (await fetch(TRANSLATIONS + INDEX)).json();
    for (const each of index.languages ?? []) {
      language.insertBefore(new Option(each.name, `repo:${each.file}`), language.lastElementChild);
    }
  } catch {
    // Offline: the translation kept, or a file, still works.
  }
}

let lastLanguage = '';

async function chooseLanguage() {
  const choice = language.value;
  if (choice === 'file') {
    language.value = lastLanguage;
    $('#po-file').click();
    return;
  }
  lastLanguage = choice;
  if (choice === '') {
    translation = null;
    await store.remove('translation');
    say(translationStatus, 'Downloaded once, then kept for offline use.');
  } else if (choice.startsWith('repo:')) {
    const name = language.selectedOptions[0].text;
    say(translationStatus, `Downloading ${name}…`);
    try {
      const response = await fetch(TRANSLATIONS + choice.slice(5));
      if (!response.ok) throw new Error(response.statusText);
      await keepTranslation(name, await response.text());
    } catch {
      say(translationStatus, `${name} could not be downloaded. Check the connection and choose it again.`, 'error');
    }
  }
  saveOptions();
}

async function takeTranslationFile(file) {
  if (!file) return;
  try {
    const text = await file.text();
    let entry = $('option[value="po-file"]', language);
    if (!entry) {
      entry = new Option('', 'po-file');
      language.insertBefore(entry, language.lastElementChild);
    }
    entry.textContent = file.name;
    language.value = lastLanguage = 'po-file';
    await keepTranslation(file.name, text);
    saveOptions();
  } catch {
    say(translationStatus, "That .po file couldn't be read.", 'error');
  }
}

async function keepTranslation(name, text) {
  translation = text;
  await store.put('translation', { name, text });
  say(translationStatus, `${name} is kept for offline use.`, 'ok');
}

// The translation kept, when the options chose one; offline, the
// language it came from is offered again from what is kept.
async function restoreTranslation() {
  const kept = await store.get('translation');
  const wanted = readJson(OPTIONS_KEY, {}).language ?? '';
  if (kept?.text && wanted !== '') {
    if (![...language.options].some((each) => each.value === wanted)) {
      language.insertBefore(new Option(kept.name, wanted), language.lastElementChild);
    }
    language.value = wanted;
    translation = kept.text;
    say(translationStatus, `${kept.name} is kept for offline use.`, 'ok');
  }
  lastLanguage = language.value;
}

language.addEventListener('change', chooseLanguage);
$('#po-file').addEventListener('change', (event) => {
  takeTranslationFile(event.target.files[0]);
  event.target.value = '';
});

// The saves ------------------------------------------------------------------

const savesList = $('#saves-list');
const savesStatus = $('#saves-status');
const money = (value) => `${Number(value).toLocaleString('en-US')} G`;

// What the save under `key` holds: null when empty, `{ broken: true }`
// when it is no save of the game.
function held(key) {
  const bytes = saves.read(key);
  if (!bytes) return null;
  const details = save_details(rom, bytes);
  if (!details) return { broken: true };
  const found = { level: details.level, area: details.area, money: details.money, played: details.played };
  details.free();
  return found;
}

function renderSaves() {
  const ready = Boolean(rom);
  $('#saves-norom').hidden = ready;
  savesList.hidden = !ready;
  const count = $('#saves-count');
  if (!ready) {
    count.hidden = true;
    return;
  }
  const items = saves.SAVES.map((save) => ({ ...save, holds: held(save.key) }));
  const used = items.filter((save) => save.imports && save.holds).length;
  count.hidden = false;
  count.textContent = `${used} of ${items.filter((save) => save.imports).length}`;
  savesList.replaceChildren(...items.map(slotCard));
}

function slotCard(save) {
  const id = `card-${save.key}`;
  const actions = h('div', { class: 'slot-a' });
  if (save.holds) {
    actions.append(h('button', {
      class: 'btn btn-sm', type: 'button', 'aria-haspopup': 'menu', 'aria-expanded': 'false', 'aria-describedby': id,
      onclick: (event) => openExport(event.currentTarget, save),
    }, icon('i-down'), 'Export', icon('i-chev', 14)));
  }
  if (save.imports) {
    actions.append(h('button', {
      class: 'btn btn-sm', type: 'button', 'aria-describedby': id, onclick: () => pickImport(save),
    }, icon('i-up'), 'Import…'));
  }
  let body;
  if (!save.holds) {
    body = [h('p', { class: 'muted grow' }, 'Empty.')];
  } else if (save.holds.broken) {
    body = [h('p', { class: 'muted grow' }, 'Not a save of the game.')];
  } else {
    body = [
      h('div', {}, h('p', { class: 'slot-lv' }, `Lv ${save.holds.level}`), h('p', { class: 'slot-area' }, `Area ${save.holds.area}`)),
      h('dl', { class: 'stats' },
        h('div', {}, h('dt', {}, 'Money'), h('dd', {}, money(save.holds.money))),
        h('div', {}, h('dt', {}, 'Time played'), h('dd', {}, save.holds.played ?? '—'))),
    ];
  }
  const conflict = conflicts.get(save.key);
  const settling = conflict
    ? h('div', { class: 'conflict' },
      h('p', {}, `Changed here and in the cloud. This browser: ${conflict.browser}. Cloud: ${conflict.cloud}.`),
      h('div', { class: 'slot-a' },
        h('button', { class: 'btn btn-sm', type: 'button', onclick: () => settle(save.key, conflict, true) }, "Keep this browser's"),
        h('button', { class: 'btn btn-sm', type: 'button', onclick: () => settle(save.key, conflict, false) }, "Take the cloud's")))
    : null;
  const empty = !save.holds || save.holds.broken;
  return h('li', { class: `slot${empty ? ' empty' : ''}${save.imports ? '' : ' auto'}` },
    h('h4', { class: 'slot-n', id }, save.label, save.imports ? null : h('span', { class: 'hint note-inline' }, 'export only')),
    ...body, settling, actions);
}

// The export menu, one for every card, under the button that opened it.
const exportMenu = h('div', { class: 'menu', role: 'menu', hidden: true, id: 'export-menu' },
  [['sav', '.sav', 'For most emulators and flash carts'], ['srm', '.srm', 'For RetroArch'],
    ['cartridge', 'Cartridge', "As the cartridge holds it, without the port's notes"]]
    .map(([kind, name, note]) => h('button', { class: 'menu-i', role: 'menuitem', type: 'button', tabindex: '-1', 'data-kind': kind },
      h('b', {}, name), h('span', {}, note))));
document.body.append(exportMenu);
let menuFor = null;
let menuButton = null;

function openExport(button, save) {
  if (menuButton === button) {
    closeExport(true);
    return;
  }
  closeExport();
  menuFor = save;
  menuButton = button;
  const bounds = button.getBoundingClientRect();
  exportMenu.style.top = `${bounds.bottom + scrollY + 6}px`;
  exportMenu.style.left = `${Math.min(bounds.left + scrollX, innerWidth - 306 + scrollX)}px`;
  exportMenu.setAttribute('aria-label', `Export ${save.label} as`);
  exportMenu.hidden = false;
  button.setAttribute('aria-expanded', 'true');
  exportMenu.firstChild.focus();
}

function closeExport(refocus) {
  if (exportMenu.hidden) return;
  exportMenu.hidden = true;
  menuButton?.setAttribute('aria-expanded', 'false');
  if (refocus) menuButton?.focus();
  menuButton = null;
  menuFor = null;
}

exportMenu.addEventListener('keydown', (event) => {
  const items = $$('.menu-i', exportMenu);
  const at = items.indexOf(document.activeElement);
  const next = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: items.length - 1 }[event.key];
  if (next !== undefined) {
    event.preventDefault();
    items[(next + items.length) % items.length].focus();
  } else if (event.key === 'Escape') {
    event.preventDefault();
    closeExport(true);
  } else if (event.key === 'Tab') {
    closeExport();
  }
});
exportMenu.addEventListener('click', (event) => {
  const item = event.target.closest('.menu-i');
  if (!item) return;
  const save = menuFor;
  closeExport(true);
  exportSave(save, item.dataset.kind);
});
document.addEventListener('pointerdown', (event) => {
  if (!exportMenu.hidden && !exportMenu.contains(event.target) && event.target.closest('button') !== menuButton) closeExport();
});

function exportSave(save, kind) {
  const bytes = saves.exported(rom, save.key, kind);
  if (!bytes) return;
  const name = `${romName}.${kind === 'srm' ? 'srm' : 'sav'}`;
  saves.download(name, bytes);
  say(savesStatus, `${save.label} exported as ${name}.`, 'ok');
}

// An import, after the player sees what it replaces.
let importing = null;
const importDialog = $('#import-dialog');

function pickImport(save) {
  importing = save;
  $('#save-file').click();
}

$('#save-file').addEventListener('change', async (event) => {
  const file = event.target.files[0];
  event.target.value = '';
  if (!file || !importing) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  const details = save_details(rom, bytes);
  if (!details) {
    say(savesStatus, "That file isn't a Zoids Saga save.", 'error');
    return;
  }
  const incoming = { level: details.level, area: details.area, money: details.money, played: details.played };
  details.free();
  const save = importing;
  const now = held(save.key);
  const empty = !now || now.broken;
  $('#import-eyebrow').textContent = `Import into ${save.label}`;
  $('#import-title').textContent = empty ? `Import into ${save.label}?` : `Replace the save in ${save.label}?`;
  $('#import-desc').textContent = empty
    ? `${save.label} is empty.`
    : `${save.label} already holds a save. Export it first if you want to keep a copy.`;
  $('#import-fname').textContent = file.name;
  $('#import-slot-h').textContent = `In ${save.label} now`;
  $('#import-format').textContent = /\.srm$/i.test(file.name)
    ? 'Read as a RetroArch save (.srm).'
    : 'Read as an emulator save (.sav).';
  const row = (label, theirs, ours) => h('tr', {}, h('th', { scope: 'row' }, label), h('td', { class: 'col-file' }, theirs), h('td', {}, ours));
  const ours = (pick) => (empty ? '—' : pick(now));
  $('#import-rows').replaceChildren(
    row('Level', incoming.level, ours((slot) => slot.level)),
    row('Area', incoming.area, ours((slot) => slot.area)),
    row('Money', money(incoming.money), ours((slot) => money(slot.money))),
    row('Time played', incoming.played ?? '—', ours((slot) => slot.played ?? '—')),
  );
  const confirm = $('#import-confirm');
  confirm.textContent = empty ? 'Import' : `Replace ${save.label}`;
  confirm.className = `btn ${empty ? 'btn-primary' : 'btn-danger-solid'}`;
  $('#import-export-first').hidden = empty;
  $('#import-export-first span').textContent = `Export ${save.label} first`;
  importDialog.returnValue = '';
  importDialog.onclose = async () => {
    if (importDialog.returnValue !== 'confirm') {
      say(savesStatus, 'Import cancelled. Nothing changed.', 'info');
      return;
    }
    saves.write(save.key, bytes);
    renderSaves();
    say(savesStatus, `${save.label} now holds the save from ${file.name}.`, 'ok');
    if (signedIn) await runSync();
  };
  importDialog.showModal();
});
$('#import-export-first').addEventListener('click', () => {
  if (importing) exportSave(importing, 'sav');
});

// The cloud ------------------------------------------------------------------

const cloudPanel = $('#cloud');
const cloudStatus = $('#cloud-status');

function setCloud(state, email) {
  cloudPanel.dataset.state = state;
  for (const part of $$('[data-when]', cloudPanel)) part.hidden = part.dataset.when !== state;
  if (email) {
    $('#cloud-sent-to').textContent = email;
    $('#cloud-user').textContent = email;
    $('#cloud-avatar').textContent = email[0];
  }
}

function syncState(text, kind = '') {
  $('#cloud-sync-state').textContent = text;
  $('#cloud-px').className = `px ${kind}`;
}

async function openCloud() {
  if (!cloud.enabled) return;
  $('#tab-cloud').hidden = false;
  const error = cloud.takeSignIn();
  await showAccount();
  if (error) {
    selectTab($('#tab-cloud'));
    say($('#cloud-form-status'), `The sign-in link did not work: ${error}`, 'error');
  }
  if (signedIn && rom) await runSync();
}

async function showAccount() {
  const email = await cloud.account().catch(() => null);
  signedIn = Boolean(email);
  setCloud(signedIn ? 'signed-in' : 'signed-out', email);
}

async function sendLink(email, status) {
  say(status, 'Sending…');
  try {
    await cloud.sendLink(email);
    return true;
  } catch (error) {
    say(status, `The link couldn't be sent: ${error.message}`, 'error');
    return false;
  }
}

$('#cloud-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const input = $('#cloud-email');
  const status = $('#cloud-form-status');
  if (!input.checkValidity()) {
    input.setAttribute('aria-invalid', 'true');
    say(status, 'Enter an email address, like you@example.com.', 'error');
    input.focus();
    return;
  }
  input.removeAttribute('aria-invalid');
  if (await sendLink(input.value, status)) {
    say(status, '');
    setCloud('link-sent', input.value);
    $('#cloud-resend').focus();
  }
});
$('#cloud-resend').addEventListener('click', async () => {
  const email = $('#cloud-sent-to').textContent;
  const status = $('#cloud-sent-status');
  if (await sendLink(email, status)) say(status, `Sent again to ${email}.`, 'ok');
});
$('#cloud-other').addEventListener('click', () => {
  setCloud('signed-out');
  $('#cloud-email').focus();
});
$('#cloud-signout').addEventListener('click', async () => {
  await cloud.signOut();
  conflicts = new Map();
  await showAccount();
  renderSaves();
  say($('#cloud-form-status'), 'Signed out; the saves stay in this browser.', 'ok');
  $('#cloud-email').focus();
});
$('#cloud-sync').addEventListener('click', () => runSync());

// Syncs the saves with the cloud; `uploadsOnly` while the game plays, so
// nothing changes under it.
async function runSync(uploadsOnly = false) {
  if (!signedIn || !rom) return;
  if (!navigator.onLine) {
    syncState('Offline · will sync later', 'off');
    return;
  }
  syncState('Syncing…', 'busy');
  try {
    const found = await sync.syncAll(rom, { uploadsOnly });
    if (!uploadsOnly) conflicts = found;
    const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    syncState(`Synced at ${time}`);
    if (conflicts.size) {
      say(cloudStatus, `${conflicts.size} ${conflicts.size === 1 ? 'save was' : 'saves were'} changed here and in the cloud: choose which to keep under Saves.`, 'info');
    } else {
      say(cloudStatus, '');
    }
    if (!launcher.hidden) renderSaves();
  } catch (error) {
    syncState('Sync failed · try again', 'err');
    say(cloudStatus, `The cloud could not be reached: ${error.message}`, 'error');
  }
}

async function settle(key, conflict, keepBrowser) {
  await sync.settle(rom, key, conflict, keepBrowser);
  conflicts.delete(key);
  if (!conflicts.size) say(cloudStatus, '');
  renderSaves();
}

const deleteDialog = $('#delete-dialog');
const deleteText = $('#delete-confirm-text');
$('#cloud-delete').addEventListener('click', () => {
  deleteText.value = '';
  $('#delete-confirm').disabled = true;
  deleteDialog.returnValue = '';
  deleteDialog.showModal();
});
deleteText.addEventListener('input', () => {
  $('#delete-confirm').disabled = deleteText.value.trim().toUpperCase() !== 'DELETE';
});
deleteDialog.addEventListener('close', async () => {
  if (deleteDialog.returnValue !== 'confirm') return;
  try {
    await cloud.deleteAccount();
    conflicts = new Map();
    await showAccount();
    renderSaves();
    say($('#cloud-form-status'), 'Your account and its cloud saves are deleted.', 'ok');
  } catch (error) {
    say(cloudStatus, `The account couldn't be deleted: ${error.message}`, 'error');
  }
});

// Offline and installing -------------------------------------------------------

function netState() {
  const element = $('#net-state');
  const cached = Boolean(navigator.serviceWorker?.controller);
  element.lastChild.textContent = !navigator.onLine ? 'Offline' : cached ? 'Ready offline' : 'Online';
  element.firstChild.className = `px${!navigator.onLine ? ' off' : cached ? '' : ' info'}`;
}

addEventListener('online', netState);
addEventListener('offline', () => {
  netState();
  if (signedIn) syncState('Offline · will sync later', 'off');
});
let installPrompt = null;
addEventListener('beforeinstallprompt', (event) => {
  event.preventDefault();
  installPrompt = event;
  $('#install').hidden = false;
});
$('#install').addEventListener('click', async () => {
  if (!installPrompt) return;
  installPrompt.prompt();
  await installPrompt.userChoice;
  installPrompt = null;
  $('#install').hidden = true;
});

// Touch screens show the pad's options in the pause menu and its help.
const markTouch = () => document.documentElement.classList.add('touch');
if (matchMedia('(pointer: coarse)').matches) markTouch();
addEventListener('touchstart', markTouch, { once: true, passive: true });

// Playing ------------------------------------------------------------------

const stage = $('#stage');
const canvas = $('#screen');
const pause = $('#pause-dialog');

// Whether the on-screen pad will likely show: chosen always, or a touch
// screen with the automatic choice.
function padLikely() {
  const touch = option('touch');
  return touch === 'on' || (touch === 'auto' && matchMedia('(pointer: coarse)').matches);
}

// The whole screen for the game and its pad, where the browser allows a
// page to take it (not every phone's browser does).
function fillScreen() {
  const page = document.documentElement;
  if (!page.requestFullscreen || document.fullscreenElement) return;
  page.requestFullscreen({ navigationUI: 'hide' }).catch(() => {});
}

function begin() {
  if (!rom || session) return;
  closeExport();
  saveOptions();
  const settings = [
    `mode=${option('mode')}`,
    ...ENHANCEMENTS.map((name) => `${name}=${option(name)}`),
    `color=${option('color')}`,
    `trail=${option('trail')}`,
    `upscale=${option('upscale')}`,
    `filter=${option('filter')}`,
    `volume=${option('volume')}`,
    `muted=${muted() ? 1 : 0}`,
    `scaling=${option('scaling')}`,
    `touch=${option('touch')}`,
    `touch-size=${option('touch-size')}`,
    `touch-opacity=${option('touch-opacity')}`,
  ].join('\n');
  launcher.hidden = true;
  stage.hidden = false;
  document.documentElement.style.overflow = 'hidden';
  canvas.classList.toggle('smooth', option('scaling') === 'smooth');
  if (padLikely()) fillScreen();
  try {
    const chosen = language.value === '' ? undefined : translation ?? undefined;
    session = start(rom, settings, chosen, canvas, $('#pad'), $('#game-menu'), $('#grid'));
    if (signedIn) {
      setInterval(() => runSync(true), SYNC_EVERY_MS);
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') runSync(true);
      });
    }
  } catch (error) {
    session = null;
    stage.hidden = true;
    launcher.hidden = false;
    document.documentElement.style.overflow = '';
    setRom('error', 'The game could not start.', String(error));
  }
}

play.addEventListener('click', begin);
$('#playbar-play').addEventListener('click', begin);
// Leaving the game, from its own menu or the page's, starts the page anew.
addEventListener('re-zoids-saga:left', () => window.location.reload());
// The enhanced mode's settings changed in the game's pause menu: kept for
// the next game.
addEventListener('re-zoids-saga:enhancements', (event) => {
  for (const line of String(event.detail).split('\n')) {
    const [name, value] = line.split('=');
    if (ENHANCEMENTS.includes(name)) setOption(name, value);
  }
  saveOptions();
});

function openPause() {
  if (!session || pause.open) return;
  session.pause();
  $('#q-volume').value = option('volume');
  $('#q-opacity').value = option('touch-opacity');
  $('#q-pad').checked = option('touch') !== 'off';
  for (const radio of $$('input[name="q-scaling"]')) radio.checked = radio.value === option('scaling');
  pause.returnValue = '';
  pause.showModal();
}

$('#game-menu').addEventListener('click', openPause);
pause.addEventListener('cancel', () => {
  pause.returnValue = 'resume';
});
pause.addEventListener('close', () => {
  if (pause.returnValue === 'quit') session?.quit();
  else session?.resume();
});
$('#q-volume').addEventListener('input', (event) => {
  setOption('volume', event.target.value);
  $('#volume-out').value = `${event.target.value}%`;
  session?.set_volume(Number(event.target.value));
  saveOptions();
});
$('#q-opacity').addEventListener('input', (event) => {
  setOption('touch-opacity', event.target.value);
  session?.set_touch_opacity(Number(event.target.value));
  saveOptions();
});
$('#q-pad').addEventListener('change', (event) => {
  const kept = readJson(OPTIONS_KEY, {}).touch;
  const touch = event.target.checked ? (kept && kept !== 'off' ? kept : 'auto') : 'off';
  setOption('touch', touch);
  session?.set_touch(touch);
  saveOptions();
});
for (const radio of $$('input[name="q-scaling"]')) {
  radio.addEventListener('change', () => {
    setOption('scaling', radio.value);
    canvas.classList.toggle('smooth', radio.value === 'smooth');
    session?.set_scaling(radio.value);
    saveOptions();
  });
}
addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && session && !pause.open) {
    event.preventDefault();
    openPause();
  }
});
// The pause button shows for a while after the mouse moves.
let showTimer;
stage.addEventListener('pointermove', (event) => {
  if (event.pointerType !== 'mouse') return;
  stage.classList.add('show-ui');
  clearTimeout(showTimer);
  showTimer = setTimeout(() => stage.classList.remove('show-ui'), 2000);
});
// The pause button takes its taps itself, not as the pad's fingers.
$('#game-menu').addEventListener('pointerdown', (event) => event.stopPropagation());

// A gamepad on the page (the game reads the pads itself) --------------------

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]):not([tabindex="-1"]), select:not([disabled]), summary, [tabindex="0"]';

// What a gamepad moves through: an open dialog, the export menu or the
// launcher; nothing while the game plays.
function layer() {
  return $('dialog[open]') || (exportMenu.hidden ? null : exportMenu) || (launcher.hidden ? null : launcher);
}

function moveFocus(step) {
  const root = layer();
  if (!root) return;
  const items = $$(FOCUSABLE, root).filter((element) => element.offsetParent !== null || element === document.activeElement);
  const groups = new Set();
  const stops = items.filter((element) => {
    if (element.type !== 'radio') return true;
    if (groups.has(element.name)) return false;
    groups.add(element.name);
    return true;
  });
  const active = document.activeElement;
  let at = stops.indexOf(active);
  if (active?.type === 'radio') at = stops.findIndex((element) => element.name === active.name);
  let target = stops[(at + step + stops.length) % stops.length];
  if (target?.type === 'radio') target = $(`input[name="${target.name}"]:checked`) || target;
  target?.focus();
}

function sideways(step) {
  const element = document.activeElement;
  if (element?.getAttribute('role') === 'tab') {
    const tabs = visibleTabs();
    selectTab(tabs[(tabs.indexOf(element) + step + tabs.length) % tabs.length], true);
    return true;
  }
  if (element?.type === 'radio') {
    const group = $$(`input[name="${element.name}"]:not(:disabled)`);
    const next = group[(group.indexOf(element) + step + group.length) % group.length];
    next.checked = true;
    next.focus();
    next.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  }
  if (element?.type === 'range') {
    element.value = Number(element.value) + step * (Number(element.step) || 1);
    element.dispatchEvent(new Event('input', { bubbles: true }));
    element.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  }
  return false;
}

function padPress(button) {
  if (!layer()) return;
  if (button === 'up') moveFocus(-1);
  else if (button === 'down') moveFocus(1);
  else if (button === 'left') { if (!sideways(-1)) moveFocus(-1); }
  else if (button === 'right') { if (!sideways(1)) moveFocus(1); }
  else if (button === 'a') document.activeElement?.click();
  else if (button === 'b') {
    const open = $('dialog[open]');
    if (open) open.close(open === pause ? 'resume' : 'cancel');
    else closeExport(true);
  }
}

let padBefore = {};
let polling = false;

// Reads the first gamepad once a picture, in the standard mapping.
function pollPad() {
  const pad = [...(navigator.getGamepads?.() ?? [])].find(Boolean);
  if (!pad) {
    polling = false;
    return;
  }
  const pressed = (index) => Boolean(pad.buttons[index]?.pressed);
  const now = {
    up: pressed(12) || pad.axes[1] < -0.6,
    down: pressed(13) || pad.axes[1] > 0.6,
    left: pressed(14) || pad.axes[0] < -0.6,
    right: pressed(15) || pad.axes[0] > 0.6,
    a: pressed(1),
    b: pressed(0),
  };
  for (const button of Object.keys(now)) {
    if (now[button] && !padBefore[button]) padPress(button);
  }
  padBefore = now;
  requestAnimationFrame(pollPad);
}

addEventListener('gamepadconnected', () => {
  if (!polling) {
    polling = true;
    pollPad();
  }
});

// On a phone the Play bar stays in reach once the button scrolls away.
if ('IntersectionObserver' in window) {
  new IntersectionObserver(([entry]) => document.body.classList.toggle('play-offscreen', !entry.isIntersecting)).observe(play);
}

// Starting ---------------------------------------------------------------------

if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('sw.js').then(netState, () => {});
}
await offerLanguages();
restoreOptions();
await restoreTranslation();
saveOptions();
await restoreRom();
await openCloud();
const savedTab = $(`#${readJson(TAB_KEY, 'tab-options')}`);
if (savedTab && !savedTab.hidden) selectTab(savedTab);
netState();
