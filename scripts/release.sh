#!/usr/bin/env bash
#
# release.sh — Automated release: version bump, codename management, pre-flight, tag, push
#
# Usage:
#   ./scripts/release.sh <version> <codename> [--dry-run]
#
# Example:
#   ./scripts/release.sh 0.2.2 GoBackJack --dry-run   # preview all changes
#   ./scripts/release.sh 0.2.2 GoBackJack             # full release
#
# Prerequisites:
#   - CHANGELOG.md must already have a [VERSION] section (the creative step)
#   - Codename must exist in docs/release-codenames.md candidates
#
# Safety: nothing is committed until ALL validation passes (fmt, clippy, test,
# bundle). If anything fails, the working tree has modifications but no commits —
# just `git checkout .` to reset and try again.
#
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
RESET='\033[0m'

TOTAL_STEPS=11
step() { echo -e "\n${BOLD}[$1/${TOTAL_STEPS}]${RESET} $2"; }
pass() { echo -e "  ${GREEN}PASS${RESET} $1"; }
fail() { echo -e "  ${RED}FAIL${RESET} $1"; exit 1; }
warn() { echo -e "  ${YELLOW}WARN${RESET} $1"; }
info() { echo -e "  $1"; }

# ── Args ──────────────────────────────────────────────────────────────────────

VERSION="${1:-}"
CODENAME="${2:-}"
DRY_RUN=false
for arg in "$@"; do
    [[ "$arg" == "--dry-run" ]] && DRY_RUN=true
done

if [[ -z "$VERSION" || -z "$CODENAME" ]]; then
    echo "Usage: ./scripts/release.sh <version> <codename> [--dry-run]"
    echo "  e.g. ./scripts/release.sh 0.2.2 GoBackJack"
    exit 1
fi

# Validate version format (semver: X.Y.Z)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fail "Invalid version format '$VERSION' — expected X.Y.Z (e.g. 0.2.2)"
fi

TAG="v${VERSION}"
CODENAMES_FILE="docs/release-codenames.md"
CLAUDEMD_FILE="CLAUDE.md"
CARGO_TOMLS=(
    crates/openwurli-dsp/Cargo.toml
    crates/openwurli-plugin/Cargo.toml
    tools/preamp-bench/Cargo.toml
    tools/reed-renderer/Cargo.toml
)

echo -e "${BOLD}OpenWurli release ${TAG} \"${CODENAME}\"${RESET}"
$DRY_RUN && echo -e "${YELLOW}(dry run — preview changes only)${RESET}"

# ── Step 1: Validate args ────────────────────────────────────────────────────

step 1 "Validating arguments"

# Detect current version from openwurli-dsp (single source of truth)
OLD_VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/openwurli-dsp/Cargo.toml)
if [[ -z "$OLD_VERSION" ]]; then
    fail "Could not detect current version from crates/openwurli-dsp/Cargo.toml"
fi

# Track what's already done (for idempotent re-runs after partial failure)
VERSION_ALREADY_BUMPED=false
CODENAME_ALREADY_MOVED=false

if [[ "$OLD_VERSION" == "$VERSION" ]]; then
    VERSION_ALREADY_BUMPED=true
    warn "Version already at ${VERSION} (re-run)"
else
    pass "Version bump: ${OLD_VERSION} → ${VERSION}"
fi

# Codename: check if already moved to Already Used, or still in candidates
if grep -q "| ${CODENAME} |" "$CODENAMES_FILE"; then
    if sed -n '/^## Candidates/,$ p' "$CODENAMES_FILE" | grep -q "| ${CODENAME} |"; then
        pass "Codename '${CODENAME}' found in candidates"
    else
        CODENAME_ALREADY_MOVED=true
        warn "Codename '${CODENAME}' already in Already Used (re-run)"
    fi
else
    fail "Codename '${CODENAME}' not found anywhere in ${CODENAMES_FILE}"
fi

# ── Step 2: Check CHANGELOG.md ───────────────────────────────────────────────

step 2 "Checking CHANGELOG.md"
if grep -q "^## \[${VERSION}\]" CHANGELOG.md; then
    pass "CHANGELOG.md has a [${VERSION}] section"
else
    fail "CHANGELOG.md is missing a [${VERSION}] section. Add release notes before releasing."
