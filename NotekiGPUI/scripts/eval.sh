#!/usr/bin/env bash
# One command to evaluate the GPUI prototype. Writes target/eval-report.md.
#
#   scripts/eval.sh              # everything but mutants (~2 min after a first build)
#   scripts/eval.sh --mutants    # also the mutation check (~1 min more)
#   FUZZ=100000 scripts/eval.sh  # a longer robustness run
set -uo pipefail
cd "$(dirname "$0")/.."

FUZZ="${FUZZ:-20000}"
REPORT=target/eval-report.md
LOGS=target/eval-logs
mkdir -p "$LOGS"
rows=()
failures=0

step() { # name, log file, command...
  local name="$1" log="$LOGS/$2.log"; shift 2
  local start=$SECONDS
  if "$@" >"$log" 2>&1; then status=PASS; else status=FAIL; failures=$((failures + 1)); fi
  local detail
  detail="$(grep -hE '^test result:' "$log" | awk '{p+=$4; f+=$6} END {if (NR) printf "%d passed, %d failed", p, f}')"
  printf '%-34s %s  %3ss  %s\n' "$name" "$status" "$((SECONDS - start))" "$detail"
  rows+=("| $name | $status | ${detail:-see log} | $((SECONDS - start)) s | \`$log\` |")
}

smoke() {
  local dir; dir="$(mktemp -d)"
  cp -R ../docs/demo-vault "$dir/vault"
  cargo build -q --release || return 1
  NOTEKI_SETTINGS="$dir/settings.json" NOTEKI_VAULT="$dir/vault" ./target/release/noteki >"$dir/out.log" 2>&1 &
  local pid=$!
  sleep 4
  # Touch a note as another app would: the watcher must not crash the app.
  printf '\n- Added by the smoke test\n' >>"$dir/vault/Ideas.md"
  sleep 2
  local alive=0; kill -0 "$pid" 2>/dev/null && alive=1
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  cat "$dir/out.log"
  echo "alive after 6 s: $alive"
  grep -q '"open_tabs"' "$dir/settings.json" && grep -q 'Welcome.md' "$dir/settings.json" && echo "session persisted: yes"
  ! grep -qi 'panicked' "$dir/out.log" && [[ $alive == 1 ]] && grep -q 'Welcome.md' "$dir/settings.json"
}

echo "Evaluating NotekiGPUI ($(git rev-parse --short HEAD 2>/dev/null)$(git diff --quiet 2>/dev/null || echo '+dirty'))"
step "Format (cargo fmt --check)" fmt cargo fmt --check
step "Lints (clippy -D warnings)" clippy cargo clippy -q --all-targets -- -D warnings
step "Unit tests (core, settings)" unit cargo test -q --lib
step "Regressions of the first prototype" regressions cargo test -q --test regressions
step "Robustness fuzz ($FUZZ cases/prop)" robustness env NOTEKI_FUZZ_CASES="$FUZZ" cargo test -q --release --test robustness -- --nocapture
step "Demo vault golden checks" demo cargo test -q --test demo_vault
step "Headless UI tests (GPUI)" ui cargo test -q --test ui
step "Performance budgets (5,000 notes)" perf cargo run -q --release --example perf
step "Smoke run of the real app" smoke smoke
if [[ "${1:-}" == "--mutants" ]]; then
  step "Mutation check" mutants scripts/mutants.sh
fi

{
  echo "# NotekiGPUI evaluation"
  echo
  echo "$(date '+%Y-%m-%d %H:%M') · $(rustc --version) · $(sw_vers -productName 2>/dev/null) $(sw_vers -productVersion 2>/dev/null)"
  echo
  echo "| Check | Result | Detail | Time | Log |"
  echo "|---|---|---|---:|---|"
  printf '%s\n' "${rows[@]}"
  echo
  echo "## Performance"
  echo
  grep -E '^\||^Vault|^First' "$LOGS/perf.log" || true
  echo
  echo "## Robustness"
  echo
  grep -E 'markdown-rs panicked' "$LOGS/robustness.log" || true
  if [[ -f "$LOGS/mutants.log" ]]; then
    echo
    echo "## Mutation check"
    echo
    grep -E '^\||^Mutation score' "$LOGS/mutants.log"
  fi
} >"$REPORT"

echo
echo "Report: $REPORT"
[[ $failures -eq 0 ]] && echo "ALL CHECKS PASSED" || echo "$failures CHECK(S) FAILED"
exit $((failures > 0))
