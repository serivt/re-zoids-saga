//! The launcher's executable: [`launcher::run`] with the command line.
//!
//! Built with the `packaged` feature, as players download it, SDL3 is
//! linked in and, on Windows, no console window opens behind the game's.

#![cfg_attr(all(windows, feature = "packaged"), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    launcher::run(std::env::args_os().skip(1))
}
