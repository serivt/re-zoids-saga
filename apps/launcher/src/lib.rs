//! Launcher: ROM picker, title detection, transparent extraction and game start.
//!
//! [`run`] is the whole program: the desktop's executable passes it the
//! command line, Android's library an empty one.

mod download;
mod fast;
mod front;
mod mute;
mod pacing;
mod quit;
mod saves;
mod settings;
mod update;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use extraction::{Identification, Title};
use game_core::{
    DEFAULT_PLAYER_NAME, Event as GameEvent, Extension, Field, Game, GameData, PlayMode, Scope,
    ScriptRunner, ScriptWindows, TextPainter, Translation, WindowPainter,
};
use gba_runtime::apu::{SAMPLE_RATE, SAMPLES_PER_FRAME};
use gba_runtime::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use platform::{AudioOut, Display, Event, Frame, Input, Rgb};
use platform_sdl3::{FileStorage, Sdl3Display, autosave_path, preferences_dir, slot_path};

/// The function key that turns the debugging mode on or off, only in a
/// build with the `debug-mode` feature.
#[cfg(feature = "debug-mode")]
const DEBUG_KEY: u8 = 10;
const USAGE: &str = "usage: launcher [<rom-path> [string-id]] [--version] [--room] [--dump <frame.ppm>] [--save <file.sav>] [--slots <n>] [--translation <file.po>] [--touch] [--export-template <file.pot> [table[:first-last]...]]\n  without a ROM the launcher shows its own screen to choose the ROM, a translation and the options (keys, gamepad buttons, window size, fullscreen, filter, volume), remembered in the user's settings folder, which the game given a ROM here plays with too; without a string id the launcher boots the game (arrows move, X = A, Z = B, Return = Start, Backspace = Select, A = L, S = R, Space = the enhanced mode's fast forward and M = mute (a press turns the sound off or on again) by default, or the keys chosen in the launcher's options, and any gamepad: its D-pad or left stick moves, its right face button is A, the bottom one B, Start, Back = Select, the shoulders L and R and the right stick's click the fast forward, unless chosen otherwise; Esc asks whether to quit; in a build with the debug-mode feature, F10 turns a debugging mode on and off: the roaming enemies are intangible, to walk through them without battles, and the protagonist's attacks beat what they hit); --room skips to the first room; --save keeps the save in that file instead of next to the ROM with the extension .sav, the way emulators do; --slots sets the save slots (4 by default, 1 for the original's single save): slot 1 is that .sav and slot n the same name with .n before the extension, each a save an emulator can load; --version prints the port's version; --translation shows the messages of a PO file; --touch shows the on-screen pad for touch screens, as Android always does (with SDL_MOUSE_TOUCH_EVENTS=1 the mouse plays the fingers); --export-template writes the PO template of the given tables (title, name-entry, pause-menu, dialogue, battle, battle-text, battle-menu, battle-label, item, name, part, system, zoid-guide, character-guide), and the port's own messages (port), by default the title, the name entry, dialogue 30-41 and the port's messages";
const DEFAULT_TEMPLATE_SCOPES: [&str; 4] = ["title", "name-entry", "dialogue:30-41", "port"];
const WINDOW_SCALE: u32 = 3;
const FIRST_ROOM_MAP: usize = extraction::saga::FIRST_ROOM_MAP;
const PLAYER_START: (usize, usize) = extraction::saga::PLAYER_START;
const FRAME_DURATION: std::time::Duration = std::time::Duration::from_micros(16_743);
const AUDIO_QUEUE_FRAMES: usize = 6;
const RENDER_FRAME_LIMIT: usize = 600;
/// The most save slots the list shows without scrolling.
const MAX_SLOTS: usize = 9;

/// Reports on the console what the game could not read or write.
struct StorageReport;

impl Extension for StorageReport {
    fn name(&self) -> &'static str {
        "launcher-storage-report"
    }

    fn on_event(&mut self, event: &GameEvent) {
        match event {
            GameEvent::StorageFailed(reason) => eprintln!("Save:       {reason}"),
            GameEvent::AchievementUnlocked(key) => println!("Achievement: {key}"),
            _ => {}
        }
    }
}

/// Remembers the enhanced mode's settings the player changes in the pause
/// menu, so the launcher shows them and the next game plays with them.
struct ModeKeeper;

