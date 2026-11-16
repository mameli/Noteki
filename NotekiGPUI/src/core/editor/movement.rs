//! Where the cursor goes, as a byte offset into the text: by character, by word,
//! to a line's ends. Pure functions; every offset returned is on a char boundary.

use unicode_segmentation::UnicodeSegmentation;

/// The largest char boundary at or before `offset`.
pub fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut o = offset.min(text.len());
    while !text.is_char_boundary(o) {
        o -= 1;
    }
    o
}

/// The start of the grapheme cluster before `offset` (`è` written as `e` + accent,
/// an emoji with modifiers and `\r\n` are one step).
pub fn prev_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    text[..offset]
        .grapheme_indices(true)
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// The end of the grapheme cluster that starts at or contains `offset`.
pub fn next_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    text[offset..]
        .graphemes(true)
        .next()
        .map(|g| offset + g.len())
        .unwrap_or(text.len())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// ⌥←: the start of the word before `offset`, skipping the spaces and
/// punctuation between, as macOS text views do.
pub fn prev_word_start(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    let mut chars = text[..offset].char_indices().rev().peekable();
    while chars.peek().is_some_and(|&(_, c)| !is_word_char(c)) {
        chars.next();
    }
    let mut start = chars.peek().map(|&(i, c)| i + c.len_utf8()).unwrap_or(0);
    for (i, c) in chars {
        if !is_word_char(c) {
            break;
        }
        start = i;
    }
    start
}

/// ⌥→: the end of the word after `offset`, skipping the spaces and
/// punctuation before it.
pub fn next_word_end(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    let mut chars = text[offset..].char_indices().peekable();
    while chars.peek().is_some_and(|&(_, c)| !is_word_char(c)) {
        chars.next();
    }
    let mut end = chars.peek().map(|&(i, _)| offset + i).unwrap_or(text.len());
    for (i, c) in chars {
        if !is_word_char(c) {
            break;
        }
        end = offset + i + c.len_utf8();
    }
    end
}

/// The start of the logical line holding `offset` (just after the previous `\n`).
pub fn line_start(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    text[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

/// The end of the logical line holding `offset`: before its `\n`, or its `\r\n`.
pub fn line_end(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    let end = text[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(text.len());
    if end > offset && text[..end].ends_with('\r') {
        end - 1
    } else {
        end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphemes_step_over_combining_accents_emoji_and_crlf() {
        let t = "e\u{301}x👍🏽y\r\nz";
        assert_eq!(next_grapheme(t, 0), 3); // e + combining acute
        assert_eq!(prev_grapheme(t, 3), 0);
        let thumbs = 4;
        assert_eq!(next_grapheme(t, thumbs), thumbs + 8); // 👍🏽 is 8 bytes
        let cr = t.find('\r').unwrap();
        assert_eq!(next_grapheme(t, cr), cr + 2);
        assert_eq!(prev_grapheme(t, cr + 2), cr);
        assert_eq!(prev_grapheme(t, 0), 0);
        assert_eq!(next_grapheme(t, t.len()), t.len());
    }

    #[test]
    fn offsets_inside_a_char_are_floored() {
        let t = "aè";
        assert_eq!(floor_boundary(t, 2), 1);
        assert_eq!(next_grapheme(t, 2), 3);
        assert_eq!(prev_grapheme(t, 2), 0);
    }

    #[test]
    fn words_skip_spaces_and_punctuation() {
        let t = "Ciao, mondo  bello";
        assert_eq!(next_word_end(t, 0), 4);
        assert_eq!(next_word_end(t, 4), 11);
        assert_eq!(next_word_end(t, 11), t.len());
        assert_eq!(prev_word_start(t, t.len()), 13);
        assert_eq!(prev_word_start(t, 13), 6);
        assert_eq!(prev_word_start(t, 6), 0);
        assert_eq!(prev_word_start(t, 0), 0);
        assert_eq!(next_word_end(t, t.len()), t.len());
    }

    #[test]
    fn words_count_accented_letters() {
        let t = "perché sì";
        assert_eq!(next_word_end(t, 0), "perché".len());
        assert_eq!(prev_word_start(t, t.len()), "perché ".len());
    }

    #[test]
    fn line_ends_stop_before_the_newline_and_crlf() {
        let t = "one\r\ntwo\nthree";
        assert_eq!(line_start(t, 6), 5);
        assert_eq!(line_end(t, 6), 8);
        assert_eq!(line_end(t, 0), 3);
        assert_eq!(line_start(t, 3), 0);
        assert_eq!(line_end(t, t.len()), t.len());
        assert_eq!(line_start(t, t.len()), 9);
        assert_eq!(line_end("", 0), 0);
        // On an empty line.
        assert_eq!(line_end("a\n\nb", 2), 2);
        assert_eq!(line_start("a\n\nb", 2), 2);
    }
}
