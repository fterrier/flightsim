//! The game's textures, drawn in code. (Engines' "prototype grid"
//! textures are image files shipped with a game; ours is simple enough to
//! draw pixel by pixel.)
// TODO(step-09): textures as image files, loaded with the scenario.

use engine::{Color, Texture};

/// Texture pixels across one grid cell: enough for lines a few pixels
/// wide to stay sharp up close.
const PIXELS_PER_CELL: u32 = 32;

/// Line widths, in texture pixels (with 2 m cells, a pixel is about 6 cm).
const MINOR_LINE_PIXELS: f32 = 2.0;
const MAJOR_LINE_PIXELS: f32 = 4.0;

/// The colours of a grid tile.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GridColors {
    /// Between the lines.
    pub(crate) ground: Color,
    pub(crate) minor: Color,
    pub(crate) major: Color,
}

/// One tile of a ground grid, meant to be repeated: `cells` × `cells`
/// cells, with a minor line between cells and a major line along the
/// tile's edges. Major lines are centred on the edges, half on each side,
/// so repeated tiles join seamlessly.
pub(crate) fn grid_tile(cells: u32, colors: GridColors) -> Texture {
    let size = PIXELS_PER_CELL * cells;
    Texture::from_fn(size, size, |x, y| {
        match line_at(x, size).max(line_at(y, size)) {
            Line::Major => colors.major,
            Line::Minor => colors.minor,
            Line::None => colors.ground,
        }
    })
}

/// What a pixel column (or row) shows, from least to most visible: where
/// lines cross, the more visible one wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Line {
    None,
    Minor,
    Major,
}

/// Whether pixel column (or row) `pixel` of a tile `size` pixels wide is
/// on a line, judging by the distance from its centre to the nearest one.
fn line_at(pixel: u32, size: u32) -> Line {
    let centre = pixel as f32 + 0.5;
    let to_tile_edge = centre.min(size as f32 - centre);
    let cell = PIXELS_PER_CELL as f32;
    let in_cell = centre % cell;
    let to_cell_edge = in_cell.min(cell - in_cell);
    if to_tile_edge < MAJOR_LINE_PIXELS / 2.0 {
        Line::Major
    } else if to_cell_edge < MINOR_LINE_PIXELS / 2.0 {
        Line::Minor
    } else {
        Line::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLORS: GridColors = GridColors {
        ground: Color::rgb(0.0, 0.0, 0.0),
        minor: Color::rgb(0.5, 0.0, 0.0),
        major: Color::rgb(1.0, 1.0, 1.0),
    };

    /// Pixel (x, y) of `tile`, as it would be drawn from `colors` alone.
    fn is(tile: &Texture, x: u32, y: u32, color: Color) -> bool {
        let one = Texture::from_fn(1, 1, |_, _| color);
        let i = ((y * tile.width + x) * 4) as usize;
        tile.srgba[i..i + 4] == one.srgba[..]
    }

    #[test]
    fn a_tile_is_its_cells_square() {
        let tile = grid_tile(5, COLORS);
        assert_eq!((tile.width, tile.height), (5 * 32, 5 * 32));
    }

    #[test]
    fn ground_shows_between_minor_and_major_lines() {
        let tile = grid_tile(5, COLORS);
        assert!(is(&tile, 16, 16, COLORS.ground)); // a cell's middle
        assert!(is(&tile, 32, 16, COLORS.minor)); // between two cells
        assert!(is(&tile, 16, 0, COLORS.major)); // the tile's edge
        assert!(is(&tile, 32, 0, COLORS.major)); // crossing: major wins
    }

    #[test]
    fn major_lines_straddle_the_tile_edges_so_tiles_join_seamlessly() {
        let tile = grid_tile(5, COLORS);
        // Two pixels on each side of the edge, which wraps round.
        for x in [0, 1, 158, 159] {
            assert!(is(&tile, x, 16, COLORS.major), "column {x}");
        }
        assert!(!is(&tile, 2, 16, COLORS.major));
    }
}