fi

# ══════════════════════════════════════════════════════════════════════════════
# Steps 3-5: File modifications (reversible with `git checkout .`)
# ══════════════════════════════════════════════════════════════════════════════

# ── Step 3: Bump versions in Cargo.toml files ────────────────────────────────

step 3 "Bumping versions in Cargo.toml files"
if $VERSION_ALREADY_BUMPED; then
    warn "Already at ${VERSION} — skipping"
else
    for toml in "${CARGO_TOMLS[@]}"; do
        if $DRY_RUN; then
            info "Would update ${toml}: version \"${OLD_VERSION}\" → \"${VERSION}\""
        else
            sed -i "s/^version = \"${OLD_VERSION}\"/version = \"${VERSION}\"/" "$toml"
            pass "Updated ${toml}"
        fi
    done
fi

# ── Step 4: Update CHANGELOG.md link references ─────────────────────────────

step 4 "Updating CHANGELOG.md link references"
if grep -q "compare/v${VERSION}\.\.\.HEAD" CHANGELOG.md && grep -q "^\[${VERSION}\]:" CHANGELOG.md; then
    warn "Links already up to date — skipping"
elif $DRY_RUN; then
    info "Would update [Unreleased] compare link: v${OLD_VERSION} → v${VERSION}"
    info "Would insert [${VERSION}] compare link"
else
    sed -i "s|compare/v${OLD_VERSION}\.\.\.HEAD|compare/v${VERSION}...HEAD|" CHANGELOG.md
    if ! grep -q "^\[${VERSION}\]:" CHANGELOG.md; then
        sed -i "/^\[Unreleased\]:/a\\[${VERSION}]: https://github.com/hal0zer0/openwurli/compare/v${OLD_VERSION}...v${VERSION}" CHANGELOG.md
    fi
    pass "Updated [Unreleased] and added [${VERSION}] link"
fi

# ── Step 5: Update codename tables ───────────────────────────────────────────

step 5 "Updating codename tables"
if $CODENAME_ALREADY_MOVED; then
    warn "Codename already processed — skipping"
else
    # Extract metadata from candidates section (after "## Candidates" line)
    CANDIDATE_LINE=$(sed -n '/^## Candidates/,$ p' "$CODENAMES_FILE" | grep "| ${CODENAME} |")

    LYRIC=$(echo "$CANDIDATE_LINE" | awk -F'|' '{gsub(/^ +| +$/, "", $3); print $3}')
    SONG=$(echo "$CANDIDATE_LINE" | awk -F'|' '{gsub(/^ +| +$/, "", $4); print $4}')
    ARTIST=$(awk "/^### /{artist=\$0} /\\| ${CODENAME} \\|/{print artist}" "$CODENAMES_FILE" | sed 's/^### //; s/ (.*//')

    # Some tables (Soul/R&B, etc.) have artist in column 5
    INLINE_ARTIST=$(echo "$CANDIDATE_LINE" | awk -F'|' '{if (NF >= 6) {gsub(/^ +| +$/, "", $5); print $5}}')
    if [[ -n "$INLINE_ARTIST" ]]; then
        ARTIST="$INLINE_ARTIST"
    fi

    info "Lyric: ${LYRIC}"
    info "Song: ${SONG}"
    info "Artist: ${ARTIST}"

    if $DRY_RUN; then
        info "Would remove candidate line: ${CANDIDATE_LINE}"
        info "Would add to Already Used: | v${VERSION} | ${CODENAME} | ${SONG} | ${ARTIST} |"
        info "Would add to CLAUDE.md: | v${VERSION} | ${CODENAME} | ${LYRIC} — ${SONG}, ${ARTIST} |"
    else
        # Remove from candidates FIRST (before adding to Already Used)
        sed -i "/| ${CODENAME} |/d" "$CODENAMES_FILE"
        pass "Removed from candidates"

        # Add to Already Used table
        CANDIDATES_LINE=$(grep -n "^## Candidates" "$CODENAMES_FILE" | cut -d: -f1)
        LAST_USED_LINE=$(awk -v max="$CANDIDATES_LINE" '/^\| v[0-9]/ && NR < max {line=NR} END{print line}' "$CODENAMES_FILE")
        sed -i "${LAST_USED_LINE}a\\| v${VERSION} | ${CODENAME} | ${SONG} | ${ARTIST} |" "$CODENAMES_FILE"
        pass "Added to Already Used table"

        # Update CLAUDE.md codenames table
        LYRIC_SOURCE="${LYRIC} — ${SONG}, ${ARTIST}"
        LAST_CODENAME_LINE=$(awk '/^## Version Codenames/,/^Format in CHANGELOG/{if (/^\| v[0-9]/) line=NR} END{print line}' "$CLAUDEMD_FILE")
        sed -i "${LAST_CODENAME_LINE}a\\| v${VERSION} | ${CODENAME} | ${LYRIC_SOURCE} |" "$CLAUDEMD_FILE"
        pass "Updated CLAUDE.md codenames table"
    fi
