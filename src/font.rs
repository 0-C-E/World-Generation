//! Minimal 5x7 bitmap font for debug overlays.
//!
//! Renders printable ASCII characters into an RGB pixel buffer, with each
//! character occupying a 5-wide by 7-tall cell. This module is intentionally
//! standalone so it can be reused anywhere a quick text overlay is needed
//! (debug tiles, minimaps, HUD sprites, etc.).

/// Width of a single glyph in pixels.
pub const GLYPH_W: u32 = 5;

/// Height of a single glyph in pixels.
pub const GLYPH_H: u32 = 7;

/// Horizontal advance (glyph width + 1px spacing).
pub const ADVANCE: u32 = GLYPH_W + 1;

/// Draw a string of ASCII text into an RGB pixel buffer.
///
/// * `pixels` - flat RGB buffer (3 bytes per pixel, row-major).
/// * `stride` - number of pixels per row (e.g. 256 for a 256-wide image).
/// * `x0, y0` - top-left corner of the first character.
/// * `text` - ASCII string to render.
/// * `color` - RGB color for the text.
pub fn draw_text(pixels: &mut [u8], stride: u32, x0: u32, y0: u32, text: &str, color: [u8; 3]) {
    let mut cx = x0;
    for ch in text.chars() {
        let glyph = glyph_for(ch);
        for row in 0..GLYPH_H {
            for col in 0..GLYPH_W {
                if glyph[row as usize] & (1 << (4 - col)) != 0 {
                    set_pixel(pixels, stride, cx + col, y0 + row, color);
                }
            }
        }
        cx += ADVANCE;
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Write a single RGB pixel, bounds-checked.
fn set_pixel(pixels: &mut [u8], stride: u32, x: u32, y: u32, color: [u8; 3]) {
    let off = ((y * stride + x) * 3) as usize;
    if off + 2 < pixels.len() {
        pixels[off] = color[0];
        pixels[off + 1] = color[1];
        pixels[off + 2] = color[2];
    }
}

/// 5x7 glyph bitmap for a printable ASCII character.
///
/// Each of the 7 bytes represents one row; the top 5 bits encode the pixels.
fn glyph_for(ch: char) -> [u8; 7] {
    match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'a'..='z' => {
            let i = (ch as u8 - b'a') as usize;
            const AZ: [[u8; 7]; 26] = [
                [
                    0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
                ], // a
                [
                    0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
                ], // b
                [
                    0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
                ], // c
                [
                    0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
                ], // d
                [
                    0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
                ], // e
                [
                    0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
                ], // f
                [
                    0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
                ], // g
                [
                    0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
                ], // h
                [
                    0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
                ], // i
                [
                    0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
                ], // j
                [
                    0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
                ], // k
                [
                    0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
                ], // l
                [
                    0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
                ], // m
                [
                    0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
                ], // n
                [
                    0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
                ], // o
                [
                    0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
                ], // p
                [
                    0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
                ], // q
                [
                    0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
                ], // r
                [
                    0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110,
                ], // s
                [
                    0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
                ], // t
                [
                    0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
                ], // u
                [
                    0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100,
                ], // v
                [
                    0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
                ], // w
                [
                    0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
                ], // x
                [
                    0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
                ], // y
                [
                    0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
                ], // z
            ];
            AZ[i]
        }
        'A'..='Z' => glyph_for((ch as u8 - b'A' + b'a') as char),
        '=' => [
            0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ' ' => [0; 7],
        _ => [
            0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
        ],
    }
}
