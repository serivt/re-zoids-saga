// The page around the game: choosing the ROM (read here, never sent),
// the translation and the options, then the canvas scaled to the window.
import init, { check_rom, start } from './pkg/re_zoids_saga_web.js';

const OPTIONS_KEY = 're-zoids-saga/page-options';
const SCREEN = { width: 240, height: 160 };

const $ = (id) => document.getElementById(id);
const menu = $('menu');
const stage = $('stage');
const canvas = $('screen');
const play = $('play');
const status = $('rom-status');
const fields = ['mode', 'scaling', 'color', 'trail', 'volume'];

let rom = null;

await init();
restoreOptions();

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
for (const id of fields) {
  $(id).addEventListener('change', saveOptions);
}
play.addEventListener('click', begin);
window.addEventListener('resize', fit);
window.addEventListener('re-zoids-saga:left', () => window.location.reload());

async function takeRom(file) {
  if (!file) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  const check = check_rom(bytes);
  status.textContent = check.message;
  status.className = `status ${check.playable ? 'good' : 'bad'}`;
  rom = check.playable ? bytes : null;
  play.disabled = !rom;
}

async function begin() {
  if (!rom) return;
  const file = $('translation').files[0];
  const translation = file ? await file.text() : undefined;
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
    start(rom, settings, translation, canvas);
  } catch (error) {
    stage.hidden = true;
    menu.hidden = false;
    status.textContent = `The game could not start: ${error}`;
    status.className = 'status bad';
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

function restoreOptions() {
  try {
    const options = JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? '{}');
    for (const id of fields) {
      if (options[id] !== undefined) $(id).value = options[id];
    }
  } catch {
    // Nothing kept, or not readable: the defaults stay.
  }
}
