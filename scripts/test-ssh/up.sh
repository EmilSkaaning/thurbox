#!/usr/bin/env bash
# Bring up the local SSH test target (sshd + tmux + git in a container) and
# print the environment exports needed to run the `ssh_integration` test.
#
# Usage:
#   ./scripts/test-ssh/up.sh
#   eval "$(./scripts/test-ssh/up.sh --export)"   # set vars in the current shell
#
# Idempotent: reuses an existing keypair, rebuilds the image, and replaces the
# container. Uses plain `podman`/`docker` build+run (no compose required).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
KEY_DIR="$HERE/.keys"
KEY="$KEY_DIR/id_ed25519"
PORT="${THURBOX_SSH_PORT:-2222}"
IMAGE="thurbox-test-sshd"
NAME="thurbox-test-sshd"

# Pick a container engine: podman (preferred) or docker.
if command -v podman >/dev/null 2>&1; then
    ENGINE=podman
elif command -v docker >/dev/null 2>&1; then
    ENGINE=docker
else
    echo "error: need podman or docker on PATH" >&2
    exit 1
fi

# 1. Generate a throwaway keypair (gitignored) if absent.
mkdir -p "$KEY_DIR"
chmod 700 "$KEY_DIR"
if [ ! -f "$KEY" ]; then
    ssh-keygen -t ed25519 -N "" -C "thurbox-test" -f "$KEY" >/dev/null
    echo "generated test keypair at $KEY" >&2
fi

# 2. Stage the public key into the build context for the Dockerfile COPY.
cp "$KEY.pub" "$HERE/authorized_keys"

# 3. Build the image.
echo "building image $IMAGE with $ENGINE ..." >&2
"$ENGINE" build -t "$IMAGE" -f "$HERE/Dockerfile" "$HERE" >&2

# 4. (Re)start the container, bound to localhost only.
"$ENGINE" rm -f "$NAME" >/dev/null 2>&1 || true
echo "starting $NAME on 127.0.0.1:$PORT ..." >&2
"$ENGINE" run -d --name "$NAME" -p "127.0.0.1:$PORT:22" "$IMAGE" >&2

# 5. Wait for sshd to accept the key (up to ~30s).
echo "waiting for sshd ..." >&2
for _ in $(seq 1 30); do
    if ssh -p "$PORT" -i "$KEY" \
        -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o BatchMode=yes \
        -o ConnectTimeout=2 \
        tester@localhost true 2>/dev/null; then
        echo "sshd is ready." >&2
        cat <<EOF >&2

Run the integration test with:

  export THURBOX_SSH_IT=1
  export THURBOX_SSH_DEST=tester@localhost
  export THURBOX_SSH_PORT=$PORT
  export THURBOX_SSH_KEY=$KEY
  cargo nextest run --test ssh_integration --no-capture

(these defaults match the container, so just THURBOX_SSH_IT=1 is enough)
EOF
        # --export prints shell-evalable assignments on stdout.
        if [ "${1:-}" = "--export" ]; then
            echo "export THURBOX_SSH_IT=1"
            echo "export THURBOX_SSH_DEST=tester@localhost"
            echo "export THURBOX_SSH_PORT=$PORT"
            echo "export THURBOX_SSH_KEY=$KEY"
        fi
        exit 0
    fi
    sleep 1
done

echo "error: sshd did not become ready in time; check '$ENGINE logs $NAME'" >&2
exit 1
