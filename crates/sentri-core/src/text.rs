//! Sanitization of untrusted text before it is stored or rendered.

/// Removes control characters (including ANSI escape sequences' ESC byte),
/// collapses whitespace and truncates to `max_chars` characters.
///
/// External content is untrusted: a raw ESC byte rendered in a terminal could
/// change colors, move the cursor or worse, so it never survives this function.
pub fn clean(input: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(input.len().min(max_chars * 4));
    let mut count = 0;
    let mut pending_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            pending_space = count > 0;
            continue;
        }
        if ch.is_control() || is_invisible_format(ch) {
            continue;
        }
        if pending_space {
            if count + 1 >= max_chars {
                break;
            }
            out.push(' ');
            count += 1;
            pending_space = false;
        }
        if count >= max_chars {
            break;
        }
        out.push(ch);
        count += 1;
    }
    if count >= max_chars && input.chars().count() > max_chars {
        out.pop();
        out.push('…');
    }
    out
}

/// Like [`clean`] but returns `None` for empty results.
pub fn clean_opt(input: Option<&str>, max_chars: usize) -> Option<String> {
    input.map(|s| clean(s, max_chars)).filter(|s| !s.is_empty())
}

/// Bidirectional overrides and zero-width characters can disguise text.
fn is_invisible_format(ch: char) -> bool {
    matches!(ch, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_escape_sequences_and_controls() {
        assert_eq!(clean("\u{1b}[31mred\u{1b}[0m\u{7}", 100), "[31mred[0m");
        assert_eq!(clean("a\u{202E}b", 100), "ab");
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(clean("  hello \n\t world  ", 100), "hello world");
    }

    #[test]
    fn truncates_with_ellipsis() {
        assert_eq!(clean("abcdefgh", 5), "abcd…");
        assert_eq!(clean("abcde", 5), "abcde");
        assert_eq!(clean("ñandú ñandú", 7).chars().count(), 7);
    }

    #[test]
    fn clean_opt_drops_empty() {
        assert_eq!(clean_opt(Some(" \u{1b} "), 10), None);
        assert_eq!(clean_opt(None, 10), None);
    }
}
