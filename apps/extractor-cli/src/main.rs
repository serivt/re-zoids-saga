//! Development CLI over the extraction library: inspect and dump extracted data.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let command = args.next().context(USAGE)?;
    match command.to_str() {
        Some("identify") => identify(&args.next().map(PathBuf::from).context(USAGE)?),
        _ => bail!(USAGE),
    }
}

const USAGE: &str = "usage: extractor-cli identify <rom-path>";

fn identify(rom_path: &Path) -> Result<()> {
    let rom = std::fs::read(rom_path)
        .with_context(|| format!("cannot read ROM {}", rom_path.display()))?;
    let identification = extraction::identify(&rom)?;
    println!("{}", identification.title);
    println!("game_code={}", identification.header.game_code);
    println!("version={}", identification.header.version);
    println!("sha1={}", identification.sha1);
    println!("verified={}", identification.is_verified());
    Ok(())
}
