//! Picks how images are drawn and caches what each drawn size needs.
//!
//! At startup SENTRI asks the terminal (through `ratatui-image`) whether it
//! speaks the Kitty, Sixel or iTerm2 graphics protocol and how large a cell
//! is in pixels. With a protocol, images are drawn with real pixels; without
//! one (most SSH sessions, tmux without passthrough, the Linux console) the
//! half-block renderer in `ui::image` is used. Nothing here assumes a
//! protocol exists: detection failures simply mean half blocks.
//!
//! Every drawn variant (one per image, size and fit) is resampled once from
//! the full-resolution source with Lanczos3, then kept in a small cache so
//! redrawing a frame never resizes or re-encodes anything.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use image::imageops::FilterType;
use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::widgets::Widget;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{FontSize, Resize};
use sentri_app::{ImageMode, Thumbnail};

use crate::ui::image::{cell_pixels, fitted, paint};

/// Drawn variants kept in memory (each one is a few hundred KB at most).
const CACHE_LIMIT: usize = 24;

/// How an image fills its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fit {
    /// Whole image, aspect ratio kept, centered horizontally.
    Contain,
    /// Fills the box, cropping around the center (avatars).
    Cover,
}

/// Which renderer is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Kitty,
    Sixel,
    Iterm2,
    /// Half blocks or ASCII, drawn with ordinary characters.
    Text,
}

impl Backend {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Kitty => "kitty graphics",
            Self::Sixel => "sixel",
            Self::Iterm2 => "iTerm2 images",
            Self::Text => "half blocks",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    id: String,
    width: u16,
    height: u16,
    fit: Fit,
    graphics: bool,
}

enum Variant {
    /// Encoded for a graphics protocol, drawn at `rect` (relative size).
    Protocol(Box<Protocol>, Size),
    /// Pixels already sized for the cell grid.
    Cells(Thumbnail, Size),
    /// The image could not be prepared at this size.
    Failed,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<Key, Variant>,
    order: VecDeque<Key>,
}

impl Cache {
    fn insert(&mut self, key: Key, variant: Variant) {
        if self.entries.insert(key.clone(), variant).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > CACHE_LIMIT {
            if let Some(old) = self.order.pop_front() {
                self.entries.remove(&old);
            }
        }
    }
}

/// The image renderer and its cache of drawn variants.
pub struct Graphics {
    picker: Option<Picker>,
    cache: RefCell<Cache>,
}

impl std::fmt::Debug for Graphics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graphics")
            .field("backend", &self.backend())
            .field("cached", &self.cache.borrow().entries.len())
            .finish_non_exhaustive()
    }
}

impl Default for Graphics {
    fn default() -> Self {
        Self::text()
    }
}

impl Clone for Graphics {
    /// Copies the renderer; the cache starts empty.
    fn clone(&self) -> Self {
        Self {
            picker: self.picker.clone(),
            cache: RefCell::default(),
        }
    }
}

/// The protocol a mode asks for; `None` means "whatever the terminal says".
const fn wanted(mode: ImageMode) -> Option<ProtocolType> {
    match mode {
        ImageMode::Kitty => Some(ProtocolType::Kitty),
        ImageMode::Sixel => Some(ProtocolType::Sixel),
        ImageMode::Iterm2 => Some(ProtocolType::Iterm2),
        _ => None,
    }
}

/// Whether a mode needs the terminal to be asked about graphics.
pub const fn wants_probe(mode: ImageMode) -> bool {
    matches!(
        mode,
        ImageMode::Auto | ImageMode::Kitty | ImageMode::Sixel | ImageMode::Iterm2
    )
}

impl Graphics {
    /// Half blocks and ASCII only.
    pub fn text() -> Self {
        Self {
            picker: None,
            cache: RefCell::default(),
        }
    }

    /// A renderer for a known protocol and cell size (used by tests and
    /// when detection already happened).
    pub fn with_protocol(protocol: ProtocolType, cell: (u16, u16)) -> Self {
        if protocol == ProtocolType::Halfblocks {
            return Self::text();
        }
        #[allow(deprecated)] // the documented way to build a picker without a terminal
        let mut picker = Picker::from_fontsize(FontSize::from(cell));
        picker.set_protocol_type(protocol);
        Self {
            picker: Some(picker),
            cache: RefCell::default(),
        }
    }

    /// Asks the terminal what it supports. Must run after entering the
    /// alternate screen and before terminal events are read. A forced
    /// protocol (`kitty`, `sixel`, `iterm2`) is used even when the terminal
    /// does not announce it, which helps inside tmux or over SSH.
    pub fn detect(mode: ImageMode) -> Self {
        if !wants_probe(mode) {
            return Self::text();
        }
        let picker = match Picker::from_query_stdio() {
            Ok(picker) => picker,
            Err(error) => {
                tracing::debug!(%error, "terminal graphics query failed; using half blocks");
                return Self::text();
            }
        };
        Self::from_picker(picker, mode)
    }

