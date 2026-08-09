# Thurbox v2 — Security Model

v2 introduces something v1 does not have: **third-party code running
continuously inside a tool that holds credentials, drives shells on remote
hosts, and is operated by LLM agents**. This document states the threat model,
what the capability system does and does not buy, and the changes required
before third-party plugins are installable.

It is written to be uncomfortable where the design is weak. Two of the findings
it once recorded have since been closed by
[ADR-V2](ARCHITECTURE.md#adr-v2) — §3 and §4 keep the history, because a
capability model that was once decorative is worth remembering as one.

---

## 1. What capabilities are

Capabilities are an **informed-consent and least-privilege** mechanism. They
are not a sandbox, and the docs say so — but the phrasing has been doing more
work than the mechanism can support, which §3 corrects.

| They do | They do not |
|---|---|
| Make a plugin declare its reach before it runs | Confine a plugin that lies or is compromised |
| Let the installer show that reach to a human | Prevent a granted capability being abused |
| Bind only the host functions a plugin declared | Make a granted capability safe to hand out |
| Keep an unprivileged pane genuinely unprivileged | Make `shell`, `pty`, or broad `fs` anything less than full trust |

The honest summary: **all sixteen capabilities are enforced** — §3. What they
do not do is make a *granted* capability safe.

---

## 2. Trust boundaries and assets

```text
┌─ user's account ───────────────────────────────────────────────┐
│                                                                │
│  thurbox kernel ──calls──▶ plugin VM + thread (per plugin)     │
│      │                        ▲                                │
│      │                        └── same UID and process; reaches │
│      │                            env/fs/net only via bindings  │
│      │                                                         │
│      ├──▶ tmux ──▶ agent CLIs (hold provider credentials)      │
│      ├──▶ ssh ──▶ remote hosts (worktrees, shells)             │
│      ├──▶ SQLite (sessions, tasks, messages, plugin data)      │
│      └──▶ control socket ◀── thurbox-cli, agents, any local    │
│                              process that can open it          │
└────────────────────────────────────────────────────────────────┘
```

**Assets worth protecting**, in rough order of severity:

1. **Provider credentials** — `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
   `GITHUB_TOKEN`, `~/.claude`, `~/.codex`, ssh keys. thurbox's whole purpose
   is running CLIs that authenticate.
2. **Source code** across every repo the user has a session in.
3. **Remote hosts** — a `sessions = control` plugin can spawn shells on any
   host in `hosts.toml`.
4. **The user's git history** — a plugin with `shell` can force-push.
5. Session/task/message state in SQLite.

**Adversaries**, in rough order of likelihood:

| Adversary | Vector |
|---|---|
| A careless plugin | Overbroad capabilities, leaked secrets in logs |
| A compromised vendored module | A plugin vendors a Luau module it does not audit |
| A malicious plugin | Published to look useful |
| **Prompt injection via an agent** | Untrusted text (PR comment, issue body, CI log) instructing an agent to invoke a destructive command |
| A hostile local process | Connecting to the control socket |

The fourth is not hypothetical for this project: thurbox's own PR-watching
workflows feed GitHub comment bodies to agents, and `docs/v2` makes every
plugin command agent-callable.

---

## 3. Resolved — `fs`, `net` and `shell` are now enforced

**This section previously recorded a defect.** An earlier draft of
[FEATURES-Plugin-API.md §7](FEATURES-Plugin-API.md#7-host-api) claimed `fs`
was "enforced on the runtime process itself", which was not implementable on
Bun — no shipping JavaScript runtime except Deno has a permission model, so
`fs`, `net` and `shell` were declared but unenforceable, and three of sixteen
capabilities were theatre.

[ADR-V2](ARCHITECTURE.md#adr-v2) closes it. Under Luau the enforcement is not
a check the kernel performs — it is an **absence**:

| Capability | How it is enforced |
|---|---|
| `fs` | There is no `io` table in the plugin's environment unless the kernel binds one, scoped to the granted paths |
| `net` | No socket or HTTP library exists unless `ctx.fetch` is bound, and it is bound with the host allowlist baked in |
| `shell` | No `os.execute`, no `io.popen`. `ctx.exec` exists only when granted |
| `require` | Not bound. Module resolution is the kernel's, so a plugin cannot reach outside its own directory |

All sixteen capabilities are now enforced, and
[N3](CONSTITUTION-DELTA.md#n3--capabilities-are-declared-gated-and-shown) is
restored to its full form rather than narrowed to "gated where gateable".

### What this does *not* make it

A sandbox is not a proof. Two honest caveats:

- **Lua sandbox escapes are a real class**, historically via metatables, the
  string metatable, `debug`, and coroutine tricks. Luau is hardened against
  these — read-only tables, `safeenv`, no `debug` by default, no
  `setfenv`/`getfenv` — and Roblox's business depends on the hardening holding
  against a far more adversarial population than thurbox will ever have. But
  "hardened by a company with strong incentives" is a different claim from
  "proven", and the design should not pretend otherwise.
- **A granted capability is still full trust in its domain.** `shell = true`
  is arbitrary code execution; `fs` over `{repo}` can read every secret in the
  repo. Capabilities constrain reach, not intent.

### What replaces the old open decision

The Bun-versus-Deno choice this section used to leave open is moot — there is
no JavaScript runtime. The remaining runtime risk is different in kind: a
memory-safety bug in the Luau VM itself would be in-process, and
[C2](ARCHITECTURE.md#adr-v2) (no native code in plugins) is what keeps that
surface to the VM rather than to every plugin's dependency tree.

---

## 4. Plugin environment

Out of process, the danger was inheritance: a child process gets the parent's
environment by default, and thurbox's holds `ANTHROPIC_API_KEY`,
`GITHUB_TOKEN` and similar. A plugin with no capabilities at all could have
read them from `process.env`.

In-process there is **no ambient environment to inherit**. A Luau VM has no
`os.getenv` unless the kernel binds one, so the exposure is opt-in rather than
opt-out — the inverse of the process model's default, and strictly safer.

**Required, and now trivial**:

- Do not bind `os.getenv` at all. A plugin needing configuration reads a
  declared setting or a path granted under `fs`.
- The `THURBOX_*` identity vars a plugin legitimately needs (`THURBOX_SESSION`,
  `THURBOX_TASK`) are exposed as typed fields on `ctx`, not as environment
  lookups.
- `ctx.exec`, when granted, spawns children with a **scrubbed environment** —
  an explicit allowlist plus `THURBOX_*`. The inheritance hazard moves from
  the plugin to the processes a plugin spawns, and that is where it must be
  handled.

An earlier draft of this document claimed in-process would make environment
scrubbing *harder*. That was true of an embedded JavaScript engine with Node
compatibility, and false of Luau, where the binding simply does not exist.

---

## 5. The control socket

[FEATURES-Agent-API.md §6](FEATURES-Agent-API.md#6-control-socket) places a control socket at
`$XDG_RUNTIME_DIR/thurbox/control.sock` exposing `command/run`,
`event/subscribe`, and `state/query` — enough to create sessions, drive panes,
and run every plugin command. Its permissions are unspecified.

**Required**:

- Socket mode `0600`, in a directory the kernel creates `0700`, with an
  ownership check on connect. `$XDG_RUNTIME_DIR` is usually already
  user-private; when it is unset the fallback path must not be world-writable
  `/tmp`.
- On Windows, a named pipe with an explicit DACL granting the current user
  only — the default is more permissive than people expect.
- Reject connections whose peer UID differs from the kernel's (`SO_PEERCRED` /
  `LOCAL_PEERCRED`).
- The socket exists only while the TUI runs, and is unlinked on shutdown and on
  the panic path.

Without this, any local process — including a compromised dependency in an
unrelated project — can drive the user's thurbox.

---

## 6. Supply chain

v1 pinned official extensions to the binary's release tag, which was a real
integrity property. v2's `plugin install <name|url|path>` has no stated
equivalent.

**Required before third-party plugins are installable**:

| Control | Why |
|---|---|
| A lockfile recording resolved version **and content hash** per plugin and per dependency | `plugin update` must be able to detect substitution, not just a version change |
| Official plugins pinned to the binary's release tag, as v1 did | Keeps the fetched plugin matched to the binary |
| `plugin install` prints the capability set and prompts on anything beyond `db` | Already specified; it is the consent moment |
| Deterministic re-resolution — no floating ranges at install time | A reinstall must produce the same bytes |
| License allowlist for bundled plugins in CI | Already specified in the Rule 4/5 amendment. There is no package manager to audit ([ADR-V2](ARCHITECTURE.md#adr-v2)), so the whole supply chain is the plugin's own source |

Signature verification is **not** proposed for 2.0.0: it needs a key
distribution story thurbox does not have, and a lockfile with content hashes
gets most of the benefit.

---

## 7. Agent-driven execution and prompt injection

[ADR-V10](ARCHITECTURE.md#adr-v10) makes every plugin command agent-callable,
and thurbox agents routinely read untrusted text. The existing controls are
sound as far as they go: per-command `agent_policy` (`allow`/`confirm`/`deny`)
and a depth-3 loop guard.

What is missing is the **threat being named** and the **defaults being derived
from it**:

- A command that deletes, force-pushes, installs, or spawns a process defaults
  to `confirm` or `deny`. A plugin opts *down* explicitly, and `plugin doctor`
  reports every command that did.
- `plugin.install`, `plugin enable`, and anything mutating capabilities are
  `deny` — permanently. An agent must never be able to widen its own reach.
- `thurbox-cli command list` already reports policy, so an agent can tell in
  advance which calls block on a human. That is the property to preserve.

The realistic attack is not an agent turning malicious; it is an agent
faithfully following instructions embedded in a PR comment it was asked to
summarize. Confirmation prompts are the control, and they only work if the
dangerous defaults are conservative from the start.

---

## 8. Spawn contributions

[FEATURES-Backend-API.md §11](FEATURES-Backend-API.md#11-spawn-contributions) lets a plugin append
env vars and args to spawned agent sessions. Append-only and veto-free bounds
*who wins*, not *what can be injected*.

Appending `LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`, `GIT_SSH_COMMAND`,
`GIT_EXTERNAL_DIFF`, `BASH_ENV`, `NODE_OPTIONS`, or `PATH` turns a spawn
contribution into arbitrary code execution inside every agent session.

**Required**: a denylist of env keys a contribution may not set, plus a rule
that `PATH` may only be prepended with paths under `{plugin_data}`. Rejections
are logged and surfaced in `plugin doctor` rather than silently dropped.

---

## 9. Remote blast radius

A plugin with `sessions = "control"` can spawn on any host in `hosts.toml`.
The reach is therefore not bounded by the local machine, and neither the
install prompt nor the capability table says so.

**Required**: `sessions = "control"` is described at install as granting
*local and remote* session creation, and `plugin doctor` lists which hosts are
reachable. A finer `sessions = "control-local"` is worth considering but is
not proposed for 2.0.0 — one more axis for a distinction most plugins will not
use.

---

## 10. What is already sound

Worth stating, so the findings above are read in proportion:

- **VM isolation** ([ADR-V4](ARCHITECTURE.md#adr-v4)) genuinely contains
  errors, hangs and runaway memory: an uncaught error unwinds one VM, the
  interrupt handler stops a spinning one, and `set_memory_limit` caps its
  allocation. What it does *not* contain is memory corruption, which is why
  [C2](ARCHITECTURE.md#adr-v2) forbids native code in a plugin.
- **The kernel renders**
  ([N1](CONSTITUTION-DELTA.md#n1--the-kernel-renders-plugins-describe)) —
  view-tree text is escape-stripped when the tree is converted at push time, so
  a plugin cannot emit OSC 52 to steal the clipboard or rewrite the user's
  terminal title. The `surface` carve-out
  terminates escapes in the kernel's own parser.
- **No raw SQL handle** ([ADR-V17](ARCHITECTURE.md#adr-v17)) — namespace
  rewriting means a plugin cannot read the sessions table or another plugin's
  data.
- **Default-deny capabilities** — a plugin that declares nothing can draw and
  nothing else.
- **`pty` is correctly classified** as arbitrary code execution rather than a
  weaker `shell`.
- **Stdout is the protocol**, so a plugin cannot smuggle escape sequences to
  the terminal through logging.

---

## 11. Required before third-party plugins are installable

Ordered by severity. The first two are gating.

| # | Change | Phase |
|---|---|---|
| 1 | No `os.getenv` binding; `ctx.exec` children get a scrubbed environment (§4) | 1 |
| 2 | ~~Resolve the enforcement gap~~ — closed by [ADR-V2](ARCHITECTURE.md#adr-v2) (§3) | done |
| 3 | Control-socket permissions, peer-UID check, Windows DACL (§5) | 5 |
| 4 | Spawn-contribution env denylist (§8) | 2 |
| 5 | Conservative `agent_policy` defaults for destructive verbs (§7) | 5 |
| 6 | Lockfile with content hashes; official plugins tag-pinned (§6) | before a public registry |
| 7 | Install prompt names remote reach for `sessions = control` (§9) | 1 |

Items 1, 3, 4, and 7 are each small and testable. Item 2 is a decision, not an
implementation, and item 6 is a prerequisite for a surface (a public registry)
that 2.0.0 does not ship.

---

## 12. Non-goals

- **A real sandbox for plugins on Bun.** Rejected as out of scope for 2.0.0
  (§3 option C).
- **Signature verification.** No key distribution story yet (§6).
- **Defending against a malicious *kernel* build.** If thurbox itself is
  compromised, nothing here helps.
- **Multi-user isolation.** thurbox is single-user; the control socket is
  local and owner-only by §5, and that is the whole model.
- **Protecting the user from their own agents.** An agent with a shell in a
  worktree can do what the user can. Confirmation prompts reduce accidents,
  not authority.