impl Extension for ModeKeeper {
    fn name(&self) -> &'static str {
        "launcher-mode-keeper"
    }

    fn on_event(&mut self, event: &GameEvent) {
        let GameEvent::EnhancementsChanged(enhancements) = event else {
            return;
        };
        let Some(path) = settings_path() else {
            return;
        };
        let mut settings = settings::Settings::load(&path);
        settings.mode.enhancements = *enhancements;
        if let Err(error) = settings.store(&path) {
            eprintln!("Settings:   {error}");
        }
    }
}

/// Runs the launcher with `args`, the command line without the program's
/// name: without a ROM it shows its own screen, with one it plays it or
/// does what the options ask.
///
/// # Errors
///
/// Returns an error when the options, the ROM or a translation cannot be
/// read, or the window cannot be opened.
pub fn run(args: impl IntoIterator<Item = OsString>) -> Result<()> {
    let options = Options::parse(args)?;
    if options.version {
        println!("{} {}", front::PROJECT_NAME, front::VERSION);
        return Ok(());
    }
    if options.rom_path.is_none()
        && let Some(path) = &options.dump_path
    {
        let settings = saved_settings();
        let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
        front::Front::new(&settings).draw(&mut frame);
        return write_ppm(path, &frame);
    }
    let from_front = options.rom_path.is_none();
    let mut shown = None;
    loop {
        let mut display = None;
        let mut settings = None;
        let mut game_options = options.clone();
        if from_front {
            let mut window = match shown.take() {
                Some(window) => window,
                None => open_window(
                    front::PROJECT_NAME,
                    SCREEN_WIDTH,
                    SCREEN_HEIGHT,
                    saved_settings().scale,
                )?,
            };
            let Some(choice) = front::run(&mut window, settings_path().as_deref())? else {
                return Ok(());
            };
            game_options.rom_path = Some(choice.rom);
            game_options.translation = choice.translation.or(game_options.translation);
            settings = Some(choice.settings);
            display = Some(window);
        }
        match launch(&game_options, display, settings)? {
            Some(window) if from_front => shown = Some(window),
            _ => return Ok(()),
        }
    }
}

/// Plays the ROM `options` name in `display`, the launcher's window, or a
/// new one, with `settings` or those saved; returns the window when the
/// player left the game for the launcher, `None` once it is over.
fn launch(
    options: &Options,
    display: Option<Sdl3Display>,
    settings: Option<settings::Settings>,
) -> Result<Option<Sdl3Display>> {
    let rom_path = options.rom_path.clone().context(USAGE)?;
    let rom = std::fs::read(&rom_path)
        .with_context(|| format!("cannot read ROM {}", rom_path.display()))?;
    let identification = extraction::identify(&rom)
        .with_context(|| format!("cannot identify ROM {}", rom_path.display()))?;
    print_identification(&identification);
    let kind = front::RomKind::identified(&identification);
    if !kind.playable() {
        bail!(
            "{}",
            game_core::port_text::default_text(kind.message()).unwrap_or_default()
        );
    }
    let title = identification.title.to_string();
    if let Some(string_id) = &options.string_id {
        let frame = render_string(&rom, identification.title, string_id)?;
        return match &options.dump_path {
            Some(path) => write_ppm(path, &frame),
            None => show(&title, &frame),
        }
        .map(|()| None);
    }
    if let Some((path, scopes)) = &options.template {
        return export_template(&rom, path, scopes).map(|()| None);
    }
    let mut game = if options.room || options.dump_path.is_some() {
        Game::in_first_room(&rom)?
    } else {
        Game::new(&rom)?
    };
    let save_path = options
        .save_path
        .clone()
        .unwrap_or_else(|| rom_path.with_extension(saves::SAVE_EXTENSION));
    keep_saves(&mut game, &save_path, options.slots);
    game.extensions()
        .borrow_mut()
        .insert(Box::new(StorageReport));
    game.extensions().borrow_mut().insert(Box::new(ModeKeeper));
    let achievements = saves::achievements_file(&save_path);
    println!("Save:       achievements in {}", achievements.display());
    game.set_achievement_storage(Box::new(FileStorage::new(achievements)));
    if let Some(path) = &options.translation {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read translation {}", path.display()))?;
        let translation = Translation::from_po(&text)
            .with_context(|| format!("cannot parse translation {}", path.display()))?;
        let messages = translation.len();
        let problems = game.set_translation(translation)?;
        println!("Translation: {messages} messages from {}", path.display());
        for problem in problems {
            eprintln!("Translation: {problem}");
        }
    }
    let settings = settings.unwrap_or_else(saved_settings);
    game.set_play_mode(settings.mode.play_mode());
    println!("Mode:       {:?}", settings.mode.play_mode());
    match &options.dump_path {
        Some(path) => {
            let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
            game.draw(&mut frame);
            write_ppm(path, &frame).map(|()| None)
        }
        None => play(display, &mut game, &settings, options.touch),
    }
}

