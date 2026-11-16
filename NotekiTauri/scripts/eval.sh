#!/usr/bin/env bash
# One command to evaluate the Tauri prototype. Writes src-tauri/target/eval-report.md.
#
#   scripts/eval.sh              # everything but mutants
#   scripts/eval.sh --mutants    # also the mutation check of the tests
#   FUZZ=100000 scripts/eval.sh  # a longer robustness run
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
FUZZ="${FUZZ:-20000}"
REPORT=src-tauri/target/eval-report.md
LOGS=src-tauri/target/eval-logs
mkdir -p "$LOGS"
rows=(); failures=0

step() { # name, log, command...
  local name="$1" log="$LOGS/$2.log"; shift 2
  local start=$SECONDS status
  if "$@" >"$log" 2>&1; then status=PASS; else status=FAIL; failures=$((failures + 1)); fi
  local detail
  detail="$(grep -hE '^test result:' "$log" | awk '{p+=$4; f+=$6} END {if (NR) printf "%d passed, %d failed", p, f}')"
  printf '%-36s %s  %3ss  %s\n' "$name" "$status" "$((SECONDS - start))" "$detail"
  rows+=("| $name | $status | ${detail:-see log} | $((SECONDS - start)) s | \`$log\` |")
}

in_rust() { (cd "$ROOT/src-tauri" && "$@"); }

smoke() {
  local dir; dir="$(mktemp -d)"
  cp -R ../docs/demo-vault "$dir/vault"
  node build.mjs && in_rust cargo build -q --release || return 1
  NOTEKI_SETTINGS="$dir/settings.json" NOTEKI_VAULT="$dir/vault" src-tauri/target/release/noteki-tauri >"$dir/out.log" 2>&1 &
  local pid=$!
  sleep 6
  printf '\n- Added by the smoke test\n' >>"$dir/vault/Ideas.md"   # the watcher must cope
  sleep 2
  local alive=0; kill -0 "$pid" 2>/dev/null && alive=1
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  cat "$dir/out.log"; cat "$dir/settings.json" 2>/dev/null
  echo "alive after 8 s: $alive"
  # The web view ran app.ts and opened the first note through the backend:
  grep -q '"recentNotes": \[' "$dir/settings.json" && grep -q 'Ideas.md' "$dir/settings.json" || { echo "the window never opened a note"; return 1; }
  ! grep -qi 'panicked' "$dir/out.log" && [[ $alive == 1 ]]
}

echo "Evaluating NotekiTauri ($(git rev-parse --short HEAD)$(git diff --quiet -- . || echo '+dirty'))"
step "Format (cargo fmt --check)" fmt in_rust cargo fmt --check
step "Lints (clippy -D warnings)" clippy in_rust cargo clippy -q --all-targets -- -D warnings
step "Unit tests" unit in_rust cargo test -q --lib
step "Backend session tests" session in_rust cargo test -q --test session
step "Demo vault golden checks" demo in_rust cargo test -q --test demo_vault
step "Robustness fuzz ($FUZZ cases/prop)" robustness in_rust env NOTEKI_FUZZ_CASES="$FUZZ" cargo test -q --release --test robustness
step "TypeScript typecheck (app.ts)" tsc ../Editor/node_modules/.bin/tsc -p tsconfig.json
step "Frontend build" web node build.mjs
step "Performance budgets (5,000 notes)" perf in_rust cargo run -q --release --example perf
step "Smoke run of the real app" smoke smoke
[[ "${1:-}" == "--mutants" ]] && step "Mutation check" mutants scripts/mutants.sh

{
  echo "# NotekiTauri evaluation"
  echo
  echo "$(date '+%Y-%m-%d %H:%M') · $(rustc --version) · $(sw_vers -productName 2>/dev/null) $(sw_vers -productVersion 2>/dev/null)"
  echo
  echo "| Check | Result | Detail | Time | Log |"
  echo "|---|---|---|---:|---|"
  printf '%s\n' "${rows[@]}"
  echo; echo "## Performance"; echo
  grep -E '^\||^Vault|^First' "$LOGS/perf.log" || true
  if [[ -f "$LOGS/mutants.log" && "${1:-}" == "--mutants" ]]; then
    echo; echo "## Mutation check"; echo
    grep -E '^\||^Mutation score' "$LOGS/mutants.log"
  fi
} >"$REPORT"
echo; echo "Report: $REPORT"
[[ $failures -eq 0 ]] && echo "ALL CHECKS PASSED" || echo "$failures CHECK(S) FAILED"
exit $((failures > 0))
