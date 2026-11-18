//! Find and replace in a Note: what a query matches, and what replacing gives.

use std::ops::Range;

use regex::{Regex, RegexBuilder};

/// A search as the find bar sets it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub match_case: bool,
    pub regex: bool,
    pub whole_word: bool,
}

impl Query {
    /// The query as a regular expression; an error for an invalid regex.
    fn compile(&self) -> Result<Option<Regex>, String> {
        if self.text.is_empty() {
            return Ok(None);
        }
        let pattern = if self.regex {
            self.text.clone()
        } else {
            regex::escape(&self.text)
        };
        let pattern = if self.whole_word {
            format!(r"\b(?:{pattern})\b")
        } else {
            pattern
        };
        RegexBuilder::new(&pattern)
            .case_insensitive(!self.match_case)
            .multi_line(true)
            .build()
            .map(Some)
            .map_err(|e| e.to_string())
    }
}

/// Every match of `query` in `text`, in order, never empty.
pub fn find_all(text: &str, query: &Query) -> Result<Vec<Range<usize>>, String> {
    Ok(match query.compile()? {
        Some(re) => re
            .find_iter(text)
            .map(|m| m.range())
            .filter(|r| !r.is_empty())
            .collect(),
        None => Vec::new(),
    })
}

/// What replaces the match at `range`: `replacement`, with `$1`... filled in
/// for a regex.
pub fn replacement_for(
    text: &str,
    range: Range<usize>,
    query: &Query,
    replacement: &str,
) -> String {
    if !query.regex {
        return replacement.to_string();
    }
    let Ok(Some(re)) = query.compile() else {
        return replacement.to_string();
    };
    match re.captures_at(text, range.start) {
        Some(caps) if caps.get(0).is_some_and(|m| m.range() == range) => {
            let mut out = String::new();
            caps.expand(replacement, &mut out);
            out
        }
        _ => replacement.to_string(),
    }
}

/// The text with every match replaced, and how many there were.
pub fn replace_all(
    text: &str,
    query: &Query,
    replacement: &str,
) -> Result<(String, usize), String> {
    let Some(re) = query.compile()? else {
        return Ok((text.to_string(), 0));
    };
    let count = re.find_iter(text).filter(|m| !m.is_empty()).count();
    let out = if query.regex {
        re.replace_all(text, replacement).into_owned()
    } else {
        re.replace_all(text, regex::NoExpand(replacement))
            .into_owned()
    };
    Ok((out, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str) -> Query {
        Query {
            text: text.into(),
            ..Query::default()
        }
    }

    #[test]
    fn plain_text_ignores_case_unless_asked() {
        let t = "Nota, nota e NOTA; notaio.";
        assert_eq!(find_all(t, &q("nota")).unwrap().len(), 4);
        let case = Query {
            match_case: true,
            ..q("nota")
        };
        assert_eq!(
            find_all(t, &case).unwrap(),
            vec![6..10, 19..23],
            "and in notaio"
        );
        let word = Query {
            whole_word: true,
            ..q("nota")
        };
        assert_eq!(find_all(t, &word).unwrap().len(), 3, "not in notaio");
        assert!(
            find_all(t, &q("(")).unwrap().is_empty(),
            "plain text: no regex"
        );
        assert!(find_all(t, &q("")).unwrap().is_empty());
    }

    #[test]
    fn regex_and_its_errors() {
        let t = "uno 12 due 345";
        let re = Query {
            regex: true,
            ..q(r"\d+")
        };
        assert_eq!(find_all(t, &re).unwrap(), vec![4..6, 11..14]);
        let bad = Query {
            regex: true,
            ..q("(")
        };
        assert!(find_all(t, &bad).is_err());
        let empty_matches = Query {
            regex: true,
            ..q("x*")
        };
        assert!(
            find_all(t, &empty_matches).unwrap().is_empty(),
            "no empty matches"
        );
    }

    #[test]
    fn replacing_one_and_all() {
        let t = "Mario Rossi, Anna Bianchi";
        let re = Query {
            regex: true,
            match_case: true,
            ..q(r"(\w+) (\w+)")
        };
        assert_eq!(replacement_for(t, 0..11, &re, "$2 $1"), "Rossi Mario");
        assert_eq!(
            replace_all(t, &re, "$2 $1").unwrap(),
            ("Rossi Mario, Bianchi Anna".into(), 2)
        );
        // Plain text: a `$` is a dollar.
        assert_eq!(
            replace_all("a b a", &q("a"), "$1").unwrap(),
            ("$1 b $1".into(), 2)
        );
    }
}
