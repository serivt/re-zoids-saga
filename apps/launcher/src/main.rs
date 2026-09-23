//! Launcher: ROM picker, title detection, transparent extraction and game start.

use std::path::PathBuf;

use anyhow::{Context, Result};
use extraction::Identification;

fn main() -> Result<()> {
    let rom_path = rom_path_from_args()?;
    let rom = std::fs::read(&rom_path)
        .with_context(|| format!("cannot read ROM {}", rom_path.display()))?;
    let identification = extraction::identify(&rom)
        .with_context(|| format!("cannot identify ROM {}", rom_path.display()))?;
    print_identification(&identification);
    Ok(())
}

fn rom_path_from_args() -> Result<PathBuf> {
    std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .context("usage: launcher <rom-path>")
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
