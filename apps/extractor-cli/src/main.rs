//! Development CLI over the extraction library: inspect and dump extracted data.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use extraction::{StringTable, Title};

const USAGE: &str = "usage:\n  extractor-cli identify <rom-path>\n  extractor-cli dump-text <rom-path> [table]\n  extractor-cli check-layout <rom-path> [table]";

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let command = args.next().context(USAGE)?;
    let rom_path = args.next().map(PathBuf::from).context(USAGE)?;
    match command.to_str() {
        Some("identify") => identify(&rom_path),
        Some("dump-text") => {
            let table = args.next().and_then(|s| s.into_string().ok());
            dump_text(&rom_path, table.as_deref())
        }
        Some("check-layout") => {
            let table = args.next().and_then(|s| s.into_string().ok());
            check_layout(&rom_path, table.as_deref())
        }
        _ => bail!(USAGE),
    }
}

fn read_rom(rom_path: &Path) -> Result<Vec<u8>> {
    std::fs::read(rom_path).with_context(|| format!("cannot read ROM {}", rom_path.display()))
}

fn identify(rom_path: &Path) -> Result<()> {
    let rom = read_rom(rom_path)?;
    let identification = extraction::identify(&rom)?;
    println!("{}", identification.title);
    println!("game_code={}", identification.header.game_code);
    println!("version={}", identification.header.version);
    println!("sha1={}", identification.sha1);
    println!("verified={}", identification.is_verified());
    Ok(())
}

fn dump_text(rom_path: &Path, table_name: Option<&str>) -> Result<()> {
    let rom = read_rom(rom_path)?;
    let identification = extraction::identify(&rom)?;
    let tables = string_tables(identification.title, table_name)?;
    for table in tables {
        for string in table.read(&rom)? {
            println!("## {} @{:#x}", string.id, string.offset);
            println!("{}", string.script.plain_text());
            println!();
        }
    }
    Ok(())
}

fn check_layout(rom_path: &Path, table_name: Option<&str>) -> Result<()> {
    let rom = read_rom(rom_path)?;
    let identification = extraction::identify(&rom)?;
    let area = game_core::DIALOGUE_TEXT_AREA;
    let mut messages = 0;
    let mut overflowing = 0;
    for table in string_tables(identification.title, table_name)? {
        for string in table.read(&rom)? {
            for (index, text) in string.script.message_texts().iter().enumerate() {
                messages += 1;
                let body = text.split_once('\n').map_or("", |(_, body)| body);
                let layout = area.layout(body, localization::monospace);
                if layout.overflows() {
                    overflowing += 1;
                    println!(
                        "{}#{index}: {} lines: {:?}",
                        string.id,
                        layout.lines.len(),
                        layout.lines
                    );
                }
            }
        }
    }
    println!(
        "{messages} messages, {overflowing} need more than {} lines of {} cells",
        area.rows, area.columns
    );
    Ok(())
}

fn string_tables(title: Title, table_name: Option<&str>) -> Result<Vec<StringTable>> {
    let all = match title {
        Title::Saga => extraction::saga::STRING_TABLES,
        Title::Legacy | Title::Fuzors => bail!("no string tables are known for {title} yet"),
    };
    match table_name {
        None => Ok(all.to_vec()),
        Some(name) => all
            .iter()
            .find(|table| table.name == name)
            .copied()
            .map(|table| vec![table])
            .with_context(|| {
                let names: Vec<&str> = all.iter().map(|table| table.name).collect();
                format!("unknown table {name:?}; known: {}", names.join(", "))
            }),
    }
}
