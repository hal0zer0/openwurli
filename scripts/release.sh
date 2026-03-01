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

# Codename must exist in candidates
if ! grep -q "| ${CODENAME} |" "$CODENAMES_FILE"; then
    fail "Codename '${CODENAME}' not found in ${CODENAMES_FILE} candidates"
fi
pass "Codename '${CODENAME}' found in candidates"

# Detect old version from openwurli-dsp (single source of truth)
OLD_VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/openwurli-dsp/Cargo.toml)
if [[ -z "$OLD_VERSION" ]]; then
    fail "Could not detect current version from crates/openwurli-dsp/Cargo.toml"
fi
if [[ "$OLD_VERSION" == "$VERSION" ]]; then
    fail "Version is already ${VERSION} — nothing to bump"
fi
pass "Version bump: ${OLD_VERSION} → ${VERSION}"

# ── Step 2: Check CHANGELOG.md ───────────────────────────────────────────────

step 2 "Checking CHANGELOG.md"
if grep -q "^## \[${VERSION}\]" CHANGELOG.md; then
    pass "CHANGELOG.md has a [${VERSION}] section"
else
    fail "CHANGELOG.md is missing a [${VERSION}] section. Add release notes before releasing."
fi

# ── Step 3: Bump versions in Cargo.toml files ────────────────────────────────

step 3 "Bumping versions in Cargo.toml files"
for toml in "${CARGO_TOMLS[@]}"; do
    if $DRY_RUN; then
        info "Would update ${toml}: version \"${OLD_VERSION}\" → \"${VERSION}\""
    else
        sed -i "s/^version = \"${OLD_VERSION}\"/version = \"${VERSION}\"/" "$toml"
        pass "Updated ${toml}"
    fi
done

# ── Step 4: Update CHANGELOG.md link references ─────────────────────────────

step 4 "Updating CHANGELOG.md link references"
if $DRY_RUN; then
    info "Would update [Unreleased] compare link: v${OLD_VERSION} → v${VERSION}"
    info "Would insert [${VERSION}] compare link"
else
    # Update [Unreleased] compare link to point at new tag
    sed -i "s|compare/v${OLD_VERSION}\.\.\.HEAD|compare/v${VERSION}...HEAD|" CHANGELOG.md
    # Insert new version compare link after [Unreleased] line (skip if already present)
    if ! grep -q "^\[${VERSION}\]:" CHANGELOG.md; then
        sed -i "/^\[Unreleased\]:/a\\[${VERSION}]: https://github.com/hal0zer0/openwurli/compare/v${OLD_VERSION}...v${VERSION}" CHANGELOG.md
    fi
    pass "Updated [Unreleased] and added [${VERSION}] link"
fi

# ── Step 5: Update codename tables ───────────────────────────────────────────

step 5 "Updating codename tables"

# Extract metadata from candidates section (after "## Candidates" line)
# The candidate line looks like: | GoBackJack | "Go back, Jack" | Do It Again |
CANDIDATE_LINE=$(sed -n '/^## Candidates/,$ p' "$CODENAMES_FILE" | grep "| ${CODENAME} |")

# Extract lyric (column 2) and song (column 3)
LYRIC=$(echo "$CANDIDATE_LINE" | awk -F'|' '{gsub(/^ +| +$/, "", $3); print $3}')
SONG=$(echo "$CANDIDATE_LINE" | awk -F'|' '{gsub(/^ +| +$/, "", $4); print $4}')

# Extract artist from the section header above the candidate
# Look for the ### header preceding this codename's line
ARTIST=$(awk "/^### /{artist=\$0} /\\| ${CODENAME} \\|/{print artist}" "$CODENAMES_FILE" | sed 's/^### //; s/ (.*//')

# Some tables (Soul/R&B, etc.) have artist in column 4/5 — check for that
INLINE_ARTIST=$(echo "$CANDIDATE_LINE" | awk -F'|' '{if (NF >= 6) {gsub(/^ +| +$/, "", $5); print $5}}')
if [[ -n "$INLINE_ARTIST" ]]; then
    ARTIST="$INLINE_ARTIST"
fi

info "Lyric: ${LYRIC}"
info "Song: ${SONG}"
info "Artist: ${ARTIST}"

# 5a: Remove from candidates (BEFORE adding to Already Used, to avoid matching the new row)
if $DRY_RUN; then
    info "Would remove candidate line: ${CANDIDATE_LINE}"