/// The window's icon (`assets/icons`, see `tools/package/icons.py`).
const ICON: &[u8] = include_bytes!("../../../assets/icons/re-zoids-saga-128.rgba");
const ICON_SIDE: u32 = 128;

/// Opens the window, with the project's icon; a missing icon is only
/// reported.
/// Keeps the game's `slots` save slots and its autosave beside the save
/// at `save_path` (see `docs/formats/save.md`).
fn keep_saves(game: &mut Game<'_>, save_path: &Path, slots: usize) {
    let slots: Vec<Box<dyn platform::SaveStorage>> = (0..slots)
        .map(|slot| {
            let path = slot_path(save_path, slot);
            println!("Save:       slot {} in {}", slot + 1, path.display());
            Box::new(FileStorage::new(path)) as Box<dyn platform::SaveStorage>
        })
        .collect();
    game.set_save_slots(slots);
    let autosave = autosave_path(save_path);
    println!("Save:       autosave in {}", autosave.display());
    game.set_autosave_storage(Box::new(FileStorage::new(autosave)));
}

fn open_window(title: &str, width: usize, height: usize, scale: u32) -> Result<Sdl3Display> {
    let mut display = Sdl3Display::open(title, width, height, scale)?;
    if let Err(error) = display.set_icon(ICON, ICON_SIDE) {
        eprintln!("Icon:       {error}");
    }
    Ok(display)
}

/// The settings the launcher's screen remembered, or the defaults.
fn saved_settings() -> settings::Settings {
    settings_path()
        .map(|path| settings::Settings::load(&path))
        .unwrap_or_default()
}

/// The launcher's settings file in the user's settings folder.
fn settings_path() -> Option<PathBuf> {
    match preferences_dir(settings::ORGANIZATION, settings::APP) {
        Ok(folder) => Some(folder.join(settings::FILE_NAME)),
        Err(error) => {
            eprintln!("Settings:   {error}");
            None
        }
    }
}

#[derive(Clone)]
struct Options {
    rom_path: Option<PathBuf>,
    string_id: Option<String>,
    dump_path: Option<PathBuf>,
    save_path: Option<PathBuf>,
    slots: usize,
    room: bool,
    translation: Option<PathBuf>,
    template: Option<(PathBuf, Vec<String>)>,
    version: bool,
    touch: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut args = args.into_iter().peekable();
        let rom_path = args
            .next_if(|arg| !arg.to_string_lossy().starts_with("--"))
            .map(PathBuf::from);
        let mut string_id = None;
        let mut dump_path = None;
        let mut save_path = None;
        let mut slots = game_core::slots::DEFAULT_SLOTS;
        let mut room = false;
        let mut translation = None;
        let mut template = None;
        let mut version = false;
        let mut touch = cfg!(target_os = "android");
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--dump") => dump_path = Some(args.next().map(PathBuf::from).context(USAGE)?),
                Some("--room") => room = true,
                Some("--version") => version = true,
                Some("--touch") => touch = true,
                Some("--save") => save_path = Some(args.next().map(PathBuf::from).context(USAGE)?),
                Some("--slots") => {
                    slots = args
                        .next()
                        .and_then(|count| count.to_str()?.parse().ok())
                        .filter(|count| (1..=MAX_SLOTS).contains(count))
                        .context(USAGE)?;
                }
                Some("--translation") => {
                    translation = Some(args.next().map(PathBuf::from).context(USAGE)?);
                }
                Some("--export-template") => {
                    let path = args.next().map(PathBuf::from).context(USAGE)?;
                    let scopes: Vec<String> = args
                        .by_ref()
                        .map(|scope| scope.to_string_lossy().into_owned())
                        .collect();
                    template = Some((path, scopes));
                }
                Some(id) if !id.starts_with("--") => string_id = Some(id.to_owned()),
                _ => bail!(USAGE),
            }
        }
        Ok(Self {
            rom_path,
            string_id,
            dump_path,
            save_path,
            slots,
            room,
            translation,
            template,
            version,
            touch,
        })
    }
}

