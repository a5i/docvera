#!/usr/bin/env bash
# Bootstrap ONLY @a5i/docvera for npm Trusted Publishing (OIDC).
# Like eregex, real releases contain all platform binaries in one npm package.
# Uses local npm login for a deprecated 0.0.0 placeholder; no GitHub NPM_TOKEN.
# Usage: bash scripts/npm-trusted-publishing-bootstrap.sh [--dry-run]
set -euo pipefail

die() { printf 'npm-bootstrap: %s\n' "$*" >&2; exit 1; }

DRY_RUN=0
[ $# -le 1 ] || die "usage: $0 [--dry-run]"
case "${1:-}" in
  '') ;;
  --dry-run) DRY_RUN=1 ;;
  *) die "usage: $0 [--dry-run]" ;;
esac

NAME="@a5i/docvera"
PLACEHOLDER="0.0.0"
cd "$(dirname "$0")/.."
[ -f npm/docvera/package.json ] || die "expected npm/docvera/package.json"

npm whoami >/dev/null 2>&1 || die "not logged in to npm; run: npm login"
echo "npm user: $(npm whoami)"
[ "$DRY_RUN" -eq 1 ] && echo "(dry run: nothing will reach the registry)"

deprecate_placeholder() {
  local i
  for i in 1 2 3 4 5 6; do
    if npm deprecate "$NAME@$PLACEHOLDER" \
      "Bootstrap placeholder for npm Trusted Publishing setup; use >= 0.1.0."; then
      return 0
    fi
    echo "   deprecate failed (attempt $i/6) — npm may still be processing" >&2
    [ "$i" -eq 6 ] || sleep 20
  done
  echo "   WARNING: could not deprecate $NAME@$PLACEHOLDER; run later:" >&2
  echo "     npm deprecate $NAME@$PLACEHOLDER 'Bootstrap placeholder for npm Trusted Publishing setup; use >= 0.1.0.'" >&2
}

if npm view "$NAME@$PLACEHOLDER" version >/dev/null 2>&1; then
  echo "$NAME@$PLACEHOLDER already on npm — skip publish"
  if [ "$DRY_RUN" -eq 0 ] && [ -z "$(npm view "$NAME@$PLACEHOLDER" deprecated 2>/dev/null)" ]; then
    deprecate_placeholder
  fi
else
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  cp npm/docvera/package.json npm/docvera/index.js npm/docvera/index.d.ts npm/docvera/platforms.cjs "$tmp/"
  cp LICENSE "$tmp/LICENSE"
  sed -i.bak "s|\"version\": \".*\"|\"version\": \"$PLACEHOLDER\"|" "$tmp/package.json"
  rm -f "$tmp/package.json.bak"
  if [ "$DRY_RUN" -eq 1 ]; then
    npm publish --dry-run --access public "$tmp"
  else
    if ! npm publish --access public "$tmp"; then
      # A publish conflict can mean a previous attempt landed despite registry lag.
      npm view "$NAME@$PLACEHOLDER" version >/dev/null 2>&1 \
        || die "npm publish failed for $NAME"
      echo "$NAME@$PLACEHOLDER is on npm — continuing"
    fi
    deprecate_placeholder
  fi
fi

cat <<'EOF'

One-time setup for ONLY @a5i/docvera:
  https://www.npmjs.com/package/@a5i/docvera/access
  organization/user: a5i
  repository:        docvera
  workflow filename: release.yml
  environment:       (leave empty)
  allowed actions:   npm stage publish ONLY (direct npm publish stays off)

No @a5i/docvera-<platform> packages are needed. Previously bootstrapped
platform placeholders are unused; no Trusted Publishers are needed for them.

Commit all release files (including new scripts/workflow) before tagging.
Then push the release tag, e.g. v0.1.0.
After CI stages the SINGLE package, inspect with npm stage list/view/download,
then approve it once with 2FA:
  npm stage approve <stage-id>
Stable releases use the latest npm tag; prereleases use next.
EOF