else
    sed -i "/| ${CODENAME} |/d" "$CODENAMES_FILE"
    pass "Removed from candidates"
fi

# 5b: Add to Already Used table
if $DRY_RUN; then
    info "Would add to Already Used: | v${VERSION} | ${CODENAME} | ${SONG} | ${ARTIST} |"
else
    # Find the last "| v" line in the Already Used section (before ## Candidates)
    CANDIDATES_LINE=$(grep -n "^## Candidates" "$CODENAMES_FILE" | cut -d: -f1)
    LAST_USED_LINE=$(awk -v max="$CANDIDATES_LINE" '/^\| v[0-9]/ && NR < max {line=NR} END{print line}' "$CODENAMES_FILE")
    sed -i "${LAST_USED_LINE}a\\| v${VERSION} | ${CODENAME} | ${SONG} | ${ARTIST} |" "$CODENAMES_FILE"
    pass "Added to Already Used table"
fi

# 5c: Update CLAUDE.md codenames table
# LYRIC already has quotes from the table (e.g. "Go back, Jack"), use as-is
LYRIC_SOURCE="${LYRIC} — ${SONG}, ${ARTIST}"
if $DRY_RUN; then
    info "Would add to CLAUDE.md: | v${VERSION} | ${CODENAME} | ${LYRIC_SOURCE} |"
else
    # Find the last "| v" line in the Version Codenames table
    # The table is between "## Version Codenames" and "Format in CHANGELOG"
    LAST_CODENAME_LINE=$(awk '/^## Version Codenames/,/^Format in CHANGELOG/{if (/^\| v[0-9]/) line=NR} END{print line}' "$CLAUDEMD_FILE")
    sed -i "${LAST_CODENAME_LINE}a\\| v${VERSION} | ${CODENAME} | ${LYRIC_SOURCE} |" "$CLAUDEMD_FILE"
    pass "Updated CLAUDE.md codenames table"
fi

# ── Step 6: Commit version/codename changes ──────────────────────────────────

step 6 "Committing release changes"
if $DRY_RUN; then
    warn "Dry run — skipping commit"
else
    git add "${CARGO_TOMLS[@]}" CHANGELOG.md "$CODENAMES_FILE" "$CLAUDEMD_FILE"
    git commit -m "$(cat <<EOF
Release v${VERSION} "${CODENAME}"

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
    pass "Committed release changes"
fi

# ── Step 7: cargo fmt ────────────────────────────────────────────────────────

step 7 "cargo fmt --check"
if $DRY_RUN; then
    warn "Dry run — skipping"
else
    if cargo fmt --check 2>&1; then
        pass "Formatting OK"
    else
        fail "Run 'cargo fmt' to fix formatting"
    fi
fi

# ── Step 8: cargo clippy ─────────────────────────────────────────────────────

step 8 "cargo clippy --workspace -- -D warnings"
if $DRY_RUN; then
    warn "Dry run — skipping"
else
    if cargo clippy --workspace -- -D warnings 2>&1; then
        pass "No clippy warnings"
    else
        fail "Fix clippy warnings before release"
    fi
fi

# ── Step 9: cargo test ───────────────────────────────────────────────────────

step 9 "cargo test --workspace"
if $DRY_RUN; then
    warn "Dry run — skipping"
else
    if cargo test --workspace 2>&1; then
        pass "All tests pass"
    else
        fail "Tests failed"
    fi
fi

# ── Step 10: Bundle + install ────────────────────────────────────────────────

step 10 "cargo xtask bundle openwurli --release"
if $DRY_RUN; then
    warn "Dry run — skipping"
else
    if cargo xtask bundle openwurli --release 2>&1; then
        pass "Plugin bundled"
        cp target/bundled/openwurli.clap ~/.clap/
        cp -r target/bundled/openwurli.vst3 ~/.vst3/
        pass "Installed to ~/.clap/ and ~/.vst3/"
    else
        fail "Bundle failed"
    fi
fi

# ── Step 11: Tag + push ─────────────────────────────────────────────────────

step 11 "Tag and push"
if $DRY_RUN; then
    warn "Dry run — skipping tag and push"
    echo -e "\n${GREEN}${BOLD}Dry run complete.${RESET} All validations passed."
    echo "Run without --dry-run to release."
    exit 0
fi

# Clean working tree check
if ! git diff --quiet || ! git diff --cached --quiet; then
    fail "Uncommitted changes. Commit or stash first."
fi
pass "Clean working tree"

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