    fn from_picker(mut picker: Picker, mode: ImageMode) -> Self {
        if let Some(protocol) = wanted(mode) {
            picker.set_protocol_type(protocol);
        }
        tracing::debug!(
            protocol = ?picker.protocol_type(),
            font = ?picker.font_size(),
            tmux = picker.tmux_detected(),
            "terminal graphics"
        );
        if picker.protocol_type() == ProtocolType::Halfblocks {
            return Self::text();
        }
        Self {
            picker: Some(picker),
            cache: RefCell::default(),
        }
    }

    pub fn backend(&self) -> Backend {
        match self.picker.as_ref().map(Picker::protocol_type) {
            Some(ProtocolType::Kitty) => Backend::Kitty,
            Some(ProtocolType::Sixel) => Backend::Sixel,
            Some(ProtocolType::Iterm2) => Backend::Iterm2,
            Some(ProtocolType::Halfblocks) | None => Backend::Text,
        }
    }

    /// The renderer used for `mode`: half blocks and ASCII always draw with
    /// characters, even in a terminal that supports graphics.
    fn picker_for(&self, mode: ImageMode) -> Option<&Picker> {
        match mode {
            ImageMode::Halfblock | ImageMode::Ascii | ImageMode::Off => None,
            _ => self.picker.as_ref(),
        }
    }

    /// Draws `image` (identified by `id`, usually its URL) inside `area` and
    /// returns the cells it used (zero-sized when nothing was drawn).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        buf: &mut Buffer,
        area: Rect,
        id: &str,
        image: &Thumbnail,
        fit: Fit,
        mode: ImageMode,
        indexed: bool,
    ) -> Rect {
        let empty = Rect::new(area.x, area.y, 0, 0);
        if mode == ImageMode::Off || area.width == 0 || area.height == 0 {
            return empty;
        }
        let picker = self.picker_for(mode);
        let key = Key {
            id: id.to_owned(),
            width: area.width,
            height: area.height,
            fit,
            graphics: picker.is_some(),
        };
        let mut cache = self.cache.borrow_mut();
        if !cache.entries.contains_key(&key) {
            let variant = match picker {
                Some(picker) => encode(picker, image, area, fit),
                None => cells(image, area, fit),
            };
            cache.insert(key.clone(), variant);
        }
        let Some(variant) = cache.entries.get(&key) else {
            return empty;
        };
        match variant {
            Variant::Protocol(protocol, size) => {
                let rect = placed(area, *size);
                ratatui_image::Image::new(protocol).render(rect, buf);
                rect
            }
            Variant::Cells(pixels, size) => {
                let rect = placed(area, *size);
                paint(pixels, rect, mode, indexed, buf);
                rect
            }
            Variant::Failed => empty,
        }
    }
}

/// `size` centered horizontally at the top of `area`.
fn placed(area: Rect, size: Size) -> Rect {
    let width = size.width.min(area.width);
    let height = size.height.min(area.height);
    Rect::new(area.x + (area.width - width) / 2, area.y, width, height)
}

fn cells(image: &Thumbnail, area: Rect, fit: Fit) -> Variant {
    let (pixels, size) = match fit {
        Fit::Contain => {
            let target = fitted(image, area);
            (
                cell_pixels(image, target),
                Size::new(target.width, target.height),
            )
        }
        Fit::Cover => (
            image.cover(u32::from(area.width), u32::from(area.height) * 2),
            Size::new(area.width, area.height),
        ),
    };
    if size.width == 0 || size.height == 0 {
        return Variant::Failed;
    }
    Variant::Cells(pixels, size)
}

