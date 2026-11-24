# Noteki

Light native app (Rust, GPUI) to read and write the Markdown Vault kept with Obsidian.

Start from:
- `docs/SPEC.md` — product specification and budgets
- `CONTEXT.md` — domain vocabulary (use these terms, avoid the listed synonyms)
- `docs/adr/` — architecture decisions (flag conflicts before overriding)
- `README.md` — layout, build and platform notes (macOS and Linux)

## Architecture

- Keep domain and editor rules in `src/core/` (no GPUI dependency) and unit-test them there.
- `src/editor/` and `src/app/` wire `core` to GPUI windows and the filesystem.
- Keep element IDs and `.test_support()` markers stable: `tests/ui.rs` drives the real `NotekiApp` headlessly through them.

## Commands

- `cargo test --lib` — unit tests (`core` and settings)
- `cargo test --test ui` — headless GPUI UI tests
- `cargo test --test demo_vault --test regressions` — demo vault and regression suites
- `scripts/eval.sh` — full verification suite (writes `target/eval-report.md`)
- `cargo test --release --test keystroke_timing -- --ignored --nocapture` — keystroke, search, and scroll (`scroll_timing`) timings
- `scripts/bundle.sh` — builds ad-hoc signed `target/release/Noteki.app`
- `scripts/release.sh` — builds (`dist`), signs, notarizes, and publishes GitHub Release + Sparkle appcast + Homebrew cask (`--dry-run` skips publishing). Bump `version` and `[package.metadata.noteki] build` in `Cargo.toml`, add the version section in `CHANGELOG.md`, commit, and push `main` first.

## Agent skills & docs

- **Issue tracker**: local Markdown files under `.scratch/`. See `docs/agents/issue-tracker.md`.
- **Domain docs**: single-context `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
