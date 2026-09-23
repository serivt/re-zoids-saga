//! Launcher: ROM picker, title detection, transparent extraction and game start.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use extraction::{Identification, Title};
use game_core::{
    DEFAULT_PLAYER_NAME, Field, Game, ScriptRunner, ScriptWindows, TextPainter, WindowPainter,
};
use gba_runtime::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use platform::{Display, Event, Frame, Rgb};
use platform_sdl3::Sdl3Display;

const USAGE: &str = "usage: launcher <rom-path> [string-id] [--room] [--dump <frame.ppm>]\n  without a string id the launcher boots the game (arrows move, X = A, Z = B, Return = Start, Backspace = Select, Esc quits); --room skips to the first room";
const WINDOW_SCALE: u32 = 3;
const FIRST_ROOM_MAP: usize = extraction::saga::FIRST_ROOM_MAP;
const PLAYER_START: (usize, usize) = extraction::saga::PLAYER_START;
const FRAME_DURATION: std::time::Duration = std::time::Duration::from_micros(16_743);
const RENDER_FRAME_LIMIT: usize = 600;

fn main() -> Result<()> {
    let options = Options::parse()?;
    let rom = std::fs::read(&options.rom_path)
        .with_context(|| format!("cannot read ROM {}", options.rom_path.display()))?;
    let identification = extraction::identify(&rom)
        .with_context(|| format!("cannot identify ROM {}", options.rom_path.display()))?;
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
    let mut game = if options.room || options.dump_path.is_some() {
        Game::in_first_room(&rom)?
    } else {
        Game::new(&rom)?
    };
    match &options.dump_path {
        Some(path) => {
            let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
            game.draw(&mut frame);
            write_ppm(path, &frame)
        }
        None => play(&title, &mut game),
    }
}

struct Options {
    rom_path: PathBuf,
    string_id: Option<String>,
    dump_path: Option<PathBuf>,
    room: bool,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut args = std::env::args_os().skip(1);
        let rom_path = args.next().map(PathBuf::from).context(USAGE)?;
        let mut string_id = None;
        let mut dump_path = None;
        let mut room = false;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--dump") => dump_path = Some(args.next().map(PathBuf::from).context(USAGE)?),
                Some("--room") => room = true,
                Some(id) if !id.starts_with("--") => string_id = Some(id.to_owned()),
                _ => bail!(USAGE),
            }
        }
        Ok(Self {
            rom_path,
            string_id,
            dump_path,
            room,
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
    Field::load(rom, FIRST_ROOM_MAP, PLAYER_START)?.draw(&mut frame);
    windows.draw(&mut frame, &window, &painter);
    Ok(frame)
}

fn show(title: &str, frame: &Frame) -> Result<()> {
    let mut display = Sdl3Display::open(title, frame.width(), frame.height(), WINDOW_SCALE)?;
    loop {
        if display.poll_events().contains(&Event::Quit) {
            return Ok(());
        }
        display.present(frame)?;
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

fn play(title: &str, game: &mut Game<'_>) -> Result<()> {
    let mut display = Sdl3Display::open(title, SCREEN_WIDTH, SCREEN_HEIGHT, WINDOW_SCALE)?;
    let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
    loop {
        let started = std::time::Instant::now();
        if display.poll_events().contains(&Event::Quit) {
            return Ok(());
        }
        game.update(display.input())?;
        game.draw(&mut frame);
        display.present(&frame)?;
        std::thread::sleep(FRAME_DURATION.saturating_sub(started.elapsed()));
    }
}

fn write_ppm(path: &Path, frame: &Frame) -> Result<()> {
    let mut bytes = format!("P6\n{} {}\n255\n", frame.width(), frame.height()).into_bytes();
    bytes.extend(frame.to_rgb24());
    std::fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
