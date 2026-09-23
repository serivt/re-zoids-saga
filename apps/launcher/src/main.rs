//! Launcher: ROM picker, title detection, transparent extraction and game start.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use extraction::{Identification, Title};
use game_core::TextPainter;
use gba_runtime::ppu::{Palette, SCREEN_HEIGHT, SCREEN_WIDTH};
use platform::{Display, Event, Frame, Rgb};
use platform_sdl3::Sdl3Display;

const USAGE: &str = "usage: launcher <rom-path> [string-id] [--dump <frame.ppm>]";
const DEFAULT_STRING_ID: &str = "dialogue_00003";
const WINDOW_SCALE: u32 = 3;
const TEXT_MARGIN: i32 = 8;

fn main() -> Result<()> {
    let options = Options::parse()?;
    let rom = std::fs::read(&options.rom_path)
        .with_context(|| format!("cannot read ROM {}", options.rom_path.display()))?;
    let identification = extraction::identify(&rom)
        .with_context(|| format!("cannot identify ROM {}", options.rom_path.display()))?;
    print_identification(&identification);
    let frame = render_string(&rom, identification.title, &options.string_id)?;
    if let Some(path) = &options.dump_path {
        write_ppm(path, &frame)?;
        return Ok(());
    }
    show(&identification.title.to_string(), &frame)
}

struct Options {
    rom_path: PathBuf,
    string_id: String,
    dump_path: Option<PathBuf>,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut args = std::env::args_os().skip(1);
        let rom_path = args.next().map(PathBuf::from).context(USAGE)?;
        let mut string_id = DEFAULT_STRING_ID.to_owned();
        let mut dump_path = None;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--dump") => dump_path = Some(args.next().map(PathBuf::from).context(USAGE)?),
                Some(id) if !id.starts_with("--") => id.clone_into(&mut string_id),
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

    let mut frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, Rgb::new(16, 24, 48));
    painter.draw(
        &mut frame,
        TEXT_MARGIN,
        TEXT_MARGIN,
        &string.script.plain_text(),
        &text_palette(),
    );
    Ok(frame)
}

fn text_palette() -> Palette {
    let mut colors = [Rgb::new(16, 24, 48); 16];
    colors[2] = Rgb::new(72, 80, 104);
    colors[3] = Rgb::new(128, 136, 160);
    colors[4] = Rgb::new(184, 192, 208);
    colors[15] = Rgb::new(248, 248, 248);
    Palette::new(colors)
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

fn write_ppm(path: &Path, frame: &Frame) -> Result<()> {
    let mut bytes = format!("P6\n{} {}\n255\n", frame.width(), frame.height()).into_bytes();
    bytes.extend(frame.to_rgb24());
    std::fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
