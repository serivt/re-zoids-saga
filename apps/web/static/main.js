// The page around the game: the ROM (read here, kept in this browser and
// never sent), the translation (downloaded from the translations'
// repository or chosen as a file, and kept), the options, the saves, then
// the canvas scaled to the window.
import init, { check_rom, save_summary, start } from './pkg/re_zoids_saga_web.js';
import * as store from './store.js';
import * as saves from './saves.js';

const OPTIONS_KEY = 're-zoids-saga/page-options';
const TRANSLATIONS = 'https://raw.githubusercontent.com/serivt/re-zoids-saga-translations/main/';
const INDEX = 'po/languages.json';
const SCREEN = { width: 240, height: 160 };

const $ = (id) => document.getElementById(id);
const menu = $('menu');
const stage = $('stage');
const canvas = $('screen');
const play = $('play');
const romStatus = $('rom-status');
const translationStatus = $('translation-status');
const saveStatus = $('save-status');
const fields = ['language', 'mode', 'scaling', 'color', 'trail', 'volume'];

let rom = null;
let romName = 'Zoids Saga';
let translation = null;

await init();
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('sw.js').catch(() => {});
}
await offerLanguages();
restoreOptions();
await restoreRom();
await restoreTranslation();

$('rom').addEventListener('change', (event) => takeRom(event.target.files[0]));
const drop = $('drop');
drop.addEventListener('dragover', (event) => {
  event.preventDefault();
  drop.classList.add('over');
});
drop.addEventListener('dragleave', () => drop.classList.remove('over'));
drop.addEventListener('drop', (event) => {
  event.preventDefault();
  drop.classList.remove('over');
  takeRom(event.dataTransfer.files[0]);
});
$('forget-rom').addEventListener('click', forgetRom);
$('language').addEventListener('change', chooseLanguage);
$('translation-file').addEventListener('change', (event) => takeTranslationFile(event.target.files[0]));
for (const id of fields) {
  $(id).addEventListener('change', saveOptions);
}
$('import-file').addEventListener('change', importSave);
play.addEventListener('click', begin);
window.addEventListener('resize', fit);
window.addEventListener('re-zoids-saga:left', () => window.location.reload());

// The ROM ------------------------------------------------------------------

async function takeRom(file) {
  if (!file) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (useRom(bytes, file.name, false)) {
    await store.put('rom', { name: file.name, bytes });
    await store.keep();
  }
}

// Takes `bytes` as the ROM when the port plays it; says what it is.
function useRom(bytes, name, remembered) {
  const check = check_rom(bytes);
  romStatus.textContent = remembered ? `${check.message} Remembered: ${name}.` : check.message;
  romStatus.className = `status ${check.playable ? 'good' : 'bad'}`;
  rom = check.playable ? bytes : null;
  romName = name.replace(/\.[^.]*$/, '') || romName;
  play.disabled = !rom;
  $('forget-rom').hidden = !rom;
  showSaves();
  return check.playable;
}

async function restoreRom() {
  const kept = await store.get('rom');
  if (kept?.bytes) useRom(new Uint8Array(kept.bytes), kept.name, true);
}

async function forgetRom() {
  await store.remove('rom');
  rom = null;
  play.disabled = true;
  $('forget-rom').hidden = true;
  $('saves').hidden = true;
  romStatus.textContent = 'The ROM is forgotten; the saves stay.';
  romStatus.className = 'status';
}

// The translation ------------------------------------------------------------

// Lists the languages the translations' repository offers, when online.
async function offerLanguages() {
  try {
    const index = await (await fetch(TRANSLATIONS + INDEX)).json();
    const select = $('language');
    for (const language of index.languages ?? []) {
      const option = new Option(language.name, `repo:${language.file}`);
      select.insertBefore(option, select.lastElementChild);
    }
  } catch {
    // Offline: the translation kept, or a file, still works.
  }
}

async function chooseLanguage() {
  const choice = $('language').value;
  $('translation-file-line').hidden = choice !== 'file';
  if (choice === '') {
    translation = null;
    await store.remove('translation');
    translationStatus.textContent = '';
  } else if (choice.startsWith('repo:')) {
    translationStatus.textContent = 'Downloading the translation…';
    translationStatus.className = 'status';
    try {
      const text = await (await fetch(TRANSLATIONS + choice.slice(5))).text();
      await keepTranslation($('language').selectedOptions[0].text, text);
    } catch {
      translationStatus.textContent = 'The translation could not be downloaded.';
      translationStatus.className = 'status bad';
    }
  }
}

async function takeTranslationFile(file) {
  if (file) await keepTranslation(file.name, await file.text());
}

async function keepTranslation(name, text) {
  translation = text;
  await store.put('translation', { name, text });
  translationStatus.textContent = `Translation: ${name}.`;
  translationStatus.className = 'status good';
}

