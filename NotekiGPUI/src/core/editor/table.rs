//! Tables as Live preview draws them in place (ADR 0008): which lines are a
//! table's rows, where each cell's text is in its line, how each column aligns.

use std::ops::Range;
use std::rc::Rc;

use crate::core::markdown::{BlockKind, Line};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// A whole table: what every row's layout depends on, so that a change to
/// any cell gives every row of the table a new style (and a new layout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub align: Vec<Align>,
    /// The cells' text of every row but the delimiter row, the header first.
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Header,
    /// The `|---|:---:|` row under the header, drawn as a rule.
    Delimiter,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub table: Rc<Table>,
    pub kind: RowKind,
    /// Each cell's text in the line, without its spaces around (relative to
    /// the line's start). A body row's cells past the header's are not shown.
    pub cells: Vec<Range<usize>>,
    /// The cursor is in it: its cells show all their text, wrapped in their
    /// columns, instead of cut with an ellipsis (the table stays drawn).
    pub editing: bool,
}

/// The cells of a row: the text between unescaped pipes, trimmed; the pipes
/// at the ends are optional.
fn cells(line: &str) -> Vec<Range<usize>> {
    let b = line.as_bytes();
    let mut bounds = vec![];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'|' => bounds.push(i),
            _ => {}
        }
        i += 1;
    }
    let mut out = Vec::new();
    let mut start = 0;
    for end in bounds.into_iter().chain([line.len()]) {
        let raw = &line[start..end];
        let lead = raw.len() - raw.trim_start().len();
        let text = raw.trim();
        out.push(start + lead..start + lead + text.len());
        start = end + 1;
    }
    // `| a | b |`: the empty pieces before the first pipe and after the last.
    if line.trim_start().starts_with('|') && out.first().is_some_and(|r| r.is_empty()) {
        out.remove(0);
    }
    if line.trim_end().ends_with('|')
        && !line.trim_end().ends_with("\\|")
        && out.last().is_some_and(|r| r.is_empty())
    {
        out.pop();
    }
    out
}

fn has_pipe(line: &str) -> bool {
    let b = line.as_bytes();
    (0..b.len()).any(|i| b[i] == b'|' && (i == 0 || b[i - 1] != b'\\'))
}

fn delimiter(line: &str, cells: &[Range<usize>]) -> Option<Vec<Align>> {
    cells
        .iter()
        .map(|r| {
            let c = &line[r.clone()];
            let dashes = c.trim_start_matches(':').trim_end_matches(':');
            if dashes.is_empty() || !dashes.bytes().all(|b| b == b'-') {
                return None;
            }
            Some(match (c.starts_with(':'), c.ends_with(':')) {
                (true, true) => Align::Center,
                (false, true) => Align::Right,
                _ => Align::Left,
            })
        })
        .collect()
}

/// The table rows among `lines`: `(line index, row)`.
pub fn tables(lines: &[Line]) -> Vec<(usize, TableRow)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        let (head, delim) = (&lines[i], &lines[i + 1]);
        let is_text = |l: &Line| l.kind == BlockKind::Text;
        let header_cells = cells(head.text);
        let delim_cells = cells(delim.text);
        let align =
            (is_text(head) && is_text(delim) && has_pipe(head.text) && has_pipe(delim.text))
                .then(|| delimiter(delim.text, &delim_cells))
                .flatten()
                .filter(|a| a.len() == header_cells.len());
        let Some(align) = align else {
            i += 1;
            continue;
        };
        let columns = align.len();
        let mut rows = vec![
            (i, RowKind::Header, header_cells),
            (i + 1, RowKind::Delimiter, delim_cells),
        ];
        let mut j = i + 2;
        while j < lines.len() && is_text(&lines[j]) && has_pipe(lines[j].text) {
            let mut c = cells(lines[j].text);
            c.truncate(columns);
            rows.push((j, RowKind::Body, c));
            j += 1;
        }
        let table = Rc::new(Table {
            align,
            rows: rows
                .iter()
                .filter(|(_, k, _)| *k != RowKind::Delimiter)
                .map(|(l, _, c)| {
                    c.iter()
                        .map(|r| lines[*l].text[r.clone()].to_string())
                        .collect()
                })
                .collect(),
        });
        for (l, kind, cells) in rows {
            out.push((
                l,
                TableRow {
                    table: table.clone(),
                    kind,
                    cells,
                    editing: false,
                },
            ));
        }
        i = j;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::markdown::lines;

    const TABLE: &str = "| Nome | Tipo | Stato |\n| :--- | :---: | ---: |\n| Markdown | Testo | OK |\n| a \\| b | `x` |\nfine";

    #[test]
    fn rows_cells_and_alignments() {
        let lines = lines(TABLE);
        let t = tables(&lines);
        let kinds: Vec<_> = t.iter().map(|(l, r)| (*l, r.kind)).collect();
        assert_eq!(
            kinds,
            vec![
                (0, RowKind::Header),
                (1, RowKind::Delimiter),
                (2, RowKind::Body),
                (3, RowKind::Body)
            ]
        );
        let table = &t[0].1.table;
        assert_eq!(table.align, vec![Align::Left, Align::Center, Align::Right]);
        assert_eq!(table.rows[0], vec!["Nome", "Tipo", "Stato"]);
        assert_eq!(
            table.rows[2],
            vec!["a \\| b", "`x`"],
            "an escaped pipe stays in its cell"
        );
        let header = &t[0].1.cells;
        assert_eq!(&lines[0].text[header[0].clone()], "Nome");
    }

    #[test]
    fn no_table_without_a_delimiter_row_or_in_code() {
        assert!(tables(&lines("a | b\nc | d")).is_empty());
        assert!(tables(&lines("```\n| a |\n|---|\n```")).is_empty());
        assert!(
            tables(&lines("| a | b |\n| --- |")).is_empty(),
            "columns must match"
        );
        assert_eq!(
            tables(&lines("a | b\n--|--\n1 | 2")).len(),
            3,
            "no outer pipes"
        );
    }
}
