#!/usr/bin/env bash
#
# Run a thurbox *dev build* in an isolated sandbox — one command to launch the
# `thurbox-dev` TUI/CLI against a throwaway or persistent environment that never
# touches your real ~/.config/thurbox or tmux server.
#
# By default only *thurbox's own* config/data are redirected (via THURBOX_*_DIR)
# — your real HOME/agents stay intact, so authenticated claude/codex/antigravity work.
# Pass --isolate-home for a fully hermetic env (fresh HOME, agents boot without
# credentials), e.g. to reproduce the demo/smoke conditions.
#
# Usage:
#   scripts/dev/sandbox.sh                 # persistent "default" profile, launch the TUI
#   scripts/dev/sandbox.sh --fresh         # throwaway env, wiped on exit
#   scripts/dev/sandbox.sh --profile foo   # named persistent profile
#   scripts/dev/sandbox.sh --isolate-home  # full isolation (fresh HOME; no agent creds)
#   scripts/dev/sandbox.sh --shell         # drop into a shell with the sandbox env
#                                          #   (run `thurbox-cli ...` against the sandbox DB)
#   scripts/dev/sandbox.sh -- session list # run `thurbox-cli <args>` in the sandbox
#   scripts/dev/sandbox.sh --clean [name]  # kill + wipe a persistent profile, then exit
#
# v2 plugin host (absent from stable builds, so it needs an explicit build):
#   scripts/dev/sandbox.sh --plugins       # build with --features plugins and launch
#   scripts/dev/sandbox.sh --plugins --show tasks,file-viewer
#                                          # …and make those bundled panes visible
#   scripts/dev/sandbox.sh --plugins --show all
#                                          # every bundled pane (see the note below)
#
# The bundled panes that reproduce a native pane ship `default_visible = false`,
# because the native pane is still what draws the interface — a visible copy
# would show two of the same pane. `--show` flips them on through the generated
# `<plugin>.<pane>.show` commands, and the choice persists in the sandbox DB.
# `--show all` turns on six panes at once, which overflows the right-hand column
# on a normal terminal; it is useful for checking that they *load*, less so for
# looking at them. Prefer naming one or two.
#
# State (persistent mode): target/dev-sandbox/<profile>/ (gitignored). Sessions
# survive across runs — its tmux-dev server is left alive on exit. `--clean`
# (or `cargo clean`) removes it.
#
# Requires: cargo, tmux >= 3.2. Build happens before any HOME override so cargo
# still resolves your real ~/.cargo.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
export TBX_REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)" # consumed by the helper
# shellcheck source=scripts/dev/lib/sandbox-env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/sandbox-env.sh"

# Logs go to stderr so `--` CLI passthrough keeps stdout clean (e.g. `--json`).
log() { printf '\033[1;36m==>\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

mode="persistent"
profile="default"
isolation="thurbox" # thurbox | full
action="tui"        # tui | shell | cli | clean
cli_args=()
features=()         # extra cargo --features flags
show_panes=""       # comma-separated plugin names, or "all"

while [ $# -gt 0 ]; do
    case "$1" in
        --fresh) mode="fresh"; shift ;;
        --profile) profile="${2:?--profile needs a name}"; shift 2 ;;
        --isolate-home) isolation="full"; shift ;;
        --plugins) features=(--features plugins); shift ;;
        --show) show_panes="${2:?--show needs a plugin name, a comma-separated list, or 'all'}"; shift 2 ;;
        --shell) action="shell"; shift ;;
        --clean) action="clean"; shift; case "${1:-}" in ""|-*) ;; *) profile="$1"; shift ;; esac ;;
        --) shift; action="cli"; cli_args=("$@"); break ;;
        -h|--help) sed -n '2,32p' "$0"; exit 0 ;;
        *) die "unknown argument: $1 (try --help)" ;;
    esac
done

command -v cargo >/dev/null || die "cargo not found"

if [ "$action" = "clean" ]; then
    log "cleaning sandbox profile '$profile'"
    tbx_sandbox_clean "$profile"
    exit 0
fi

# Build the dev binaries BEFORE the HOME override (so cargo finds ~/.cargo).
if [ -n "$show_panes" ] && [ ${#features[@]} -eq 0 ]; then
    die "--show needs --plugins (the plugin host is absent from a default build)"
fi

log "building thurbox (dev${features[*]+ ${features[*]}})"
( cd "$TBX_REPO_ROOT" && cargo build --bin thurbox --bin thurbox-cli "${features[@]+${features[@]}}" >&2 )

if [ "$isolation" = "full" ]; then
    tbx_sandbox_init_full "$mode" "$profile"
else
    tbx_sandbox_init "$mode" "$profile"
fi
export TBX_IN_SANDBOX="$profile"

log "sandbox root: $TBX_SANDBOX_ROOT ($mode, $isolation isolation)"

# Make the named bundled panes visible, through the same generated commands a
# user or an agent would call — so this exercises the real surface rather than
# writing the visibility rows behind its back.
show_bundled_panes() {
    local cli="$TBX_REPO_ROOT/target/debug/thurbox-cli"
    local wanted="$1" ids id plugin
    # `command list --json` is the source of truth for which panes exist; parsing
    # it means this never drifts from the bundled set.
    ids="$("$cli" command list --json 2>/dev/null \
        | grep -o '"id":"[^"]*\.show"' | cut -d'"' -f4)" || true
    if [ -z "$ids" ]; then
        log "no plugin panes found (is this a --plugins build?)"
        return
    fi
    for id in $ids; do
        plugin="${id%%.*}"
        if [ "$wanted" = "all" ] || [[ ",$wanted," == *",$plugin,"* ]]; then
            if "$cli" command run "$id" >/dev/null 2>&1; then
                log "showing pane: $plugin"
            else
                log "could not show pane: $plugin"
            fi
        fi
    done
}

# What to run in the sandbox env.
run_in_sandbox() {
    [ -n "$show_panes" ] && show_bundled_panes "$show_panes"
    case "$action" in
        shell)
            log "entering sandbox shell — \`thurbox\`/\`thurbox-cli\` target this sandbox; exit to leave"
            "${SHELL:-bash}" -i
            ;;
        cli) "$TBX_REPO_ROOT/target/debug/thurbox-cli" "${cli_args[@]}" ;;
        tui) "$TBX_REPO_ROOT/target/debug/thurbox" ;;
    esac
}

# Run as a child (NOT exec): a `fresh` sandbox needs the teardown trap to fire on
# exit to reap its throwaway dir + tmux server, and `exec` would discard the
# trap. Persistent sandboxes have nothing to tear down.
[ "$TBX_SANDBOX_FRESH" = "1" ] && trap tbx_sandbox_teardown EXIT INT TERM
run_in_sandbox
