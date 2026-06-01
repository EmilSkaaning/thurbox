#!/usr/bin/env bash
# Tear down the local SSH test target. Pass --purge to also drop the image and
# the generated keypair. Uses plain `podman`/`docker` (no compose required).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
IMAGE="thurbox-test-sshd"
NAME="thurbox-test-sshd"

if command -v podman >/dev/null 2>&1; then
    ENGINE=podman
elif command -v docker >/dev/null 2>&1; then
    ENGINE=docker
else
    echo "error: need podman or docker on PATH" >&2
    exit 1
fi

"$ENGINE" rm -f "$NAME" >/dev/null 2>&1 || true

# Drop the staged authorized_keys from the build context regardless.
rm -f "$HERE/authorized_keys"

if [ "${1:-}" = "--purge" ]; then
    "$ENGINE" rmi -f "$IMAGE" >/dev/null 2>&1 || true
    rm -rf "$HERE/.keys"
    echo "purged image and test keypair." >&2
else
    echo "stopped $NAME (image kept; use --purge to remove it)." >&2
fi