fn print_identification(identification: &Identification) {
    let header = &identification.header;
    println!("Title:      {}", identification.title);
    println!(
        "Header:     {} ({}, {} v{})",
        header.title, header.game_code, header.maker_code, header.version
    );
    println!("SHA-1:      {}", identification.sha1);
    match identification.known_release {
        Some(release) => println!(
            "Dump:       verified ({} rev {})",
            release.region, release.version
        ),
        None => println!("Dump:       not the supported dump"),
    }
}

fn render_string(rom: &[u8], title: Title, string_id: &str) -> Result<Frame> {
    if title != Title::Saga {
        bail!("text rendering is only implemented for {}", Title::Saga);
    }
    let (table_name, index) = string_id
        .rsplit_once('_')
        .and_then(|(name, index)| Some((name, index.parse::<usize>().ok()?)))
        .with_context(|| format!("string id {string_id:?} is not <table>_<index>"))?;
    let table = extraction::saga::string_table(table_name)
        .with_context(|| format!("unknown string table {table_name:?}"))?;
    let strings = table.read(rom)?;
    let mut runner = ScriptRunner::new(strings.iter().map(|string| string.offset).collect());
    let mut windows = ScriptWindows::new(rom, DEFAULT_PLAYER_NAME);
    runner.start(index)?;
    for _ in 0..RENDER_FRAME_LIMIT {
        if runner.is_waiting_for_key()
            || runner.update(rom, platform::Input::default(), &mut windows)?
        {
            break;
        }
    }
    let (glyphs, fallback) = extraction::saga::font(rom)?;
    let painter = TextPainter::new(rom, glyphs, Some(fallback));
    let skin = extraction::saga::window_skin(rom)?;
    let window = WindowPainter::new(skin.tiles, &skin.palette);
    let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::new(16, 24, 48));
    Field::load(&GameData::new(rom), FIRST_ROOM_MAP, PLAYER_START)?.draw(&mut frame);
    windows.draw(&mut frame, &window, &painter);
    Ok(frame)
}

