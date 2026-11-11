//! How a ground looks: a big flat square (a mesh) covered with a grid (a
//! texture), both built once when the scene loads.
//!
//! As in engines, the ground is drawn like any other object: a retained
//! mesh with a material, moved to its body's position (Unity's and
//! Godot's "prototype grid" materials are this: a grid texture tiled over
//! a plane). The grid lives in the texture, so the GPU draws it at no cost
//! per frame, and *mipmaps* (smaller pre-filtered copies of the texture,
//! used far away) keep distant lines from shimmering.

use glam::{Vec2, Vec3};

use crate::render::{Color, GroundGrid, Mesh, Texture, VIEW_DISTANCE};

/// Texture pixels across one grid cell: enough for lines a few pixels
/// wide to stay sharp up close.
const PIXELS_PER_CELL: u32 = 32;

/// Line widths, in texture pixels (with 2 m cells, a pixel is about 6 cm).
const MINOR_LINE_PIXELS: f32 = 2.0;
const MAJOR_LINE_PIXELS: f32 = 4.0;

/// The visible part of an endless ground, in render axes (x east, y up, z
/// south): a horizontal square reaching `VIEW_DISTANCE` from its centre
/// to each side, so wherever it's centred (under the camera), it covers
/// everything in view.
///
/// Its texture coordinates count *tiles*, one major cell each: the texture
/// (one tile, see [`grid_texture`]) repeats across the square, so the
/// texture stays small however big the ground is.
pub(crate) fn ground_mesh(grid: &GroundGrid) -> Mesh {
    let h = VIEW_DISTANCE;
    // North-west, south-west, south-east, north-east (z is south).
    let corners = [
        Vec3::new(-h, 0.0, -h),
        Vec3::new(-h, 0.0, h),
        Vec3::new(h, 0.0, h),
        Vec3::new(h, 0.0, -h),
    ];
    let tile = grid.major_spacing() as f32;
    Mesh {
        positions: corners.to_vec(),
        normals: vec![Vec3::Y; corners.len()],
        uvs: corners.iter().map(|c| Vec2::new(c.x, c.z) / tile).collect(),
        // Counter-clockwise seen from above: the side GPUs draw as the
        // front, and light as facing up.
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    }
}

/// What a texture pixel shows, from least to most visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Line {
    None,
    Minor,
    Major,
}

/// One tile of the grid: `major_every` cells each way, `ground` between
/// the lines. Major lines run along the tile's edges, centred on them, so
/// tiles join seamlessly: half a line on each side.
pub(crate) fn grid_texture(grid: &GroundGrid, ground: Color) -> Texture {
    let size = PIXELS_PER_CELL * grid.major_every;
    let mut srgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let color = match line_at(x, size).max(line_at(y, size)) {
                Line::Major => grid.major_color,
                Line::Minor => grid.minor_color,
                Line::None => ground,
            };
            srgba.extend([color.r, color.g, color.b].map(srgb_byte));
            srgba.push((color.a.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    Texture {
        width: size,
        height: size,
        srgba,
    }
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

/// A linear colour channel (0 to 1) as an sRGB byte, what colour textures
/// store: the standard sRGB curve, which gives dark values more of the 256
/// steps (eyes are more sensitive there).
fn srgb_byte(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUND: Color = Color::rgb(0.02, 0.03, 0.02);

    /// A line every 2 m, a major one every 10 m.
    fn grid() -> GroundGrid {
        GroundGrid::default()
    }

    /// The colour of texture pixel (x, y), as sRGB bytes.
    fn pixel(texture: &Texture, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * texture.width + x) * 4) as usize;
        [0, 1, 2, 3].map(|c| texture.srgba[i + c])
    }

    fn srgb(color: Color) -> [u8; 4] {
        [
            srgb_byte(color.r),
            srgb_byte(color.g),
            srgb_byte(color.b),
            255,
        ]
    }

    #[test]
    fn the_mesh_is_a_flat_square_reaching_the_view_distance() {
        let mesh = ground_mesh(&grid());
        assert!(mesh.positions.iter().all(|p| p.y == 0.0));
        let (min, max) = mesh.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let d = VIEW_DISTANCE;
        assert_eq!((min.x, max.x, min.z, max.z), (-d, d, -d, d));
        assert!(mesh.normals.iter().all(|n| *n == Vec3::Y));
    }

    #[test]
    fn both_triangles_face_up() {
        // Counter-clockwise seen from above: the cross product of two
        // edges points up (+y), so GPUs draw them as front faces.
        let mesh = ground_mesh(&grid());
        for [a, b, c] in mesh.triangles {
            let p = |i: u32| mesh.positions[i as usize];
            let normal = (p(b) - p(a)).cross(p(c) - p(a));
            assert!(normal.y > 0.0, "{normal}");
        }
    }

    #[test]
    fn texture_coordinates_count_tiles_of_one_major_cell() {
        // 5 km to each side, tiles of 10 m: from -500 to 500 tiles.
        let mesh = ground_mesh(&grid());
        assert!(mesh.uvs.iter().all(|uv| uv.abs() == Vec2::splat(500.0)));
    }

    #[test]
    fn the_texture_is_one_major_cell_square() {
        let texture = grid_texture(&grid(), GROUND);
        assert_eq!((texture.width, texture.height), (5 * 32, 5 * 32));
        assert_eq!(texture.srgba.len(), 160 * 160 * 4);
    }

    #[test]
    fn the_texture_shows_ground_between_minor_and_major_lines() {
        let grid = grid();
        let texture = grid_texture(&grid, GROUND);
        assert_eq!(pixel(&texture, 16, 16), srgb(GROUND)); // a cell's middle
        assert_eq!(pixel(&texture, 32, 16), srgb(grid.minor_color)); // between cells
        assert_eq!(pixel(&texture, 16, 0), srgb(grid.major_color)); // tile edge
        // Where lines cross, the more visible one wins.
        assert_eq!(pixel(&texture, 32, 0), srgb(grid.major_color));
    }

    #[test]
    fn major_lines_straddle_the_tile_edges_so_tiles_join_seamlessly() {
        let texture = grid_texture(&grid(), GROUND);
        let major = srgb(grid().major_color);
        // Two pixels on each side of the edge, which wraps round.
        for x in [0, 1, 158, 159] {
            assert_eq!(pixel(&texture, x, 16), major, "column {x}");
        }
        assert_ne!(pixel(&texture, 2, 16), major);
    }

    #[test]
    fn colours_are_stored_on_the_srgb_curve() {
        assert_eq!(srgb_byte(0.0), 0);
        assert_eq!(srgb_byte(1.0), 255);
        // Mid-grey light is much brighter than half on screen.
        assert_eq!(srgb_byte(0.5), 188);
        // The linear segment near black: 12.92 × 0.001 × 255 = 3.3.
        assert_eq!(srgb_byte(0.001), 3);
        // Out of range values are clamped.
        assert_eq!(srgb_byte(2.0), 255);
    }
}
