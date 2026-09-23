//! Drawing field scenes: a scrolling map over a repeating backdrop.

use extraction::saga::Scene;
use gba_runtime::ppu::{PaletteBank, draw_background};
use platform::Frame;

/// Draws `scene` with the map's top-left visible cell at `scroll`.
pub fn draw_scene(frame: &mut Frame, scene: &Scene, scroll: (usize, usize)) {
    let bank = PaletteBank::from_bgr555(&scene.palettes);
    let tile = |index: usize| scene.tiles.tile(index);
    draw_background(
        frame,
        |x, y| scene.backdrop.wrapping(x, y),
        tile,
        &bank,
        scroll,
        false,
    );
    draw_background(
        frame,
        |x, y| scene.map.wrapping(x, y),
        tile,
        &bank,
        scroll,
        true,
    );
}
