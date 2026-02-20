#!/usr/bin/env bash
#
# release.sh — Pre-flight checks + tag + push
#
# Mirrors the CI pipeline locally so we never push broken builds.
# Usage:
#   ./scripts/release.sh 0.1.3          # tag v0.1.3 and push
#   ./scripts/release.sh 0.1.3 --dry-run  # run checks only, no tag/push
#
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
RESET='\033[0m'

step() { echo -e "\n${BOLD}[$1/6]${RESET} $2"; }
pass() { echo -e "  ${GREEN}PASS${RESET} $1"; }
fail() { echo -e "  ${RED}FAIL${RESET} $1"; exit 1; }
warn() { echo -e "  ${YELLOW}WARN${RESET} $1"; }

# ── Args ──────────────────────────────────────────────────────────────────────

VERSION="${1:-}"
DRY_RUN=false
[[ "${2:-}" == "--dry-run" ]] && DRY_RUN=true

if [[ -z "$VERSION" ]]; then
    echo "Usage: ./scripts/release.sh <version> [--dry-run]"
    echo "  e.g. ./scripts/release.sh 0.1.3"
    exit 1
fi

TAG="v${VERSION}"
echo -e "${BOLD}OpenWurli release ${TAG}${RESET}"
$DRY_RUN && echo -e "${YELLOW}(dry run — no tag or push)${RESET}"

# ── Pre-flight: clean working tree ────────────────────────────────────────────

step 1 "Checking working tree"
if ! git diff --quiet || ! git diff --cached --quiet; then
    fail "Uncommitted changes. Commit or stash first."
fi
if git rev-parse "$TAG" >/dev/null 2>&1; then
    fail "Tag $TAG already exists."
fi
pass "Clean tree, tag $TAG is available"

# ── Formatting ────────────────────────────────────────────────────────────────

step 2 "cargo fmt --check"
if cargo fmt --check 2>&1; then
    pass "Formatting OK"
else
    fail "Run 'cargo fmt' to fix formatting"
fi

# ── Clippy ────────────────────────────────────────────────────────────────────

step 3 "cargo clippy --workspace -- -D warnings"
if cargo clippy --workspace -- -D warnings 2>&1; then
    pass "No clippy warnings"
else
    fail "Fix clippy warnings before release"
fi

# ── Tests ─────────────────────────────────────────────────────────────────────

step 4 "cargo test --workspace"
if cargo test --workspace 2>&1; then
    pass "All tests pass"
else
    fail "Tests failed"
fi

# ── Bundle ────────────────────────────────────────────────────────────────────

step 5 "cargo xtask bundle openwurli --release"
if cargo xtask bundle openwurli --release 2>&1; then
    pass "Plugin bundled"
    cp target/bundled/openwurli.clap ~/.clap/
    cp -r target/bundled/openwurli.vst3 ~/.vst3/
    pass "Installed to ~/.clap/ and ~/.vst3/"
else
    fail "Bundle failed"
fi

# ── Tag + Push ────────────────────────────────────────────────────────────────

step 6 "Tag and push"
if $DRY_RUN; then
    warn "Dry run — skipping tag and push"
    echo -e "\n${GREEN}${BOLD}All checks passed.${RESET} Run without --dry-run to release."
    exit 0
fi

git tag -a "$TAG" -m "$TAG"
git push origin main "$TAG"

echo -e "\n${GREEN}${BOLD}Released ${TAG}${RESET}"
echo "  GitHub Actions will build release artifacts automatically."
