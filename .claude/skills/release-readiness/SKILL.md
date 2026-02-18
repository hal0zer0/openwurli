# /release-readiness — GitHub Release Readiness Checker

## Description
Validates that the OpenWurli repository is ready for public GitHub release. Five escalating levels from legal blockers to ship-ready.

## User-invocable
Yes. Usage: `/release-readiness [level]` (default: level 1). Higher levels include all lower levels.

## Instructions

When the user runs `/release-readiness`, execute the checks for the requested level (and all levels below it). Report each check as PASS or FAIL with a brief explanation. Stop at the first level that has any FAIL and summarize.

### Level 1 — Legal Blockers

These must pass before anything else. A single failure here blocks release.

1. **LICENSE exists** — `LICENSE` file exists at repo root and contains "GNU GENERAL PUBLIC LICENSE" and "Version 3"
2. **Cargo.toml license field** — Root `Cargo.toml` has `license = "GPL-3.0-or-later"` in `[workspace.package]`
3. **Sub-crate license inheritance** — Every sub-crate `Cargo.toml` has `license.workspace = true`
4. **Schematic PDF not tracked** — `git ls-files docs/verified_wurlitzer_200A_series_schematic.pdf` returns empty
5. **Schematic not in history** — `git log --all --diff-filter=A -- '*.pdf'` returns empty (not just untracked — purged from history)
6. **No MIDI files tracked** — `git ls-files '*.mid'` returns empty
7. **CLAUDE.md not tracked** — `git ls-files CLAUDE.md` returns empty

### Level 2 — Repo Metadata

1. **README.md exists** — Has sections: Features, Install, Build, License (check for `#` headers)
2. **CHANGELOG.md exists** — Has at least an `[Unreleased]` section
3. **Cargo.toml has repository** — `[workspace.package]` has `repository` field pointing to GitHub
4. **Cargo.toml has authors** — `[workspace.package]` has `authors` field
5. **rust-toolchain.toml exists** — File exists with `channel = "stable"`
6. **Plugin URL populated** — `crates/openwurli-plugin/src/lib.rs` has a non-empty `URL` string
7. **SCHEMATIC_SOURCE.md exists** — `docs/SCHEMATIC_SOURCE.md` exists and explains how to obtain the schematic

### Level 3 — Code Hygiene

1. **cargo fmt** — `cargo fmt --check` passes (no formatting issues)
2. **cargo clippy** — `cargo clippy --workspace -- -D warnings` passes
3. **cargo test** — `cargo test --workspace` passes (all tests green)
4. **No TEMPORARY/HACK/FIXME in crates/** — `grep -rn 'TEMPORARY\|HACK\|FIXME' crates/` returns empty
5. **No /home/homeuser in tracked files** — `git grep '/home/homeuser' -- ':!.claude/'` returns empty
6. **No hardcoded /tmp/ in tests** — `grep -rn '"/tmp/' crates/ tools/` returns empty (use `std::env::temp_dir()`)

### Level 4 — Documentation

1. **All 9 docs present** — Check that these files exist in `docs/`:
   - `signal-chain-architecture.md`, `preamp-circuit.md`, `dk-preamp-derivation.md`
   - `dk-preamp-testing.md`, `output-stage.md`, `pickup-system.md`
   - `reed-and-hammer-physics.md`, `calibration-and-evaluation.md`, `spice-rust-mapping.md`
2. **No "AI agent consumption" language** — `grep -rn 'AI agent\|AI consumption\|agent consumption' docs/` returns empty
3. **SCHEMATIC_SOURCE.md exists** — (re-checked from L2)
4. **tools/requirements.txt exists** — Python dependencies documented
5. **.gitignore complete** — Contains entries for: schematic PDF, `*.mid`, `CLAUDE.md`, `target/`, `.venv/`, `schematic_tiles/`

### Level 5 — CI/CD & Ship

1. **CI workflow exists** — `.github/workflows/ci.yml` exists with `cargo test`, `cargo clippy`, `cargo fmt`
2. **Release workflow exists** — `.github/workflows/release.yml` exists with tag trigger
3. **Git remote configured** — `git remote -v` shows an origin
4. **No uncommitted changes** — `git status --porcelain` is empty
5. **Bundle succeeds** — `cargo xtask bundle openwurli --release` completes without error

### Output Format

```
=== Release Readiness: Level N ===

Level 1 — Legal Blockers
  [PASS] LICENSE exists (GPL-3.0)
  [FAIL] Schematic PDF in history (found in commit abc1234)
  ...

RESULT: BLOCKED at Level 1 — 1 failure(s)
```

Or if all pass:

```
=== Release Readiness: Level N ===

Level 1 — Legal Blockers
  [PASS] LICENSE exists (GPL-3.0)
  [PASS] Cargo.toml license field
  ...

Level 2 — Repo Metadata
  [PASS] README.md exists
  ...

RESULT: ALL CLEAR through Level N — ready to ship!
```