/// Resamples the source to the exact pixel size of the cells it will cover
/// and encodes it once for the terminal's protocol.
fn encode(picker: &Picker, image: &Thumbnail, area: Rect, fit: Fit) -> Variant {
    let font = picker.font_size();
    let (fw, fh) = (u32::from(font.width.max(1)), u32::from(font.height.max(1)));
    let (box_w, box_h) = (u32::from(area.width) * fw, u32::from(area.height) * fh);
    let (pixels, size) = match fit {
        Fit::Contain => {
            let (w, h) = image.fit_size(box_w, box_h);
            let cells = Size::new(
                u16::try_from(w.div_ceil(fw))
                    .unwrap_or(area.width)
                    .min(area.width),
                u16::try_from(h.div_ceil(fh))
                    .unwrap_or(area.height)
                    .min(area.height),
            );
            (image.resized(w, h), cells)
        }
        Fit::Cover => (
            image.cover(box_w, box_h),
            Size::new(area.width, area.height),
        ),
    };
    if size.width == 0 || size.height == 0 {
        return Variant::Failed;
    }
    match picker.new_protocol(
        pixels.to_dynamic(),
        size,
        Resize::Fit(Some(FilterType::Lanczos3)),
    ) {
        Ok(protocol) => Variant::Protocol(Box::new(protocol), size),
        Err(error) => {
            tracing::debug!(%error, "image could not be encoded");
            Variant::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripes(width: u32, height: u32) -> Thumbnail {
        let pixels: Vec<[u8; 3]> = (0..width * height)
            .map(|i| {
                if (i % width) < width / 2 {
                    [200, 40, 40]
                } else {
                    [40, 40, 200]
                }
            })
            .collect();
        Thumbnail::from_pixels(width, height, &pixels).unwrap()
    }

    #[test]
    fn modes_choose_the_renderer() {
        assert!(wants_probe(ImageMode::Auto));
        assert!(wants_probe(ImageMode::Sixel));
        assert!(!wants_probe(ImageMode::Halfblock));
        assert!(!wants_probe(ImageMode::Ascii));
        assert!(!wants_probe(ImageMode::Off));
        assert_eq!(Graphics::text().backend(), Backend::Text);
        assert_eq!(
            Graphics::with_protocol(ProtocolType::Kitty, (10, 20)).backend(),
            Backend::Kitty
        );
        assert_eq!(
            Graphics::with_protocol(ProtocolType::Halfblocks, (10, 20)).backend(),
            Backend::Text,
            "ratatui-image's half blocks are replaced by SENTRI's own"
        );
    }

    #[test]
    fn forced_protocol_overrides_detection() {
        #[allow(deprecated)]
        let picker = Picker::from_fontsize(FontSize::new(8, 16));
        assert_eq!(
            Graphics::from_picker(picker.clone(), ImageMode::Sixel).backend(),
            Backend::Sixel
        );
        assert_eq!(
            Graphics::from_picker(picker, ImageMode::Auto).backend(),
            Backend::Text,
            "no protocol announced: half blocks"
        );
    }

    #[test]
    fn text_renderer_draws_half_blocks_and_caches_variants() {
        let g = Graphics::text();
        let image = stripes(400, 200);
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        let used = g.draw(
            &mut buf,
            area,
            "a",
            &image,
            Fit::Contain,
            ImageMode::Auto,
            false,
        );
        assert_eq!((used.width, used.height), (20, 5));
        assert_eq!(buf[(0, 0)].symbol(), "▀");
        g.draw(
            &mut buf,
            area,
            "a",
            &image,
            Fit::Contain,
            ImageMode::Auto,
            false,
        );
        assert_eq!(g.cache.borrow().entries.len(), 1, "second frame reuses it");
        let used = g.draw(
            &mut buf,
            area,
            "a",
            &image,
            Fit::Cover,
            ImageMode::Halfblock,
            false,
        );
        assert_eq!(used, area, "cover fills the box");
        assert_eq!(g.cache.borrow().entries.len(), 2);
    }

    #[test]
    fn graphics_protocols_cover_whole_cells_at_full_resolution() {
        for protocol in [
            ProtocolType::Kitty,
            ProtocolType::Sixel,
            ProtocolType::Iterm2,
        ] {
            let g = Graphics::with_protocol(protocol, (10, 20));
            let image = stripes(400, 200);
            let area = Rect::new(2, 1, 30, 10);
            let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
            let used = g.draw(
                &mut buf,
                area,
                "a",
                &image,
                Fit::Contain,
                ImageMode::Auto,
                false,
            );
            // 300×200 px box → 300×150 image → 30×8 cells (rounded up).
            assert_eq!(
                (used.x, used.width, used.height),
                (2, 30, 8),
                "{protocol:?}"
            );
            assert_ne!(buf[(2, 1)].symbol(), "▀", "{protocol:?} draws real pixels");
            // Forcing half blocks in Settings switches renderer without restart.
            let used = g.draw(
                &mut buf,
                area,
                "a",
                &image,
                Fit::Contain,
                ImageMode::Halfblock,
                false,
            );
            assert_eq!(buf[(used.x, used.y)].symbol(), "▀");
        }
    }

    #[test]
    fn off_and_empty_areas_draw_nothing() {
        let g = Graphics::with_protocol(ProtocolType::Kitty, (10, 20));
        let image = stripes(4, 4);
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = Buffer::empty(area);
        assert_eq!(
            g.draw(
                &mut buf,
                area,
                "a",
                &image,
                Fit::Contain,
                ImageMode::Off,
                false
            )
            .width,
            0
        );
        assert_eq!(buf, Buffer::empty(area));
        let none = Rect::new(0, 0, 0, 4);
        assert_eq!(
            g.draw(
                &mut buf,
                none,
                "a",
                &image,
                Fit::Contain,
                ImageMode::Auto,
                false
            )
            .width,
            0
        );
    }

    #[test]
    fn cache_is_bounded() {
        let g = Graphics::text();
        let image = stripes(8, 8);
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        for i in 0..(CACHE_LIMIT + 10) {
            g.draw(
                &mut buf,
                area,
                &i.to_string(),
                &image,
                Fit::Contain,
                ImageMode::Halfblock,
                false,
            );
        }
        assert_eq!(g.cache.borrow().entries.len(), CACHE_LIMIT);
    }
}
