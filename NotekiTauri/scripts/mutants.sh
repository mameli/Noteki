#!/usr/bin/env bash
# Mutation check of the backend tests: puts back, one at a time, bugs that were
# fixed (and plausible new ones); the test suite must fail on each. A mutant
# that does not compile is reported as invalid (it proves nothing).
#
#   scripts/mutants.sh        # all
#   scripts/mutants.sh 3      # only #3
set -uo pipefail
cd "$(dirname "$0")/../src-tauri"

# file <TAB> text to replace <TAB> replacement <TAB> what it simulates
MUTANTS=$(cat <<'TSV'
src/markdown.rs	i += 1 + next;	i += 2;	scanner steps into a multi-byte char after a backslash (crash on \è)
src/session.rs	pub fn reload_rejected(&mut self, path: &str) -> Vec<Event> {	pub fn reload_rejected(&mut self, path: &str) -> Vec<Event> { if !path.is_empty() { return Vec::new(); }	baseline race: a reload refused by the editor is dropped, its save overwrites the other app
src/session.rs	Some(rebased) if rebased == mine => Vec::new()	Some(rebased) if rebased.is_empty() && rebased == mine => Vec::new()	a refused reload with nothing new is sent again (reload loop)
src/session.rs	if od.doc.has_conflict() || od.doc.is_deleted() {	if od.doc.has_conflict() {	saving reports success on a note deleted by another app
src/session.rs	to_save.push(path);	let _ = path;	a merge is not saved (SPEC: merged, then saved)
src/session.rs	od.revision = 0;	let _ = 0;	after a followed rename the editor refuses every reload
src/session.rs	vault.corpus.set(path, &text);	let _ = &text;	search and backlinks miss what was just saved
src/session.rs	!self.docs.contains_key(new) && (t == doc.base() || t == doc.text())	false && !self.docs.contains_key(new) && (t == doc.base() || t == doc.text())	baseline: a rename by another app shows as a deletion
src/lib.rs	m.lock().unwrap_or_else(PoisonError::into_inner)	m.lock().unwrap()	baseline: one panicking command locks every later one out
src/document.rs	if disk == self.base && !was_deleted {	if false && disk == self.base && !was_deleted {	our own save coming back from the watcher counts as an external change
TSV
)

restore() { cat "$1.mutant-backup" >"$1" && rm "$1.mutant-backup" && touch "$1"; }

only="${1:-}"; n=0; killed=0; total=0; rows=()
while IFS=$'\t' read -r file from to what; do
  n=$((n + 1))
  [[ -n "$only" && "$only" != "$n" ]] && continue
  total=$((total + 1))
  cp "$file" "$file.mutant-backup"
  FROM="$from" TO="$to" perl -0pi -e 's/\Q$ENV{FROM}\E/$ENV{TO}/' "$file"
  if cmp -s "$file" "$file.mutant-backup"; then
    verdict="NOT APPLIED"
  elif ! cargo test -q --no-run >/dev/null 2>&1; then
    verdict="INVALID (does not compile)"
  elif cargo test -q >/dev/null 2>&1; then
    verdict="SURVIVED"
  else
    verdict="killed"; killed=$((killed + 1))
  fi
  restore "$file"
  echo "#$n $verdict: $what"
  rows+=("| $n | $what | $verdict |")
done <<<"$MUTANTS"

echo
echo "| # | Mutant | Result |"
echo "|---|---|---|"
printf '%s\n' "${rows[@]}"
echo
echo "Mutation score: $killed/$total killed"
[[ $killed -eq $total ]]
