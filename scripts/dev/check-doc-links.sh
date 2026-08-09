#!/usr/bin/env bash
# Verify every relative Markdown link in the docs resolves: the target file
# exists, and any #anchor matches a heading (or an explicit <a id="…">) in it.
#
# Broken cross-references are invisible to rumdl (which lints style, not links)
# and to the compiler, so they rot silently as headings get renamed. This is the
# deterministic check for them — Constitution rule 11.
#
# Usage: scripts/dev/check-doc-links.sh [path ...]     (default: docs README.md)
set -euo pipefail

targets=("$@")
if [ "${#targets[@]}" -eq 0 ]; then
    targets=(docs README.md)
fi

# GitHub's heading-slug rule, near enough: lowercase, drop everything that is
# not alphanumeric/space/hyphen/underscore, then spaces to hyphens. Explicit
# <a id="…"> anchors are emitted as-is.
slugs_of() {
    awk '
        function slug(s,   out, i, c) {
            s = tolower(s)
            out = ""
            for (i = 1; i <= length(s); i++) {
                c = substr(s, i, 1)
                if (c ~ /[a-z0-9_-]/) out = out c
                else if (c == " ") out = out "-"
            }
            return out
        }
        /^```/ { inblock = !inblock; next }
        inblock { next }
        /^#+ / { line = $0; sub(/^#+ /, "", line); print slug(line) }
        match($0, /<a id="[^"]+"/) { print substr($0, RSTART + 7, RLENGTH - 8) }
    ' "$1"
}

broken=0

while IFS= read -r -d '' file; do
    dir=$(dirname "$file")
    links=$({ grep -o '](<\?[^)>]*>\?)' "$file" || true; } | sed 's/^](<\?//; s/>\?)$//')

    while IFS= read -r link; do
        case "$link" in
            '' | http://* | https://* | mailto:* | '#') continue ;;
        esac

        path=${link%%#*}
        anchor=""
        case "$link" in *#*) anchor=${link#*#} ;; esac

        if [ -n "$path" ]; then
            resolved="$dir/$path"
        else
            resolved="$file"
        fi

        if [ ! -e "$resolved" ]; then
            echo "$file: missing target -> $link"
            broken=$((broken + 1))
            continue
        fi

        [ -n "$anchor" ] || continue
        case "$resolved" in *.md) ;; *) continue ;; esac

        if ! slugs_of "$resolved" | grep -qxF "$anchor"; then
            echo "$file: bad anchor -> $link"
            broken=$((broken + 1))
        fi
    done <<<"$links"
done < <(find "${targets[@]}" -name '*.md' -not -path '*/node_modules/*' -print0)

if [ "$broken" -gt 0 ]; then
    echo >&2
    echo "doc links: $broken broken reference(s)" >&2
    exit 1
fi

echo "doc links: all references resolve"
