//! Small terminal chart primitives built from Unicode blocks.
//!
//! Ratatui's `Sparkline` and `BarChart` cover time series; horizontal
//! distribution bars with eighth-block precision are implemented here
//! because they are a few lines and fit every terminal.

use chrono::{Duration, NaiveDate};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use sentri_app::Daily;

const EIGHTHS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// A bar of `value / max` over `width` cells, with sub-cell precision.
pub fn bar(value: i64, max: i64, width: u16) -> String {
    if max <= 0 || width == 0 || value <= 0 {
        return String::new();
    }
    let eighths = (u64::try_from(value).unwrap_or(0) * u64::from(width) * 8)
        .div_ceil(u64::try_from(max).unwrap_or(1))
        .min(u64::from(width) * 8);
    let full = usize::try_from(eighths / 8).unwrap_or(0);
    let rest = usize::try_from(eighths % 8).unwrap_or(0);
    let mut out = "█".repeat(full);
    if rest > 0 {
        out.push(EIGHTHS[rest]);
    }
    out
}

/// Shared scale of a group of horizontal bars.
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    pub max: i64,
    pub bar_width: u16,
    pub label_width: usize,
}

/// `label ████▌ 12` with fixed label width.
pub fn hbar_line<'a>(
    prefix: Vec<Span<'a>>,
    label: &str,
    value: i64,
    scale: Scale,
    color: Color,
    muted: Style,
) -> Line<'a> {
    let Scale {
        max,
        bar_width,
        label_width,
    } = scale;
    let mut spans = prefix;
    let label: String = label.chars().take(label_width).collect();
    spans.push(Span::raw(format!("{label:<label_width$} ")));
    let drawn = bar(value, max, bar_width);
    let pad = usize::from(bar_width).saturating_sub(drawn.chars().count());
    spans.push(Span::styled(drawn, Style::new().fg(color)));
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled(format!(" {value}"), muted));
    Line::from(spans)
}

/// The last `days` values of a daily series, oldest first, zero-filled.
pub fn series(daily: &Daily, today: NaiveDate, days: i64) -> Vec<u64> {
    (0..days)
        .rev()
        .map(|back| {
            let day = today - Duration::days(back);
            daily
                .get(&day)
                .and_then(|n| u64::try_from(*n).ok())
                .unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_scale_with_eighth_precision() {
        assert_eq!(bar(10, 10, 4), "████");
        assert_eq!(bar(5, 10, 4), "██");
        assert_eq!(bar(1, 16, 2), "▏");
        assert_eq!(bar(0, 10, 4), "");
        assert_eq!(bar(50, 10, 4), "████", "clamped");
    }

    #[test]
    fn series_fills_missing_days() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
        let mut daily = Daily::new();
        daily.insert(today, 4);
        daily.insert(today - Duration::days(2), 1);
        assert_eq!(series(&daily, today, 4), [0, 1, 0, 4]);
    }
}
