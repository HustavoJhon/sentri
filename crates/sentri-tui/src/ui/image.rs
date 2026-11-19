//! Image rendering with plain characters, so it works over SSH and in tmux.
//!
//! - Half-block: each cell shows `▀` with the upper pixel as foreground and
//!   the lower pixel as background (two pixels per cell).
//! - ASCII: a grayscale ramp, for terminals with few colors.
//!
//! The image is resampled from the full-resolution source to exactly the
//! cell grid (one source pixel per half cell), so no pixel is skipped or
//! repeated the way nearest-neighbor sampling of a small thumbnail does.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use sentri_app::{ImageMode, Thumbnail};

const RAMP: &[u8] = b" .:-=+*#%@";

/// Draws an image into the area, centered and keeping its aspect ratio.
#[derive(Debug)]
pub struct ImageView<'a> {
    pub thumb: &'a Thumbnail,
    pub mode: ImageMode,
    /// Map colors to the xterm-256 palette instead of truecolor.
    pub indexed: bool,
}

/// Nearest xterm-256 color (6×6×6 cube or gray ramp).
pub fn to_indexed([r, g, b]: [u8; 3]) -> Color {
    let gray = r.abs_diff(g) < 10 && g.abs_diff(b) < 10;
    if gray {
        let level = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
        if level < 8 {
            return Color::Indexed(16);
        }
        if level > 238 {
            return Color::Indexed(231);
        }
        let step = u8::try_from((level - 8) / 10).unwrap_or(23).min(23);
        return Color::Indexed(232 + step);
    }
    let q = |v: u8| u8::try_from((u16::from(v) * 5 + 127) / 255).unwrap_or(5);
    Color::Indexed(16 + 36 * q(r) + 6 * q(g) + q(b))
}

fn luminance([r, g, b]: [u8; 3]) -> u32 {
    (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000
}

/// Cells that fit `thumb` inside `area` keeping its aspect ratio, centered
/// horizontally. A cell holds two square pixels in half-block mode.
pub fn fitted(thumb: &Thumbnail, area: Rect) -> Rect {
    let (w, h) = thumb.fit_size(u32::from(area.width), u32::from(area.height) * 2);
    let width = u16::try_from(w).unwrap_or(area.width).min(area.width);
    let height = u16::try_from(h.div_ceil(2))
        .unwrap_or(area.height)
        .min(area.height);
    if width == 0 || height == 0 {
        return Rect::new(area.x, area.y, 0, 0);
    }
    Rect::new(area.x + (area.width - width) / 2, area.y, width, height)
}

/// The pixels drawn for `cells`: two rows of pixels per cell row.
pub fn cell_pixels(thumb: &Thumbnail, cells: Rect) -> Thumbnail {
    thumb.resized(u32::from(cells.width), u32::from(cells.height) * 2)
}

/// Paints already-sized pixels (`cells.width` × `2·cells.height`).
pub fn paint(pixels: &Thumbnail, cells: Rect, mode: ImageMode, indexed: bool, buf: &mut Buffer) {
    let color = |rgb: [u8; 3]| {
        if indexed {
            to_indexed(rgb)
        } else {
            Color::Rgb(rgb[0], rgb[1], rgb[2])
        }
    };
    for cy in 0..cells.height {
        for cx in 0..cells.width {
            let x = u32::from(cx);
            let top = pixels.pixel(x, u32::from(cy) * 2).unwrap_or([0; 3]);
            let bottom = pixels.pixel(x, u32::from(cy) * 2 + 1).unwrap_or(top);
            let Some(cell) = buf.cell_mut((cells.x + cx, cells.y + cy)) else {
                continue;
            };
            if mode == ImageMode::Ascii {
                let level = luminance(top).midpoint(luminance(bottom));
                let steps = u32::try_from(RAMP.len() - 1).unwrap_or(9);
                let index = usize::try_from(level * steps / 255).unwrap_or(0);
                cell.set_char(char::from(RAMP[index]));
            } else {
                cell.set_symbol("▀")
                    .set_fg(color(top))
                    .set_bg(color(bottom));
            }
        }
    }
}

impl Widget for ImageView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.mode == ImageMode::Off {
            return;
        }
        let target = fitted(self.thumb, area);
        if target.width == 0 {
            return;
        }
        paint(
            &cell_pixels(self.thumb, target),
            target,
            self.mode,
            self.indexed,
            buf,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, pixels: &[[u8; 3]]) -> Thumbnail {
        Thumbnail::from_pixels(width, height, pixels).unwrap()
    }

    #[test]
    fn halfblock_uses_top_and_bottom_pixels() {
        // 2×2: red, green / blue, white
        let thumb = image(
            2,
            2,
            &[[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]],
        );
        let area = Rect::new(0, 0, 2, 1);
        let mut buf = Buffer::empty(area);
        ImageView {
            thumb: &thumb,
            mode: ImageMode::Halfblock,
            indexed: false,
        }
        .render(area, &mut buf);
        let cell = &buf[(0, 0)];
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn ascii_mode_uses_luminance_ramp() {
        let thumb = image(1, 2, &[[255, 255, 255], [255, 255, 255]]);
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        ImageView {
            thumb: &thumb,
            mode: ImageMode::Ascii,
            indexed: false,
        }
        .render(area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "@");
    }

    #[test]
    fn large_sources_are_resampled_to_the_cell_grid() {
        // Thin stripes alternate every pixel: nearest-neighbor sampling of a
        // 4× larger image would pick only one stripe color. Resampling
        // averages them into an even gray.
        let pixels: Vec<[u8; 3]> = (0..64 * 16)
            .map(|i| if i % 2 == 0 { [0; 3] } else { [255; 3] })
            .collect();
        let thumb = image(64, 16, &pixels);
        let area = Rect::new(0, 0, 16, 2);
        let mut buf = Buffer::empty(area);
        ImageView {
            thumb: &thumb,
            mode: ImageMode::Halfblock,
            indexed: false,
        }
        .render(area, &mut buf);
        let Color::Rgb(r, _, _) = buf[(5, 0)].fg else {
            panic!("truecolor expected");
        };
        assert!((90..=165).contains(&r), "averaged, got {r}");
    }

    #[test]
    fn off_mode_and_empty_areas_draw_nothing() {
        let thumb = image(2, 2, &[[255; 3]; 4]);
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        ImageView {
            thumb: &thumb,
            mode: ImageMode::Off,
            indexed: false,
        }
        .render(area, &mut buf);
        assert_eq!(buf, Buffer::empty(area));
        assert_eq!(fitted(&thumb, Rect::new(0, 0, 0, 5)).width, 0);
    }

    #[test]
    fn maps_to_xterm_palette() {
        assert_eq!(to_indexed([255, 0, 0]), Color::Indexed(196));
        assert_eq!(to_indexed([0, 0, 0]), Color::Indexed(16));
        assert_eq!(to_indexed([128, 128, 128]), Color::Indexed(244));
    }

    #[test]
    fn fits_keeping_aspect_ratio() {
        let wide = image(40, 10, &[[0; 3]; 400]);
        let r = fitted(&wide, Rect::new(0, 0, 20, 20));
        assert_eq!((r.width, r.height), (20, 3));
        let tall = image(10, 40, &[[0; 3]; 400]);
        let r = fitted(&tall, Rect::new(0, 0, 20, 10));
        assert_eq!((r.x, r.width, r.height), (7, 5, 10), "centered");
    }
}
