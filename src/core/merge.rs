use similar::{ChangeTag, TextDiff};
use std::collections::BTreeMap;

/// Three-way line-by-line merge (matching NotekiCore's `Merge.swift`).
/// Returns `None` on conflict.
pub fn merge_three_way(base: &str, mine: &str, theirs: &str) -> Option<String> {
    let base_lines = split_lines(base);
    let mine_lines = split_lines(mine);
    let theirs_lines = split_lines(theirs);

    let in_mine = unchanged_map(&base_lines, &mine_lines);
    let in_theirs = unchanged_map(&base_lines, &theirs_lines);

    let mut merged: Vec<&str> = Vec::new();
    let (mut base_start, mut mine_start, mut theirs_start) = (0usize, 0usize, 0usize);

    for (&base_idx, &mine_idx) in &in_mine {
        let Some(&theirs_idx) = in_theirs.get(&base_idx) else {
            continue;
        };
        let part = resolve(
            &base_lines[base_start..base_idx],
            &mine_lines[mine_start..mine_idx],
            &theirs_lines[theirs_start..theirs_idx],
        )?;
        merged.extend_from_slice(part);
        merged.push(base_lines[base_idx]);
        base_start = base_idx + 1;
        mine_start = mine_idx + 1;
        theirs_start = theirs_idx + 1;
    }

    let last = resolve(
        &base_lines[base_start..],
        &mine_lines[mine_start..],
        &theirs_lines[theirs_start..],
    )?;
    merged.extend_from_slice(last);
    Some(merged.join("\n"))
}

/// Where `offset` in `old` stands in `new`, after another app changed the
/// text (a reload, a Merge): on the same line, at the same column, wherever
/// that line went. On a line the change replaced, it goes to the start of
/// what replaced it.
pub fn follow_offset(old: &str, new: &str, offset: usize) -> usize {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let starts = |lines: &[&str]| {
        let mut at = 0;
        lines
            .iter()
            .map(|l| {
                let start = at;
                at += l.len() + 1;
                start
            })
            .collect::<Vec<_>>()
    };
    let (old_starts, new_starts) = (starts(&old_lines), starts(&new_lines));
    let offset = offset.min(old.len());
    let line = old_starts
        .partition_point(|&s| s <= offset)
        .saturating_sub(1);
    let column = offset - old_starts[line];
    let diff = TextDiff::from_slices(&old_lines, &new_lines);
    for op in diff.ops() {
        let (old_range, new_range) = (op.old_range(), op.new_range());
        if !old_range.contains(&line) {
            continue;
        }
        let new_line = if op.tag() == similar::DiffTag::Equal {
            new_range.start + (line - old_range.start)
        } else {
            new_range.start
        };
        let Some(&start) = new_starts.get(new_line) else {
            return new.len();
        };
        let mut at = start + column.min(new_lines[new_line].len());
        while !new.is_char_boundary(at) {
            at -= 1;
        }
        return at;
    }
    offset.min(new.len())
}

fn resolve<'a>(
    base: &[&'a str],
    mine: &'a [&'a str],
    theirs: &'a [&'a str],
) -> Option<&'a [&'a str]> {
    if mine == base {
        Some(theirs)
    } else if theirs == base || theirs == mine {
        Some(mine)
    } else {
        None
    }
}

fn unchanged_map(base: &[&str], other: &[&str]) -> BTreeMap<usize, usize> {
    let diff = TextDiff::from_slices(base, other);
    let mut kept = BTreeMap::new();
    for op in diff.ops() {
        for change in diff.iter_changes(op) {
            if change.tag() == ChangeTag::Equal {
                if let (Some(old_i), Some(new_i)) = (change.old_index(), change.new_index()) {
                    kept.insert(old_i, new_i);
                }
            }
        }
    }
    kept
}

fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_follows_its_line_when_text_comes_above_it() {
        let old = "# Meeting\n\n## Personal notes\nMie note";
        let new = "# Meeting\n\n## Summary\nBy Takku\n\n## Personal notes\nMie note";
        let at = old.len(); // after "Mie note"
        assert_eq!(follow_offset(old, new, at), new.len());
        let at = old.rfind("note").unwrap();
        assert_eq!(&new[follow_offset(old, new, at)..], "note");
    }

    #[test]
    fn the_cursor_stays_when_text_comes_below_it() {
        let old = "Mie note\n\nEnd";
        let new = "Mie note\n\nEnd\n## Summary\nBy Takku";
        assert_eq!(follow_offset(old, new, 3), 3);
    }

    #[test]
    fn on_a_replaced_line_the_cursor_goes_to_what_replaced_it() {
        let old = "A\nold line\nC";
        let new = "A\nnew\nC";
        assert_eq!(follow_offset(old, new, 8), 2 + 3); // column clamped
        assert_eq!(follow_offset("A\nB\nC", "A\nC", 2), 2); // line gone: the next
        assert_eq!(follow_offset("abcdef\nx", "abè\nx", 3), 2); // never inside a character
    }

    #[test]
    fn merges_disjoint_edits() {
        let base = "Title\n\nIntro\n\nSummary\n";
        let mine = "Title\n\nIntro edited\n\nSummary\n";
        let theirs = "Title\n\nIntro\n\nSummary by Takku\n";
        assert_eq!(
            merge_three_way(base, mine, theirs).as_deref(),
            Some("Title\n\nIntro edited\n\nSummary by Takku\n")
        );
    }

    #[test]
    fn detects_conflict_on_same_line() {
        let base = "Title\nLine\nEnd";
        let mine = "Title\nMine\nEnd";
        let theirs = "Title\nTheirs\nEnd";
        assert_eq!(merge_three_way(base, mine, theirs), None);
    }

    #[test]
    fn identical_edits_do_not_conflict() {
        let base = "A\nB\nC";
        let mine = "A\nX\nC";
        let theirs = "A\nX\nC";
        assert_eq!(
            merge_three_way(base, mine, theirs).as_deref(),
            Some("A\nX\nC")
        );
    }
}