fi

# ══════════════════════════════════════════════════════════════════════════════
# Steps 6-9: Validation (nothing committed yet — safe to fail)
# ══════════════════════════════════════════════════════════════════════════════

# ── Step 6: cargo fmt ────────────────────────────────────────────────────────

step 6 "cargo fmt"
if $DRY_RUN; then
    warn "Dry run — skipping"
else
    cargo fmt 2>&1
    pass "Formatted"
fi

# ── Step 7: cargo clippy ─────────────────────────────────────────────────────

step 7 "cargo clippy --workspace -- -D warnings"
if $DRY_RUN; then
    warn "Dry run — skipping"
else
    if cargo clippy --workspace -- -D warnings 2>&1; then
        pass "No clippy warnings"
    else
        fail "Fix clippy warnings, then re-run (working tree not committed — git checkout . to reset)"
    fi
fi

# ── Step 8: cargo test ───────────────────────────────────────────────────────

step 8 "cargo test --workspace"
if $DRY_RUN; then
    warn "Dry run — skipping"
else
    if cargo test --workspace 2>&1; then
        pass "All tests pass"
    else
        fail "Tests failed (working tree not committed — git checkout . to reset)"
    fi
fi

# ── Step 9: Bundle + install ─────────────────────────────────────────────────

step 9 "cargo xtask bundle openwurli --release"
if $DRY_RUN; then
    warn "Dry run — skipping"
else
    if cargo xtask bundle openwurli --release 2>&1; then
        pass "Plugin bundled"
        cp target/bundled/openwurli.clap ~/.clap/
        cp -r target/bundled/openwurli.vst3 ~/.vst3/
        pass "Installed to ~/.clap/ and ~/.vst3/"
    else
        fail "Bundle failed (working tree not committed — git checkout . to reset)"
    fi
fi

# ══════════════════════════════════════════════════════════════════════════════
# Steps 10-11: Commit, tag, push (only reached if ALL validation passed)
# ══════════════════════════════════════════════════════════════════════════════

if $DRY_RUN; then
    echo -e "\n${GREEN}${BOLD}Dry run complete.${RESET} All validations would pass."
    echo "Run without --dry-run to release."
    exit 0
fi

# ── Step 10: Commit ──────────────────────────────────────────────────────────

step 10 "Committing release"
git add "${CARGO_TOMLS[@]}" Cargo.lock CHANGELOG.md "$CODENAMES_FILE" "$CLAUDEMD_FILE"
git add -u -- '*.rs'
if git diff --cached --quiet; then
    warn "Nothing to commit — already up to date"
else
    git commit -m "$(cat <<EOF
v${VERSION} "${CODENAME}"

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
    pass "Committed"
fi

# ── Step 11: Tag + push ─────────────────────────────────────────────────────

step 11 "Tag and push"
if ! git diff --quiet || ! git diff --cached --quiet; then
    fail "Uncommitted changes after release commit — something went wrong"
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
    if [ "$(git rev-parse "$TAG"^{commit})" = "$(git rev-parse HEAD)" ]; then
        pass "Tag $TAG already exists at HEAD — pushing"
    else
        fail "Tag $TAG exists but points at a different commit"
    fi
else
    git tag -a "$TAG" -m "v${VERSION} \"${CODENAME}\""
    pass "Created tag $TAG"
fi

git push origin main "$TAG"

echo -e "\n${GREEN}${BOLD}Released ${TAG} \"${CODENAME}\"${RESET}"
echo "  GitHub Actions will build release artifacts automatically."
