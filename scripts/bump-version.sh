#!/usr/bin/env bash
# Bump the docvera version everywhere and keep every manifest in sync.
#
# The single source of truth for the four Rust crates is
# [workspace.package].version in the root Cargo.toml; every member crate uses
# `version.workspace = true`. This script updates that one line, the
# docvera-engine requirement in [workspace.dependencies] and the single npm
# version in npm/docvera/package.json, then refreshes Cargo.lock and verifies
# everything agrees.
#
# Usage:
#   ./scripts/bump-version.sh 0.2.0
#
# Afterwards, commit and tag:
#   git commit -am "Release v0.2.0"
#   git tag v0.2.0 && git push origin v0.2.0
# The release workflow's check-version job verifies all versions match the tag.

set -euo pipefail

die() { printf 'bump-version: %s\n' "$*" >&2; exit 1; }

# --- args + validation ------------------------------------------------------

NEW="${1:-}"
[ -n "$NEW" ] || die "usage: $0 <version>   (e.g. 0.2.0, 1.0.0-rc.1)"

printf '%s' "$NEW" | grep -Eq \
  '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$' \
  || die "'$NEW' is not a valid semver (expecting e.g. 0.2.0)"

# Run from the repo root regardless of where the script is invoked from.
cd "$(dirname "$0")/.."

[ -f Cargo.toml ] && [ -d crates/docvera-engine ] && [ -d npm/docvera ] \
  || die "not running from the docvera project root (expected Cargo.toml + crates/docvera-engine + npm/docvera)"

# --- helper: portable in-place sed ------------------------------------------
# GNU and BSD sed disagree on `-i`; the `.bak` suffix form works on both.
inplace() {  # inplace <pattern> <file>
  sed -i.bak "$1" "$2" && rm -f "$2.bak"
}

OLD=$(awk '
  /^\[workspace\.package\]/ { in_ws=1; next }
  /^\[/                      { in_ws=0 }
  in_ws && /^version *=/     { gsub(/version *= *"|"$/, ""); print; exit }
' Cargo.toml)
[ -n "$OLD" ] || die "could not find [workspace.package].version in Cargo.toml"
printf 'bumping %s -> %s\n\n' "$OLD" "$NEW"

# --- 1. Cargo (workspace source of truth) ------------------------------------
# The only literal `version = "..."` line in the root Cargo.toml is the one
# under [workspace.package]; the member crates inherit it.

inplace "s|^version = \".*\"|version = \"$NEW\"|" Cargo.toml

# The CLI resolves docvera-engine through [workspace.dependencies]; keep the
# requirement in lockstep or a published CLI would pull an older engine.

inplace "s|^\(docvera-engine = .*version = \"\)[^\"]*\(\"\)|\1$NEW\2|" Cargo.toml
# WASM disables the engine's default features and has its own dependency entry.
inplace "s|^\(docvera-engine = .*version = \"\)[^\"]*\(\"\)|\1$NEW\2|" crates/docvera-wasm/Cargo.toml

# --- 2. npm (npm/docvera/package.json) ----------------------------------------

NPM=npm/docvera/package.json

inplace "s|\"version\": \".*\"|\"version\": \"$NEW\"|" "$NPM"


# --- 3. refresh Cargo.lock ----------------------------------------------------
# Update only the workspace members in the lockfile; a bare `cargo update`
# would also churn third-party dependencies and break --locked CI.

cargo update -p docvera-engine -p docvera-cli -p docvera-node -p docvera-wasm --offline \
  || cargo update -p docvera-engine -p docvera-cli -p docvera-node -p docvera-wasm

# --- 4. verify everything agrees ----------------------------------------------

# Cargo: every crate inherits [workspace.package].version, so `cargo metadata`
# (which dereferences inheritance) must yield exactly one version.

CARGO_VERSIONS=$(cargo metadata --format-version 1 --no-deps \
  | grep -o '"version":"[^"]*"' | sed 's/"version":"//; s/"//' | sort -u)
[ "$(printf '%s\n' "$CARGO_VERSIONS" | wc -l)" -eq 1 ] \
  || die "cargo crates disagree on version:\n$CARGO_VERSIONS"

NPM_V=$(grep -m1 '"version"' "$NPM" | sed 's/.*: "\(.*\)".*/\1/' | tr -d ',')
ENGINE_REQ=$(sed -n 's/^docvera-engine = .*version = "\([^"]*\)".*/\1/p' Cargo.toml)
WASM_ENGINE_REQ=$(sed -n 's/^docvera-engine = .*version = "\([^"]*\)".*/\1/p' crates/docvera-wasm/Cargo.toml)

echo "cargo (all crates)   = $CARGO_VERSIONS"
echo "engine requirement   = $ENGINE_REQ"
echo "wasm engine req      = $WASM_ENGINE_REQ"
echo "npm main             = $NPM_V"

[ "$CARGO_VERSIONS" = "$NEW" ]      || die "cargo version is '$CARGO_VERSIONS', expected '$NEW'"
[ "$ENGINE_REQ"    = "$NEW" ]       || die "engine requirement is '$ENGINE_REQ', expected '$NEW'"
[ "$WASM_ENGINE_REQ" = "$NEW" ] || die "WASM engine requirement is '$WASM_ENGINE_REQ', expected '$NEW'"
[ "$NPM_V"         = "$NEW" ]       || die "npm package.json is '$NPM_V', expected '$NEW'"

printf '\nOK: all manifests at %s\n' "$NEW"
printf 'next: git commit -am "Release v%s" && git tag v%s\n' "$NEW" "$NEW"