fn show(title: &str, frame: &Frame) -> Result<()> {
    let mut display = open_window(title, frame.width(), frame.height(), WINDOW_SCALE)?;
    loop {
        if display
            .poll_events()
            .iter()
            .any(|event| matches!(event, Event::Quit | Event::Back))
        {
            return Ok(());
        }
        display.present(frame)?;
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

/// Runs the game in `display`, the launcher's window that already has
/// `settings`, or a new one given them; the sound plays at their volume and
/// the picture with their colors (the marks the launcher draws over it keep
/// their own).
/// Escape pauses the game to ask before closing; after staying, the game
/// sees no button until all are released. Returns the window when the
/// player chose to leave for the launcher, `None` when it closed.
fn play(
    display: Option<Sdl3Display>,
    game: &mut Game<'_>,
    settings: &settings::Settings,
    touch: bool,
) -> Result<Option<Sdl3Display>> {
    let mut display = match display {
        Some(display) => display,
        None => game_window(settings)?,
    };
    let mut fast = fast::FastForward::default();
    let mut mute = mute::Mute::default();
    let colors = screen_filters::ColorCorrection::new(settings.color);
    display.set_touch_fast_forward(settings.mode.enhanced);
    display.set_touch_pad(touch)?;
    let volume = i32::from(settings.volume);
    let mut audio = match display.open_audio(SAMPLE_RATE) {
        Ok(audio) => Some(audio),
        Err(error) => {
            eprintln!("no audio: {error}");
            None
        }
    };
    let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
    let metrics = game_core::TextMetrics::standard();
    let mut quitting: Option<quit::QuitPrompt> = None;
    let mut settling = false;
    let mut pacer = pacing::Pacer::new(FRAME_DURATION, std::time::Instant::now());
    loop {
        let now = std::time::Instant::now();
        let due = pacer.due(now);
        if due == 0 {
            std::thread::sleep(pacer.until_next(now));
            continue;
        }
        for event in display.poll_events() {
            match event {
                Event::Quit => return Ok(None),
                Event::Back => {
                    quitting = match quitting {
                        Some(_) => None,
                        None => Some(quit::QuitPrompt::new(display.input(), true)),
                    };
                    settling = quitting.is_none();
                }
                #[cfg(feature = "debug-mode")]
                Event::FunctionKey(DEBUG_KEY) => {
                    let on = game.toggle_debug_mode();
                    eprintln!(
                        "debug mode {}",
                        if on {
                            "on: intangible enemies, overwhelming protagonist"
                        } else {
                            "off"
                        }
                    );
                }
                Event::FunctionKey(_) | Event::Key(_) | Event::Pad(_) | Event::Pointer { .. } => {}
            }
        }
        let speed = match game.play_mode() {
            PlayMode::Classic => 1,
            PlayMode::Enhanced(enhancements) => u32::from(enhancements.fast_forward),
        };
        let input = mute.take(display.input());
        let input = fast.take(input, speed, quitting.is_some());
        if let Some(prompt) = &mut quitting {
            match prompt.update(input) {
                Some(true) => return Ok(None),
                Some(false) => {
                    quitting = None;
                    settling = true;
                }
                None => {}
            }
        }
        settling &= input != Input::default();
        game.set_time_scale(fast.frames(1));
        for _ in 0..fast.frames(due) {
            if quitting.is_some() {
                break;
            }
            game.update(if settling { Input::default() } else { input })?;
            if let Some(audio) = &mut audio
                && audio.queued_pairs() < SAMPLES_PER_FRAME * AUDIO_QUEUE_FRAMES
            {
                queue_audio(audio, game.audio(), mute.volume(volume))?;
            }
        }
        if game.wants_to_leave() {
            display.set_touch_pad(false)?;
            return Ok(Some(display));
        }
        game.draw(&mut frame);
        colors.apply(&mut frame);
        let taken = mute.draw(&mut frame, &metrics);
        fast.draw(&mut frame, &metrics, taken);
        if let Some(prompt) = quitting {
            prompt.draw(&mut frame, &metrics, &|key| {
                game_core::port_text::port_text(game.extensions(), key)
            });
        }
        display.present(&frame)?;
    }
}

/// A window of its own for a game the command line named, with
/// `settings`.
fn game_window(settings: &settings::Settings) -> Result<Sdl3Display> {
    let mut display = open_window(
        front::PROJECT_NAME,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        settings.scale,
    )?;
    front::apply(&mut display, settings)?;
    Ok(display)
}

/// Queues a frame's `samples` at `volume` percent.
fn queue_audio(audio: &mut impl AudioOut, samples: &[i16], volume: i32) -> Result<()> {
    let full = i32::from(settings::FULL_VOLUME);
    if volume == full {
        audio.queue(samples)?;
    } else {
        let quieter: Vec<i16> = samples
            .iter()
            .map(|&sample| i16::try_from(i32::from(sample) * volume / full).unwrap_or(sample))
            .collect();
        audio.queue(&quieter)?;
    }
    Ok(())
}

fn export_template(rom: &[u8], path: &Path, scopes: &[String]) -> Result<()> {
    let names: Vec<&str> = if scopes.is_empty() {
        DEFAULT_TEMPLATE_SCOPES.to_vec()
    } else {
        scopes.iter().map(String::as_str).collect()
    };
    let scopes = names
        .iter()
        .map(|name| Scope::parse(name))
        .collect::<Result<Vec<_>, _>>()?;
    let template = game_core::translation::template(&GameData::new(rom), &scopes)?;
    std::fs::write(path, &template)
        .with_context(|| format!("cannot write template {}", path.display()))?;
    println!(
        "Template:   {} messages written to {}",
        template.matches("\nmsgctxt ").count(),
        path.display()
    );
    Ok(())
}

fn write_ppm(path: &Path, frame: &Frame) -> Result<()> {
    let mut bytes = format!("P6\n{} {}\n255\n", frame.width(), frame.height()).into_bytes();
    bytes.extend(frame.to_rgb24());
    std::fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
