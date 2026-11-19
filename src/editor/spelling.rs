//! Spelling (SPEC › Window: checked and corrected, each on by default), by
//! the Mac's own spell checker: its languages, the words learned in any app,
//! its guesses. Nothing on other systems.

use std::ops::Range;

/// Whether this system checks spelling.
pub const AVAILABLE: bool = cfg!(target_os = "macos");

/// The misspelled words of `text`, as byte ranges.
pub fn misspelled(text: &str) -> Vec<Range<usize>> {
    #[cfg(target_os = "macos")]
    return mac::misspelled(text);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Vec::new()
    }
}

/// What the word `word` (a range of `text`) may have meant, the likeliest first.
pub fn guesses(text: &str, word: Range<usize>) -> Vec<String> {
    #[cfg(target_os = "macos")]
    return mac::guesses(text, word);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (text, word);
        Vec::new()
    }
}

/// The correction the Mac would make of the misspelled `word` of `text`, as
/// it types: `None` when the word is right, or the checker has none.
pub fn correction(text: &str, word: Range<usize>) -> Option<String> {
    #[cfg(target_os = "macos")]
    return mac::correction(text, word);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (text, word);
        None
    }
}

/// *Learn Spelling*: the word is right from now on, in every app.
pub fn learn(word: &str) {
    #[cfg(target_os = "macos")]
    mac::learn(word);
    #[cfg(not(target_os = "macos"))]
    let _ = word;
}

/// *Ignore Spelling*: the word is right while Noteki runs.
pub fn ignore(word: &str) {
    #[cfg(target_os = "macos")]
    mac::ignore(word);
    #[cfg(not(target_os = "macos"))]
    let _ = word;
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ops::Range;
    use std::sync::OnceLock;

    use objc2::rc::Retained;
    use objc2_app_kit::NSSpellChecker;
    use objc2_foundation::{NSInteger, NSRange, NSString};

    use crate::core::editor::buffer::{offset_from_utf16, offset_to_utf16};

    /// The app's one spell document: its ignored words.
    fn tag() -> NSInteger {
        static TAG: OnceLock<NSInteger> = OnceLock::new();
        *TAG.get_or_init(NSSpellChecker::uniqueSpellDocumentTag)
    }

    fn utf16_range(text: &str, r: &Range<usize>) -> NSRange {
        let start = offset_to_utf16(text, r.start);
        NSRange::new(start, offset_to_utf16(text, r.end) - start)
    }

    pub fn misspelled(text: &str) -> Vec<Range<usize>> {
        use objc2_foundation::NSTextCheckingType;
        let checker = NSSpellChecker::sharedSpellChecker();
        let string = NSString::from_str(text);
        let all = NSRange::new(0, string.length());
        // The language found per paragraph, as the Mac's text views do: a
        // Note in Italian and English is checked in both.
        // SAFETY: no options, no orthography given, a null word count.
        let results = unsafe {
            checker.checkString_range_types_options_inSpellDocumentWithTag_orthography_wordCount(
                &string,
                all,
                NSTextCheckingType::Spelling.0,
                None,
                tag(),
                None,
                std::ptr::null_mut(),
            )
        };
        results
            .iter()
            .filter(|r| r.resultType() == NSTextCheckingType::Spelling)
            .map(|r| {
                let r = r.range();
                offset_from_utf16(text, r.location)..offset_from_utf16(text, r.location + r.length)
            })
            .collect()
    }

    /// The language of the text (the word's paragraph): a word in Italian is
    /// corrected in Italian, whatever the Mac's first language.
    fn language_of(checker: &NSSpellChecker, string: &NSString) -> Retained<NSString> {
        use objc2_foundation::NSTextCheckingType;
        let mut orthography = None;
        // SAFETY: no options; the orthography found is written to a local.
        unsafe {
            checker.checkString_range_types_options_inSpellDocumentWithTag_orthography_wordCount(
                string,
                NSRange::new(0, string.length()),
                NSTextCheckingType::Orthography.0,
                None,
                tag(),
                Some(&mut orthography),
                std::ptr::null_mut(),
            );
        }
        orthography
            .map(|o| o.dominantLanguage())
            .filter(|l| l.length() > 0 && l.to_string() != "und")
            .unwrap_or_else(|| checker.language())
    }

    pub fn guesses(text: &str, word: Range<usize>) -> Vec<String> {
        let checker = NSSpellChecker::sharedSpellChecker();
        let (string, range) = (NSString::from_str(text), utf16_range(text, &word));
        let language = language_of(&checker, &string);
        checker
            .guessesForWordRange_inString_language_inSpellDocumentWithTag(
                range,
                &string,
                Some(&language),
                tag(),
            )
            .map(|list| list.iter().map(|g| g.to_string()).collect())
            .unwrap_or_default()
    }

    pub fn correction(text: &str, word: Range<usize>) -> Option<String> {
        let checker = NSSpellChecker::sharedSpellChecker();
        let (string, range) = (NSString::from_str(text), utf16_range(text, &word));
        let language = language_of(&checker, &string);
        checker
            .correctionForWordRange_inString_language_inSpellDocumentWithTag(
                range,
                &string,
                &language,
                tag(),
            )
            .map(|c| c.to_string())
    }

    pub fn learn(word: &str) {
        NSSpellChecker::sharedSpellChecker().learnWord(&NSString::from_str(word));
    }

    pub fn ignore(word: &str) {
        NSSpellChecker::sharedSpellChecker()
            .ignoreWord_inSpellDocumentWithTag(&NSString::from_str(word), tag());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /// The Mac's checker, read only: nothing learned or ignored.
    #[test]
    fn the_macs_checker_finds_a_misspelled_word_and_guesses() {
        let text = "This sentense is fine";
        let found = misspelled(text);
        assert_eq!(found, vec![5..13], "{found:?}");
        assert!(guesses(text, 5..13).iter().any(|g| g == "sentence"));
    }

    #[test]
    fn a_word_is_corrected_in_the_language_of_its_sentence() {
        let it = "Questo testo ha un errrore evidente e lo correggiamo subito.";
        let word = it.find("errrore").unwrap();
        assert_eq!(correction(it, word..word + 7).as_deref(), Some("errore"));
        let en = "This sentense is fine";
        assert_eq!(correction(en, 5..13).as_deref(), Some("sentence"));
    }
}
