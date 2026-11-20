//! ASCII icon mode: replaces SENTRI's own Unicode glyphs in the rendered
//! frame with plain ASCII, for terminals and fonts that lack them.
//!
//! Only glyphs SENTRI draws are translated (borders, icons, bars, marks);
//! user content such as "ñ", accents or emoji in titles is left alone.
//! Image half blocks (`▀`) are governed by the separate `images` setting.

use ratatui::buffer::Buffer;

/// Interface glyphs and their ASCII replacement, grouped by replacement.
const TABLE: &[(&[&str], &str)] = &[
    (
        &[
            "╭", "╮", "╰", "╯", "┌", "┐", "└", "┘", "├", "┤", "┬", "┴", "┼",
        ],
        "+",
    ),
    (&["─", "━", "—", "–", "•", "▪"], "-"),
    (&["│", "┃"], "|"),
    (&["▌", "›", "▶", "→"], ">"),
    (&["‹", "◀", "←"], "<"),
    (&["↑", "✈", "↗"], "^"),
    (&["↓"], "v"),
    (&["⇧", "★"], "S"),
    (&["·", "…", "▃", "▂", "▁"], "."),
    (&["◆", "✦", "●"], "*"),
    (&["▤"], "N"),
    (&["⚒"], "J"),
    (&["◷"], "@"),
    (&["⚡", "⏳", "⏰", "⚠"], "!"),
    (&["✪"], "$"),
    (&["♣"], "&"),
    (&["⌥"], "%"),
    (&["◍"], "C"),
    (&["✐"], "W"),
    (&["▦", "◫", "■", "█", "▉", "▊", "▋", "▍", "▎", "▏"], "#"),
    (&["⇅", "⟳"], "~"),
    (&["⚙"], "="),
    (&["♥"], "L"),
    (&["✓"], "A"),
    (&["✎"], "e"),
    (&["◉", "○", "◌", "◎"], "o"),
    (&["✗"], "x"),
    (&["⌂"], "H"),
    (&["◐"], "h"),
    (&["↺"], "r"),
    (&["▇", "▆", "▅", "▄"], ":"),
];

/// Replacement for one interface glyph.
pub fn ascii_for(symbol: &str) -> Option<&'static str> {
    TABLE
        .iter()
        .find(|(glyphs, _)| glyphs.contains(&symbol))
        .map(|(_, ascii)| *ascii)
}

/// Translates every interface glyph in the buffer.
pub fn asciify(buf: &mut Buffer) {
    for cell in &mut buf.content {
        if let Some(replacement) = ascii_for(cell.symbol()) {
            cell.set_symbol(replacement);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn translates_interface_glyphs_and_keeps_content() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));
        for (i, s) in ["╭", "♥", "ñ", "é", "█", "▀"].iter().enumerate() {
            buf[(u16::try_from(i).unwrap(), 0)].set_symbol(s);
        }
        asciify(&mut buf);
        let out: String = buf
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert_eq!(out, "+Lñé#▀");
    }
}
