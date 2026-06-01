# Local SSH test target

A throwaway container that runs **sshd + tmux + git**, so thurbox's remote
SSH-session code paths can be exercised end-to-end on your own machine — no real
remote host required.

It backs the opt-in integration test [`tests/ssh_integration.rs`](../../tests/ssh_integration.rs),
which drives the *real* code:

- **git over ssh** — `git::*_on` → `ssh <dest> git -C <repo> …`
  (`list_branches_on`, `create_worktree_on`, `remove_worktree_on`)
- **tmux over ssh** — `TmuxBackend::from_host` → `ssh <dest> tmux …` control mode
  (`ensure_ready`, `spawn`, live vt100 capture, `kill`)

## Requirements

- `docker` (with the `compose` plugin), **or** `podman` + `podman-compose`
- `ssh` / `ssh-keygen` on the host

## Usage

```sh
./scripts/test-ssh/up.sh        # generate key, build image, start sshd on 127.0.0.1:2222
THURBOX_SSH_IT=1 cargo nextest run --test ssh_integration --no-capture
./scripts/test-ssh/down.sh      # stop + remove the container
./scripts/test-ssh/down.sh --purge   # also drop the image and the generated keypair
```

`up.sh` is idempotent and prints the exact env exports it set up. To load them
into your current shell:

```sh
eval "$(./scripts/test-ssh/up.sh --export)"
cargo nextest run --test ssh_integration --no-capture
```

## What the container ships

- user `tester`, key-only login (the generated public key is injected at build)
- a seeded git repo at `/home/tester/repo` on branch `main` (one commit)
- a worktrees dir at `/home/tester/worktrees`
- tmux ≥ 3.2 (build fails otherwise — thurbox's minimum)

## Configuration

The test reads these env vars; defaults match this container, so
`THURBOX_SSH_IT=1` alone is enough.

| Env var                 | Default                              |
|-------------------------|--------------------------------------|
| `THURBOX_SSH_IT`        | unset → **test skips**               |
| `THURBOX_SSH_DEST`      | `tester@localhost`                   |
| `THURBOX_SSH_PORT`      | `2222`                               |
| `THURBOX_SSH_KEY`       | `scripts/test-ssh/.keys/id_ed25519`  |
| `THURBOX_SSH_REPO`      | `/home/tester/repo`                  |
| `THURBOX_SSH_WORKTREES` | `/home/tester/worktrees`             |

The same env vars let you point the test at **any** reachable host instead of
the container (e.g. a real box from your `~/.ssh/config`) — set
`THURBOX_SSH_DEST`, drop `THURBOX_SSH_PORT`/`THURBOX_SSH_KEY` if your ssh config
already covers them, and ensure the repo/worktrees paths exist there.

## Notes

- The port binds to `127.0.0.1` only; the image accepts a single generated key
  and is **not** a security boundary — it's a disposable test fixture.
- The generated keypair (`.keys/`) and the staged `authorized_keys` are
  gitignored.
- The test is skipped by default, so `cargo nextest run --all` stays hermetic
  and CI is unaffected unless `THURBOX_SSH_IT=1` is exported.
