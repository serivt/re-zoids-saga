//! Launcher: ROM picker, title detection, transparent extraction and game start.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use extraction::{Identification, Title};
use game_core::{DIALOGUE_TEXT_AREA, Field, TextPainter, WindowPainter, draw_scene, draw_sprite};
use gba_runtime::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use localization::monospace;
use platform::{Display, Event, Frame, Rgb};
use platform_sdl3::Sdl3Display;

const USAGE: &str = "usage: launcher <rom-path> [string-id] [--dump <frame.ppm>]\n  without a string id the launcher lets you walk the first room (arrows move, Esc quits)";
const WINDOW_SCALE: u32 = 3;
const FIRST_ROOM_SCENE: usize = 3;
const FIRST_ROOM_MAP: usize = 4;
const PLAYER_X: i32 = 72;
const PLAYER_Y: i32 = 64;
const PLAYER_FRAME: usize = 3;
const PLAYER_START: (usize, usize) = (5, 2);
const FRAME_DURATION: std::time::Duration = std::time::Duration::from_micros(16_743);
const BOX_ROW: usize = 12;
const BOX_ROWS: usize = 8;
const BOX_COLUMNS: usize = 30;
const PORTRAIT_DIVIDER_COLUMN: usize = 7;
const PORTRAIT_X: i32 = 8;
const PORTRAIT_Y: i32 = 104;
const SPEAKER_X: i32 = 64;
const SPEAKER_Y: i32 = 104;
const TEXT_Y: i32 = 120;

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
    let mut field = Field::load(&rom, FIRST_ROOM_MAP, PLAYER_START)?;
    match &options.dump_path {
        Some(path) => {
            let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
            field.draw(&mut frame);
            write_ppm(path, &frame)
        }
        None => walk(&title, &rom, &mut field),
    }
}

struct Options {
    rom_path: PathBuf,
    string_id: Option<String>,
    dump_path: Option<PathBuf>,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut args = std::env::args_os().skip(1);
        let rom_path = args.next().map(PathBuf::from).context(USAGE)?;
        let mut string_id = None;
        let mut dump_path = None;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--dump") => dump_path = Some(args.next().map(PathBuf::from).context(USAGE)?),
                Some(id) if !id.starts_with("--") => string_id = Some(id.to_owned()),
                _ => bail!(USAGE),
            }
        }
        Ok(Self {
            rom_path,
            string_id,
            dump_path,
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
    let string = strings
        .get(index)
        .with_context(|| format!("{table_name} has {} strings", strings.len()))?;
    let (glyphs, fallback) = extraction::saga::font(rom)?;
    let painter = TextPainter::new(rom, glyphs, Some(fallback));
    let skin = extraction::saga::window_skin(rom)?;
    let window = WindowPainter::new(skin.tiles, &skin.palette);

    let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::new(16, 24, 48));
    let scene = extraction::saga::scene(rom, FIRST_ROOM_SCENE)?;
    draw_scene(&mut frame, &scene, (0, 0));
    let player = extraction::saga::sprite_sheet(rom, extraction::saga::PLAYER_SPRITE)?;
    if let Some(image) = player.image(PLAYER_FRAME) {
        draw_sprite(&mut frame, PLAYER_X, PLAYER_Y, &image, &player.palette);
    }
    window.draw_window(&mut frame, 0, BOX_ROW, BOX_COLUMNS, BOX_ROWS);
    window.draw_divider(&mut frame, PORTRAIT_DIVIDER_COLUMN, BOX_ROW, BOX_ROWS);
    if let Some((character, expression)) = string.script.first_speaker() {
        let portrait =
            extraction::saga::portrait(rom, usize::from(character), usize::from(expression))?;
        draw_sprite(
            &mut frame,
            PORTRAIT_X,
            PORTRAIT_Y,
            &portrait.image,
            &portrait.palette,
        );
    }
    let messages = string.script.message_texts();
    let text = messages.first().map(String::as_str).unwrap_or_default();
    let (speaker, body) = text.split_once('\n').unwrap_or((text, ""));
    painter.draw(&mut frame, SPEAKER_X, SPEAKER_Y, speaker, window.palette());
    let layout = DIALOGUE_TEXT_AREA.layout(body, monospace);
    let visible = layout.visible_lines().join("\n");
    painter.draw(&mut frame, SPEAKER_X, TEXT_Y, &visible, window.palette());
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

fn walk(title: &str, rom: &[u8], field: &mut Field) -> Result<()> {
    let mut display = Sdl3Display::open(title, SCREEN_WIDTH, SCREEN_HEIGHT, WINDOW_SCALE)?;
    let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::default());
    loop {
        let started = std::time::Instant::now();
        if display.poll_events().contains(&Event::Quit) {
            return Ok(());
        }
        if let Some(exit) = field.update(display.input()) {
            let warp = field.warp(rom, exit)?;
            println!(
                "Exit {exit} -> map {} at ({}, {})",
                warp.map, warp.column, warp.row
            );
        }
        field.draw(&mut frame);
        display.present(&frame)?;
        std::thread::sleep(FRAME_DURATION.saturating_sub(started.elapsed()));
    }
}

fn write_ppm(path: &Path, frame: &Frame) -> Result<()> {
    let mut bytes = format!("P6\n{} {}\n255\n", frame.width(), frame.height()).into_bytes();
    bytes.extend(frame.to_rgb24());
    std::fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
