//! Textures: images that cover a surface, as a game supplies them (like
//! Bevy's `Image` or Godot's `ImageTexture`). The engine stores them
//! the way GPUs expect colour textures: 8-bit sRGB.

use crate::render::Color;

/// An image: `width` × `height` pixels, row by row from the top-left, 4
/// bytes each (red, green, blue: sRGB-encoded, as colour textures are
/// stored; then alpha).
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub srgba: Vec<u8>,
}

impl Texture {
    /// A `width` × `height` image whose pixel (x, y) (from the top-left)
    /// has the linear colour `pixel(x, y)`.
    pub fn from_fn(width: u32, height: u32, pixel: impl Fn(u32, u32) -> Color) -> Self {
        let mut srgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let color = pixel(x, y);
                srgba.extend([color.r, color.g, color.b].map(srgb_byte));
                srgba.push((color.a.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
        Self {
            width,
            height,
            srgba,
        }
    }
}

/// A linear colour channel (0 to 1) as an sRGB byte: the standard sRGB
/// curve, which gives dark values more of the 256 steps (eyes are more
/// sensitive there).
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

    #[test]
    fn pixels_are_stored_row_by_row_as_srgb_bytes() {
        // 2 × 2: white top-left, half-transparent black bottom-right.
        let texture = Texture::from_fn(2, 2, |x, y| match (x, y) {
            (0, 0) => Color::rgb(1.0, 1.0, 1.0),
            (1, 1) => Color {
                a: 0.5,
                ..Color::rgb(0.0, 0.0, 0.0)
            },
            _ => Color::rgb(1.0, 0.0, 0.0),
        });
        assert_eq!((texture.width, texture.height), (2, 2));
        assert_eq!(
            texture.srgba,
            [
                255, 255, 255, 255, /* */ 255, 0, 0, 255, // top row
                255, 0, 0, 255, /*     */ 0, 0, 0, 128, // bottom row
            ]
        );
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
