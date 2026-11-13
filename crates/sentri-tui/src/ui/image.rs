//! Image rendering with plain characters, so it works over SSH and in tmux.
//!
//! - Half-block: each cell shows `▀` with the upper pixel as foreground and
//!   the lower pixel as background (two pixels per cell).
//! - ASCII: a grayscale ramp, for terminals with few colors.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use sentri_app::{ImageMode, Thumbnail};

const RAMP: &[u8] = b" .:-=+*#%@";

/// Draws a thumbnail scaled (nearest neighbor) into the area, centered and
/// keeping its aspect ratio.
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

/// Size in cells that fits `thumb` inside `area` keeping aspect ratio.
pub fn fitted(thumb: &Thumbnail, area: Rect) -> Rect {
    if thumb.width == 0 || thumb.height == 0 || area.width == 0 || area.height == 0 {
        return Rect::new(area.x, area.y, 0, 0);
    }
    // Each cell is two pixels tall in half-block mode.
    let max_w = u32::from(area.width);
    let max_h = u32::from(area.height) * 2;
    let scale_w = max_w * thumb.height;
    let scale_h = max_h * thumb.width;
    let (w, h) = if scale_w <= scale_h {
        (max_w, (max_w * thumb.height / thumb.width).max(1))
    } else {
        ((max_h * thumb.width / thumb.height).max(1), max_h)
    };
    let width = u16::try_from(w).unwrap_or(area.width).min(area.width);
    let height = u16::try_from(h.div_ceil(2))
        .unwrap_or(area.height)
        .min(area.height);
    Rect::new(area.x + (area.width - width) / 2, area.y, width, height)
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
        let px_h = u32::from(target.height) * 2;
        let sample = |cx: u16, py: u32| -> [u8; 3] {
            let x = u32::from(cx) * self.thumb.width / u32::from(target.width);
            let y = py * self.thumb.height / px_h;
            self.thumb.pixel(x, y).unwrap_or([0, 0, 0])
        };
        let color = |rgb: [u8; 3]| {
            if self.indexed {
                to_indexed(rgb)
            } else {
                Color::Rgb(rgb[0], rgb[1], rgb[2])
            }
        };
        for cy in 0..target.height {
            for cx in 0..target.width {
                let top = sample(cx, u32::from(cy) * 2);
                let bottom = sample(cx, u32::from(cy) * 2 + 1);
                let cell = &mut buf[(target.x + cx, target.y + cy)];
                match self.mode {
                    ImageMode::Halfblock => {
                        cell.set_symbol("▀")
                            .set_fg(color(top))
                            .set_bg(color(bottom));
                    }
                    ImageMode::Ascii => {
                        let level = luminance(top).midpoint(luminance(bottom));
                        let steps = u32::try_from(RAMP.len() - 1).unwrap_or(9);
                        let index = usize::try_from(level * steps / 255).unwrap_or(0);
                        cell.set_char(char::from(RAMP[index]));
                    }
                    ImageMode::Off => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> Thumbnail {
        // 2×2: red, green / blue, white
        Thumbnail {
            width: 2,
            height: 2,
            pixels: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]],
        }
    }

    #[test]
    fn halfblock_uses_top_and_bottom_pixels() {
        let thumb = checker();
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
        let thumb = Thumbnail {
            width: 1,
            height: 2,
            pixels: vec![[255, 255, 255], [255, 255, 255]],
        };
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
    fn maps_to_xterm_palette() {
        assert_eq!(to_indexed([255, 0, 0]), Color::Indexed(196));
        assert_eq!(to_indexed([0, 0, 0]), Color::Indexed(16));
        assert_eq!(to_indexed([128, 128, 128]), Color::Indexed(244));
    }

    #[test]
    fn fits_keeping_aspect_ratio() {
        let wide = Thumbnail {
            width: 40,
            height: 10,
            pixels: vec![[0; 3]; 400],
        };
        let r = fitted(&wide, Rect::new(0, 0, 20, 20));
        assert_eq!((r.width, r.height), (20, 3));
    }
}
