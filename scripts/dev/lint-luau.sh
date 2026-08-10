#!/usr/bin/env bash
# Strict type-check the bundled Luau plugins against `thurbox.d.luau`.
#
# The plugins `require("@thurbox")`, which is how the Rust host resolves the
# module namespace at runtime. The `luau-analyze` builds we target do not
# support `@alias` require resolution, so this script type-checks a *copy* with
# only the module specifier rewritten to a relative path. Nothing else about the
# source is touched, and `plugin::runtime` carries a test asserting the two
# specifiers stay in sync, so the rewrite cannot silently drift.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUNDLED="$ROOT/src/plugin/bundled"

if ! command -v luau-analyze >/dev/null 2>&1; then
    echo "luau-analyze not found — skipping Luau type check." >&2
    echo "Install it from https://github.com/luau-lang/luau/releases" >&2
    exit 0
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cp "$BUNDLED/thurbox.d.luau" "$WORK/thurbox.d.luau"

status=0
found=0
while IFS= read -r -d '' plugin; do
    found=1
    name="$(basename "$(dirname "$plugin")")"
    sed 's|require("@thurbox")|require("./thurbox.d")|' "$plugin" > "$WORK/$name.luau"
    if ! luau-analyze --mode=strict "$WORK/$name.luau"; then
        echo "Luau type check failed for bundled plugin '$name'" >&2
        status=1
    fi
done < <(find "$BUNDLED" -mindepth 2 -name '*.luau' -print0)

if [ "$found" -eq 0 ]; then
    echo "No bundled Luau plugins found under $BUNDLED" >&2
    exit 1
fi

luau-analyze --mode=strict "$WORK/thurbox.d.luau" || status=1
exit "$status"
