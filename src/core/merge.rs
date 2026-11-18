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
