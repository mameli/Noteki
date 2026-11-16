#!/usr/bin/env bash
# Mutation check: puts back, one at a time, bugs that were fixed (and a few
# plausible new ones) and verifies the test suite catches each. A surviving
# mutant means a hole in the tests.
#
#   scripts/mutants.sh            # all mutants
#   scripts/mutants.sh 3          # only mutant #3
set -uo pipefail
cd "$(dirname "$0")/.."

# file | text to replace | replacement | test target (cargo test args) | what it simulates
MUTANTS=(
  "src/core/markdown.rs|i += 1 + next;|i += 2;|--test robustness --test regressions|baseline bug: backslash skips 2 bytes (crash on \\è)"
  "src/core/markdown.rs|b'-' \| b'.' \| b'_' \| b'~' \| b'/'|b'-' \| b'.' \| b'_' \| b'~' \| b'/' \| b' '|--test demo_vault --test regressions|baseline bug: wikilinks with spaces are not links in the Reader"
  "src/core/markdown.rs|if after < to \&\& b[after] == b'*' \&\& single_star_in_double|if single_star_in_double|--test robustness|fuzzer-found bug: emphasis scan lands inside a multi-byte char"
  "src/core/markdown.rs|if reader_can_parse(\&md) {|if true {|--test ui|no guard: markdown-rs panic reaches GPUI's TextView"
  "src/core/document.rs|self.is_dirty() \&\& self.conflict.is_none() \&\& !self.deleted|self.is_dirty() \&\& self.conflict.is_none()|--lib --test ui|autosave recreates a note deleted by another app"
  "src/core/tabs.rs|Some(a) if a > index => Some(a - 1),|Some(a) if a > index => Some(a),|--lib --test robustness|closing a tab on the left activates the wrong tab"
  "src/core/index.rs|if note.path == target_path {|if false {|--lib --test demo_vault|a note lists itself among its backlinks"
  "src/app/mod.rs|Duration::from_millis(1000);|Duration::from_millis(10);|--test ui|autosave without the one-second delay"
  "src/app/mod.rs|to_save.push(path.clone());|let _ = \&path;|--test ui|a merge is not saved (SPEC: merged, then saved)"
  "src/app/mod.rs|self.follow_rename(\&old, \&new);|let _ = (\&old, \&new);|--test ui|a rename by another app is not followed"
  "src/core/editor/buffer.rs|self.marked = (!new.is_empty()).then(\|\| r.start..r.start + new.len());|self.marked = None;|--lib --test ui|a dead key's accent is not kept as composed text"
  "src/editor/element.rs|std::mem::take(\&mut ed.reveal).then|false.then|--test ui|the view does not follow the cursor"
  "src/editor/element.rs|end = prev_grapheme(text, end).max(start);|end = end.max(start);|--test ui|⌘→ lands after the space a row wraps at"
)

# Restore with a fresh mtime: `mv` would keep the backup's older mtime and
# cargo would keep using the mutated build afterwards.
restore() { cat "$1.mutant-backup" >"$1" && rm "$1.mutant-backup" && touch "$1"; }

only="${1:-}"
killed=0; survived=0; total=0
report=()
for i in "${!MUTANTS[@]}"; do
  n=$((i + 1))
  [[ -n "$only" && "$only" != "$n" ]] && continue
  IFS='|' read -r file from to tests what <<<"$(printf '%s' "${MUTANTS[$i]}" | sed 's/\\|/\x01/g')"
  from="${from//$'\x01'/|}"; to="${to//$'\x01'/|}"
  from="${from//\\&/&}"; to="${to//\\&/&}"
  cp "$file" "$file.mutant-backup"
  FROM="$from" TO="$to" perl -0pi -e 's/\Q$ENV{FROM}\E/$ENV{TO}/' "$file"
  if cmp -s "$file" "$file.mutant-backup"; then
    restore "$file"
    echo "#$n NOT APPLIED (pattern not found): $what"; report+=("| $n | $what | not applied |"); total=$((total + 1)); survived=$((survived + 1)); continue
  fi
  total=$((total + 1))
  # A mutant that does not compile proves nothing about the tests.
  # shellcheck disable=SC2086
  if ! cargo test -q --no-run $tests >/dev/null 2>&1; then
    restore "$file"
    survived=$((survived + 1))
    echo "#$n INVALID (does not compile): $what"; report+=("| $n | $what | invalid: does not compile |"); continue
  fi
  # shellcheck disable=SC2086
  if cargo test -q $tests >/dev/null 2>&1; then
    survived=$((survived + 1)); verdict="SURVIVED"
  else
    killed=$((killed + 1)); verdict="killed"
  fi
  restore "$file"
  echo "#$n $verdict: $what"
  report+=("| $n | $what | $verdict |")
done

echo
echo "| # | Mutant | Result |"
echo "|---|---|---|"
printf '%s\n' "${report[@]}"
echo
echo "Mutation score: $killed/$total killed"
[[ $survived -eq 0 ]]
