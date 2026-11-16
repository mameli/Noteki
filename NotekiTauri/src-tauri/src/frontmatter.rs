use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineSpan {
    pub line: usize,
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PropertyValue {
    None,
    Text { span: LineSpan },
    List { items: Vec<LineSpan> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Property {
    pub key: String,
    #[serde(rename = "keySpan")]
    pub key_span: LineSpan,
    pub value: PropertyValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum FrontmatterReading {
    Properties { properties: Vec<Property> },
    Unreadable { unreadable: usize },
}

const SPACE: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;
const HASH: u16 = b'#' as u16;
const COLON: u16 = b':' as u16;
const DASH: u16 = b'-' as u16;
const DOT: u16 = b'.' as u16;
const COMMA: u16 = b',' as u16;
const OPEN_BRACKET: u16 = b'[' as u16;
const CLOSE_BRACKET: u16 = b']' as u16;
const OPEN_BRACE: u16 = b'{' as u16;
const DOUBLE_QUOTE: u16 = b'"' as u16;
const SINGLE_QUOTE: u16 = b'\'' as u16;
const BACKSLASH: u16 = b'\\' as u16;

fn is_indicator(u: u16) -> bool {
    matches!(
        u as u8,
        b'[' | b']' | b'{' | b'}' | b'>' | b'|' | b'*' | b'&' | b'!' | b'%' | b'@' | b'`' | b',' | b'?'
    )
}

/// Parses the YAML Frontmatter subset matching NotekiCore's `Frontmatter.swift`.
pub fn parse_frontmatter(text: &str) -> Option<FrontmatterReading> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let lines = split_utf16_lines(&units);
    let first = lines.first()?;
    if !is_delimiter(first, false) {
        return None;
    }
    let end = (1..lines.len()).find(|&idx| is_delimiter(&lines[idx], true))?;
    Some(read_lines(&lines[1..end]))
}

fn split_utf16_lines(units: &[u16]) -> Vec<&[u16]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < units.len() {
        if units[i] == b'\n' as u16 {
            let line_end = if i > start && units[i - 1] == b'\r' as u16 {
                i - 1
            } else {
                i
            };
            out.push(&units[start..line_end]);
            start = i + 1;
        }
        i += 1;
    }
    out.push(&units[start..]);
    out
}

fn is_delimiter(line: &[u16], allow_dots: bool) -> bool {
    let trimmed_end = trim_end_spaces(line);
    if trimmed_end.len() != 3 {
        return false;
    }
    trimmed_end == [DASH, DASH, DASH] || (allow_dots && trimmed_end == [DOT, DOT, DOT])
}

fn trim_end_spaces(line: &[u16]) -> &[u16] {
    let mut end = line.len();
    while end > 0 && (line[end - 1] == SPACE || line[end - 1] == TAB) {
        end -= 1;
    }
    &line[..end]
}

fn read_lines(lines: &[&[u16]]) -> FrontmatterReading {
    let mut properties: Vec<Property> = Vec::new();
    let mut keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut list_owner: Option<usize> = None;
    let mut item_indent: Option<usize> = None;

    for (idx, &units) in lines.iter().enumerate() {
        // Line numbers inside the document are 1-based, and line 1 is `---`.
        let number = idx + 2;
        let indent_len = units.iter().take_while(|&&u| u == SPACE || u == TAB).count();
        let content = &units[indent_len..];
        if content.iter().all(|&u| u == SPACE || u == TAB) || content.first() == Some(&HASH) {
            continue;
        }
        if units[..indent_len].contains(&TAB) {
            return FrontmatterReading::Unreadable { unreadable: number };
        }
        if starts_item(units, indent_len) {
            let Some(owner) = list_owner else {
                return FrontmatterReading::Unreadable { unreadable: number };
            };
            if item_indent.is_some() && item_indent != Some(indent_len) {
                return FrontmatterReading::Unreadable { unreadable: number };
            }
            let Some(item) = scalar(units, indent_len + 1, number) else {
                return FrontmatterReading::Unreadable { unreadable: number };
            };
            item_indent = Some(indent_len);
            if let PropertyValue::List { items } = &mut properties[owner].value {
                items.push(item);
            } else {
                properties[owner].value = PropertyValue::List { items: vec![item] };
            }
            continue;
        }
        if indent_len != 0 {
            return FrontmatterReading::Unreadable { unreadable: number };
        }
        let Some(prop) = parse_property(units, number) else {
            return FrontmatterReading::Unreadable { unreadable: number };
        };
        if !keys.insert(prop.key.clone()) {
            return FrontmatterReading::Unreadable { unreadable: number };
        }
        let is_none = matches!(prop.value, PropertyValue::None);
        properties.push(prop);
        list_owner = if is_none {
            Some(properties.len() - 1)
        } else {
            None
        };
        item_indent = None;
    }

    FrontmatterReading::Properties { properties }
}

fn parse_property(units: &[u16], line: usize) -> Option<Property> {
    let &first = units.first()?;
    if is_indicator(first) {
        return None;
    }
    let colon = (0..units.len())
        .find(|&i| units[i] == COLON && (i + 1 == units.len() || units[i + 1] == SPACE))?;
    let mut key_end = colon;
    while key_end > 0 && units[key_end - 1] == SPACE {
        key_end -= 1;
    }
    if key_end == 0 {
        return None;
    }
    let key = String::from_utf16(&units[..key_end]).ok()?;
    let key_span = LineSpan {
        line,
        from: 0,
        to: key_end,
    };
    let start = skip_spaces(units, colon + 1);
    if start == units.len() || units[start] == HASH {
        return Some(Property {
            key,
            key_span,
            value: PropertyValue::None,
        });
    }
    if units[start] == OPEN_BRACKET {
        let items = flow_list(units, start, line)?;
        return Some(Property {
            key,
            key_span,
            value: PropertyValue::List { items },
        });
    }
    let value = scalar(units, start, line)?;
    Some(Property {
        key,
        key_span,
        value: PropertyValue::Text { span: value },
    })
}

fn scalar(units: &[u16], start: usize, line: usize) -> Option<LineSpan> {
    let start = skip_spaces(units, start);
    if start >= units.len() || units[start] == HASH {
        return Some(LineSpan {
            line,
            from: start,
            to: start,
        });
    }
    if let Some((span, end)) = quoted(units, start, line) {
        return if ends_line(units, end) {
            Some(span)
        } else {
            None
        };
    }
    if is_indicator(units[start]) || starts_item(units, start) {
        return None;
    }
    let mut end = start;
    while end < units.len() {
        if units[end] == HASH && end > 0 && (units[end - 1] == SPACE || units[end - 1] == TAB) {
            break;
        }
        end += 1;
    }
    while end > start && (units[end - 1] == SPACE || units[end - 1] == TAB) {
        end -= 1;
    }
    if is_map(&units[start..end]) {
        return None;
    }
    Some(LineSpan {
        line,
        from: start,
        to: end,
    })
}

fn flow_list(units: &[u16], start: usize, line: usize) -> Option<Vec<LineSpan>> {
    let mut items = Vec::new();
    let mut at = skip_spaces(units, start + 1);
    if at < units.len() && units[at] == CLOSE_BRACKET {
        return if ends_line(units, at + 1) {
            Some(items)
        } else {
            None
        };
    }
    while at < units.len() {
        if let Some((span, end)) = quoted(units, at, line) {
            items.push(span);
            at = skip_spaces(units, end);
        } else {
            let mut end = at;
            while end < units.len() && units[end] != COMMA && units[end] != CLOSE_BRACKET {
                if (is_indicator(units[end]) && end == at)
                    || units[end] == OPEN_BRACKET
                    || units[end] == OPEN_BRACE
                {
                    return None;
                }
                end += 1;
            }
            let mut trimmed = end;
            while trimmed > at && units[trimmed - 1] == SPACE {
                trimmed -= 1;
            }
            if trimmed <= at || is_map(&units[at..trimmed]) {
                return None;
            }
            items.push(LineSpan {
                line,
                from: at,
                to: trimmed,
            });
            at = end;
        }
        if at >= units.len() {
            return None;
        }
        if units[at] == CLOSE_BRACKET {
            return if ends_line(units, at + 1) {
                Some(items)
            } else {
                None
            };
        }
        if units[at] != COMMA {
            return None;
        }
        at = skip_spaces(units, at + 1);
        if at < units.len() && units[at] == CLOSE_BRACKET && !items.is_empty() {
            return if ends_line(units, at + 1) {
                Some(items)
            } else {
                None
            };
        }
    }
    None
}

fn quoted(units: &[u16], start: usize, line: usize) -> Option<(LineSpan, usize)> {
    let quote = units[start];
    if quote != DOUBLE_QUOTE && quote != SINGLE_QUOTE {
        return None;
    }
    let mut at = start + 1;
    while at < units.len() {
        if quote == DOUBLE_QUOTE && units[at] == BACKSLASH {
            at += 2;
            continue;
        }
        if units[at] == quote {
            if quote == SINGLE_QUOTE && at + 1 < units.len() && units[at + 1] == SINGLE_QUOTE {
                at += 2;
                continue;
            }
            return Some((
                LineSpan {
                    line,
                    from: start + 1,
                    to: at,
                },
                at + 1,
            ));
        }
        at += 1;
    }
    None
}

fn is_map(value: &[u16]) -> bool {
    value.last() == Some(&COLON)
        || value
            .windows(2)
            .any(|w| w[0] == COLON && (w[1] == SPACE || w[1] == TAB))
}

fn starts_item(units: &[u16], start: usize) -> bool {
    units[start] == DASH
        && (start + 1 == units.len() || units[start + 1] == SPACE || units[start + 1] == TAB)
}

fn ends_line(units: &[u16], start: usize) -> bool {
    let rest = skip_spaces(units, start);
    rest == units.len() || (units[rest] == HASH && rest > start)
}

fn skip_spaces(units: &[u16], mut at: usize) -> usize {
    while at < units.len() && (units[at] == SPACE || units[at] == TAB) {
        at += 1;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_properties_and_lists() {
        let yaml = "---\ntitle: \"Weekly Sync\"\ntags:\n  - work\n  - meeting\nparticipants: [Alice, Bob]\n---";
        let reading = parse_frontmatter(yaml).unwrap();
        match reading {
            FrontmatterReading::Properties { properties } => {
                assert_eq!(properties.len(), 3);
                assert_eq!(properties[0].key, "title");
                assert_eq!(properties[1].key, "tags");
                assert_eq!(properties[2].key, "participants");
            }
            FrontmatterReading::Unreadable { unreadable } => panic!("Unexpected unreadable at {unreadable}"),
        }
    }
}
