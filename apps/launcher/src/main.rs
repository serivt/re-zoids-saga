//! Launcher: ROM picker, title detection, transparent extraction and game start.

mod front;
mod settings;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use extraction::{Identification, Title};
use game_core::{
    DEFAULT_PLAYER_NAME, Event as GameEvent, Extension, Field, Game, GameData, Scope, ScriptRunner,
    ScriptWindows, TextPainter, Translation, WindowPainter,
};
use gba_runtime::apu::{SAMPLE_RATE, SAMPLES_PER_FRAME};
use gba_runtime::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use platform::{AudioOut, Display, Event, Frame, Rgb};
use platform_sdl3::{FileStorage, Sdl3Display, preferences_dir, slot_path};

/// The function key that turns the debugging mode on or off.
const DEBUG_KEY: u8 = 10;
const USAGE: &str = "usage: launcher [<rom-path> [string-id]] [--room] [--dump <frame.ppm>] [--save <file.sav>] [--slots <n>] [--translation <file.po>] [--export-template <file.pot> [table[:first-last]...]]\n  without a ROM the launcher shows its own screen to choose the ROM and a translation, remembered in the user's settings folder; without a string id the launcher boots the game (arrows move, X = A, Z = B, Return = Start, Backspace = Select, A = L, S = R by default, or the keys chosen on the launcher's controls screen; Esc quits; F10 turns a debugging mode on and off: the roaming enemies are intangible, to walk through them without battles, and the protagonist's attacks beat what they hit); --room skips to the first room; --save keeps the save in that file instead of next to the ROM with the extension .sav, the way emulators do; --slots sets the save slots (4 by default, 1 for the original's single save): slot 1 is that .sav and slot n the same name with .n before the extension, each a save an emulator can load; --translation shows the messages of a PO file; --export-template writes the PO template of the given tables (title, name-entry, pause-menu, part, dialogue, system, zoid-guide, character-guide), and the port's own messages (port), by default the title, the name entry, dialogue 30-41 and the port's messages";
const DEFAULT_TEMPLATE_SCOPES: [&str; 4] = ["title", "name-entry", "dialogue:30-41", "port"];
const WINDOW_SCALE: u32 = 3;
const FIRST_ROOM_MAP: usize = extraction::saga::FIRST_ROOM_MAP;
const PLAYER_START: (usize, usize) = extraction::saga::PLAYER_START;
const FRAME_DURATION: std::time::Duration = std::time::Duration::from_micros(16_743);
const AUDIO_QUEUE_FRAMES: usize = 6;
const RENDER_FRAME_LIMIT: usize = 600;
const SAVE_EXTENSION: &str = "sav";
/// The most save slots the list shows without scrolling.
const MAX_SLOTS: usize = 9;

/// Reports on the console what the game could not read or write.
struct StorageReport;

impl Extension for StorageReport {
    fn name(&self) -> &'static str {
        "launcher-storage-report"
    }

    fn on_event(&mut self, event: &GameEvent) {
        if let GameEvent::StorageFailed(reason) = event {
            eprintln!("Save:       {reason}");
        }
    }
}

fn main() -> Result<()> {
    let mut options = Options::parse()?;
    let mut display = None;
    if options.rom_path.is_none()
        && let Some(path) = &options.dump_path
    {
        let settings = settings_path()
            .map(|path| settings::Settings::load(&path))
            .unwrap_or_default();
        let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
        front::Front::new(&settings).draw(&mut frame);
        return write_ppm(path, &frame);
    }
    if options.rom_path.is_none() {
        let mut shown = Sdl3Display::open(
            front::PROJECT_NAME,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
            WINDOW_SCALE,
        )?;
        let Some(choice) = front::run(&mut shown, settings_path().as_deref())? else {
            return Ok(());
        };
        options.rom_path = Some(choice.rom);
        options.translation = choice.translation.or(options.translation);
        display = Some(shown);
    }
    let rom_path = options.rom_path.clone().context(USAGE)?;
    let rom = std::fs::read(&rom_path)
        .with_context(|| format!("cannot read ROM {}", rom_path.display()))?;
    let identification = extraction::identify(&rom)
        .with_context(|| format!("cannot identify ROM {}", rom_path.display()))?;
    print_identification(&identification);
    let title = identification.title.to_string();
    if let Some(string_id) = &options.string_id {
        let frame = render_string(&rom, identification.title, string_id)?;
        return match &options.dump_path {
            Some(path) => write_ppm(path, &frame),
            None => show(&title, &frame),
        };
    }
    if identification.title != Title::Saga {
        bail!("the field is only implemented for {}", Title::Saga);
    }
    if let Some((path, scopes)) = &options.template {
        return export_template(&rom, path, scopes);
    }
    let mut game = if options.room || options.dump_path.is_some() {
        Game::in_first_room(&rom)?
    } else {
        Game::new(&rom)?
    };
    let save_path = options
        .save_path
        .clone()
        .unwrap_or_else(|| rom_path.with_extension(SAVE_EXTENSION));
    let slots: Vec<Box<dyn platform::SaveStorage>> = (0..options.slots)
        .map(|slot| {
            let path = slot_path(&save_path, slot);
            println!("Save:       slot {} in {}", slot + 1, path.display());
            Box::new(FileStorage::new(path)) as Box<dyn platform::SaveStorage>
        })
        .collect();
    game.set_save_slots(slots);
    game.extensions()
        .borrow_mut()
        .insert(Box::new(StorageReport));
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
    match &options.dump_path {
        Some(path) => {
            let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
            game.draw(&mut frame);
            write_ppm(path, &frame)
        }
        None => play(display, &mut game),
    }
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

struct Options {
    rom_path: Option<PathBuf>,
    string_id: Option<String>,
    dump_path: Option<PathBuf>,
    save_path: Option<PathBuf>,
    slots: usize,
    room: bool,
    translation: Option<PathBuf>,
    template: Option<(PathBuf, Vec<String>)>,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut args = std::env::args_os().skip(1).peekable();
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
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--dump") => dump_path = Some(args.next().map(PathBuf::from).context(USAGE)?),
                Some("--room") => room = true,
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
        None => println!("Dump:       not a validated dump; behavior may differ"),
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
    let mut display = Sdl3Display::open(title, frame.width(), frame.height(), WINDOW_SCALE)?;
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

/// Runs the game in `display`, the launcher's window, or a new one.
fn play(display: Option<Sdl3Display>, game: &mut Game<'_>) -> Result<()> {
    let mut display = if let Some(display) = display {
        display
    } else {
        let mut display = Sdl3Display::open(
            front::PROJECT_NAME,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
            WINDOW_SCALE,
        )?;
        if let Some(path) = settings_path() {
            display.set_keys(&settings::Settings::load(&path).keys);
        }
        display
    };
    let mut audio = match display.open_audio(SAMPLE_RATE) {
        Ok(audio) => Some(audio),
        Err(error) => {
            eprintln!("no audio: {error}");
            None
        }
    };
    let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
    loop {
        let started = std::time::Instant::now();
        for event in display.poll_events() {
            match event {
                Event::Quit | Event::Back => return Ok(()),
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
                Event::FunctionKey(_) | Event::Key(_) => {}
            }
        }
        game.update(display.input())?;
        if let Some(audio) = &mut audio
            && audio.queued_pairs() < SAMPLES_PER_FRAME * AUDIO_QUEUE_FRAMES
        {
            audio.queue(game.audio())?;
        }
        game.draw(&mut frame);
        display.present(&frame)?;
        std::thread::sleep(FRAME_DURATION.saturating_sub(started.elapsed()));
    }
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