// The translation kept, when the options chose one; offline, the
// language it came from is offered again from what is kept.
async function restoreTranslation() {
  const kept = await store.get('translation');
  const wanted = savedOptions().language ?? '';
  const select = $('language');
  if (kept?.text && wanted !== '') {
    if (![...select.options].some((option) => option.value === wanted)) {
      select.insertBefore(new Option(kept.name, wanted), select.lastElementChild);
    }
    select.value = wanted;
    translation = kept.text;
    translationStatus.textContent = `Translation: ${kept.name} (remembered).`;
    translationStatus.className = 'status good';
  }
  $('translation-file-line').hidden = select.value !== 'file';
}

// The saves ------------------------------------------------------------------

function showSaves() {
  $('saves').hidden = !rom;
  if (!rom) return;
  const rows = $('save-rows');
  rows.replaceChildren();
  for (const save of saves.SAVES) {
    const row = rows.insertRow();
    row.insertCell().textContent = save.label;
    row.insertCell().textContent = saves.summary(rom, save.key);
    const actions = row.insertCell();
    const held = saves.read(save.key) !== null;
    for (const [kind, label] of [['sav', '.sav'], ['srm', '.srm'], ['cartridge', 'Cartridge']]) {
      actions.append(button(label, !held, () => exportSave(save.key, kind)));
    }
    if (save.imports) {
      actions.append(button('Import…', false, () => askImport(save)));
    }
  }
}

function button(label, disabled, action) {
  const element = Object.assign(document.createElement('button'), {
    textContent: label,
    disabled,
    className: 'small',
  });
  element.addEventListener('click', action);
  return element;
}

function exportSave(key, kind) {
  const bytes = saves.exported(rom, key, kind);
  if (!bytes) return;
  saves.download(`${romName}.${kind === 'srm' ? 'srm' : 'sav'}`, bytes);
  saveStatus.textContent = 'The save was exported.';
  saveStatus.className = 'status good';
}

let importing = null;

function askImport(save) {
  importing = save;
  $('import-file').value = '';
  $('import-file').click();
}

async function importSave(event) {
  const file = event.target.files[0];
  if (!file || !importing) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  const held = save_summary(rom, bytes);
  if (!held) {
    saveStatus.textContent = 'That file is not a Zoids Saga save.';
    saveStatus.className = 'status bad';
    return;
  }
  const current = saves.summary(rom, importing.key);
  const question = `Replace ${importing.label} (${current}) with the file's save (${held})? `
    + 'The slot\'s save is kept as a backup.';
  if (!window.confirm(question)) return;
  saves.write(importing.key, bytes);
  saveStatus.textContent = 'The save was imported.';
  saveStatus.className = 'status good';
  showSaves();
}

// Playing ------------------------------------------------------------------

async function begin() {
  if (!rom) return;
  const settings = [
    `mode=${$('mode').value}`,
    `color=${$('color').value}`,
    `trail=${$('trail').value}`,
    `volume=${$('volume').value}`,
  ].join('\n');
  menu.hidden = true;
  stage.hidden = false;
  canvas.classList.toggle('smooth', $('scaling').value === 'smooth');
  fit();
  try {
    start(rom, settings, $('language').value === '' ? undefined : translation ?? undefined, canvas);
  } catch (error) {
    stage.hidden = true;
    menu.hidden = false;
    romStatus.textContent = `The game could not start: ${error}`;
    romStatus.className = 'status bad';
  }
}

// The canvas as large as the window allows: whole multiples of the
// screen for the sharp scaling, the whole window otherwise.
function fit() {
  const scale = Math.min(window.innerWidth / SCREEN.width, window.innerHeight / SCREEN.height);
  const whole = $('scaling').value === 'sharp' && scale >= 1 ? Math.floor(scale) : scale;
  canvas.style.width = `${SCREEN.width * whole}px`;
  canvas.style.height = `${SCREEN.height * whole}px`;
}

function saveOptions() {
  const options = Object.fromEntries(fields.map((id) => [id, $(id).value]));
  try {
    localStorage.setItem(OPTIONS_KEY, JSON.stringify(options));
  } catch {
    // A browser that keeps nothing for the page plays with the defaults.
  }
}

function savedOptions() {
  try {
    return JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? '{}');
  } catch {
    return {};
  }
}

function restoreOptions() {
  try {
    const options = savedOptions();
    for (const id of fields) {
      const value = options[id];
      if (value === undefined) continue;
      const field = $(id);
      if (field.tagName === 'SELECT' && ![...field.options].some((o) => o.value === value)) continue;
      field.value = value;
    }
  } catch {
    // Nothing kept, or not readable: the defaults stay.
  }
}
