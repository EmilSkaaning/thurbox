# Architecture Decisions

Each decision follows a mini-ADR format:
**Choice**, **Why**, **Rejected alternatives**.

---

## ADR-1: The Elm Architecture (TEA)

**Choice**: All state lives in a single `App` model.
Events become messages, `update()` applies them,
`view()` renders the result.

**Why**: TEA makes state transitions explicit and testable.
Every input has a traceable path from event to screen change.
There's no hidden state scattered across components, which matters
when multiple PTY sessions are producing concurrent output.

**Rejected**:

- *Component-based (each panel owns state)* — leads to
  synchronization bugs when sessions interact.
- *Ad-hoc event handlers* — untraceable control flow;
  hard to reason about as the app grows.

---

## ADR-2: Session pipeline — SessionBackend + vt100 + tui-term

**Choice**: A `SessionBackend` trait abstracts session lifecycle
(spawn, adopt, resize, kill, detach, discover). Each session runs
one coding-agent CLI inside the backend. The default backend is
local tmux (`tmux -L thurbox`); the same `TmuxBackend` also runs
over SSH for remote hosts (ADR-13).
`vt100::Parser` interprets escape sequences,
`tui_term::PseudoTerminal` renders the parsed screen into ratatui.

**Why**: The trait-based design keeps the session transport
behind a clean boundary so the app layer never touches tmux
directly. tmux provides truly persistent sessions
that survive thurbox crashes/restarts, multiple thurbox instances
share the same running sessions, and external recovery is
possible via `tmux -L thurbox attach`.

**Previous design**: `portable-pty` spawned the agent CLI
directly. Sessions died when thurbox exited, terminal content was
lost on restart, and multiple instances had no coordination.

**Rejected**:

- *`portable-pty` (previous)* — no session persistence,
  no multi-instance sharing, terminal content lost on restart.
- *`alacritty_terminal`* — full terminal emulator,
  far heavier than needed.
- *Parsing raw ANSI ourselves* — error-prone,
  massive surface area, already solved by `vt100`.

---

## ADR-3: Async — tokio multi-threaded + spawn_blocking

**Choice**: The app runs on tokio's multi-threaded runtime.
PTY read loops run inside `spawn_blocking`
(blocking I/O in a threadpool), while PTY write and event handling
run in `tokio::spawn` (async).

**Why**: PTY reads are blocking by nature
(`read()` on a file descriptor). Putting them in `spawn_blocking`
prevents stalling the async executor. The writer side is naturally
async — it awaits messages from an mpsc channel
and writes when they arrive.

**Generalized off-the-hot-path pattern**: the same
`spawn_blocking` → `mpsc` → poll-in-`tick()` shape keeps every other
blocking side effect off the UI thread, so neither rendering nor
`Ctrl+N` ever freezes. Each operation owns an in-flight guard + result
receiver on `App`, kicks off the blocking work, and applies the result
when `tick()` polls `try_recv()`:

- **Worktree sync** (`Ctrl+S`) — `git rebase` per worktree
  (`worktree_sync_rx`, the original instance of the pattern).
- **Per-tick metrics** — `refresh_system_metrics` (sysinfo + statusline
  file reads + the active pane's PID lookup) and `refresh_active_git_stats`
  (`git` diff/status shell-outs). The `sysinfo::System` is *moved into*
  the worker and returned with the result so CPU deltas persist across
  refreshes; a single in-flight guard prevents overlap.
- **Interactive spawn** — `git worktree add` (`spawn_worktree_session`)
  and `Session::spawn` (PTY/tmux window creation, 500 ms+) for the
  new-session wizard run on blocking tasks, with the follow-up
  (session adoption, task-prompt delivery) carried in a `Pending*`
  continuation applied on completion. Programmatic spawns
  (automations/tasks, restore) stay **synchronous** — they read the new
  session's id straight back, so they cannot defer it to a later tick.

**Rejected**:

- *Single-threaded tokio* — PTY reads would block the entire
  runtime, freezing the UI.
- *`std::thread` for everything* — works but loses tokio's
  structured concurrency, select!, and channel ergonomics.

---

## ADR-4: Input translation — crossterm KeyCode to xterm ANSI

**Choice**: `input.rs` maps crossterm `KeyCode`/`KeyModifiers`
to raw xterm ANSI byte sequences before writing to the PTY.

**Why**: crossterm gives us structured key events.
PTYs expect raw bytes. The translation layer is explicit and
testable — each key has a known byte sequence, and edge cases
(arrow keys, function keys, modifier combos)
are handled in one place.

**Rejected**:

- *Raw passthrough (forward crossterm's raw bytes)* —
  crossterm's internal byte representation doesn't match xterm
  sequences. Modifier keys, in particular, would break.

---

## ADR-5: Responsive layout breakpoints

**Choice**: Three layout tiers based on terminal width:

- `<80 cols` — terminal panel only (full screen)
- `>=80 cols` — two panels (left panel + terminal)
- `>=120 cols` — three panels (left panel + terminal + info)

The left panel is a single session list.

**Why**: 80 columns is the smallest usable terminal width. Below
that, showing a sidebar wastes too much space. At 120+, there's
room for supplementary info without shrinking the terminal panel
below readable width. Fixed breakpoints are predictable — the
layout never "jitters" near a threshold.

**Rejected**:

- *Fixed layout (always 3 panels)* — unusable on small terminals.
- *User-configurable breakpoints* — premature complexity.
  Can be added later if needed.

---

## ADR-6: File-based logging only

**Choice**: All tracing output goes to
`~/.local/share/thurbox/thurbox.log`.
Nothing writes to stdout or stderr.

**Why**: The TUI owns stdout entirely. Any stray `println!` or
log line to stdout would corrupt the terminal display. File-based
logging also makes it easy to `tail -f` the log in a second
terminal while developing.

**Rejected**:

- *Stderr logging* — crossterm's alternate screen captures stderr
  on some platforms, still risks display corruption.
- *In-app log panel* — useful eventually, but adds complexity
  before the core features are stable.

---

## ADR-7: Build profiles

| Profile | `opt-level` | LTO | Strip | Debug | Use case |
|---|---|---|---|---|---|
| `dev` | 0 | off | no | yes | Fast iteration |
| `test` | 1 | off | no | yes | Faster tests, still debuggable |
| `release` | 3 | full | yes | no | Distribution binary |
| `release-with-debug` | 3 | full | no | yes | Profiling / flamegraph |

**Why**: `test` at opt-level 1 catches optimization-dependent bugs
earlier while keeping compile times reasonable. The release profile
strips everything for a minimal binary. `release-with-debug` exists
specifically for `perf` / `flamegraph` workflows.

---

## ADR-8: State storage — SQLite

**Choice**: All persistent state (sessions, worktrees,
automations) is stored in a single SQLite
database at `~/.local/share/thurbox/thurbox.db` (respects
`$XDG_DATA_HOME`). WAL mode enables concurrent multi-instance
access. Agent definitions are the one exception: they live in a
human-editable TOML file (see ADR-19), not the database.

*This supersedes the original TOML file-based approach
(`~/.config/thurbox/config.toml`), which was eliminated after
the SQLite migration.*

**Why**: SQLite provides atomic transactions, concurrent access
via WAL mode, and a single source of truth. Multi-instance sync
uses `PRAGMA data_version` polling (see ADR-7b). The TUI provides
all editing UI — there is no need for a human-editable config file.

Every connection sets a **5 s busy_timeout** (the DB is shared by
the TUI, `thurbox-cli`, and the automation heartbeat; writes are
short single-row upserts, so a bounded wait beats an immediate
`SQLITE_BUSY` error or an unbounded freeze) plus the WAL-friendly
performance pragmas `synchronous = NORMAL`, `cache_size`, `mmap_size`,
and `temp_store = MEMORY` (`storage::schema::initialize`; rationale in
`docs/PERFORMANCE.md` ADR-P6). The append-only
**audit log is pruned to 90 days** on `Database::open` — entries
are debugging breadcrumbs, not compliance data, and unbounded
growth would bloat the database over months of use.

**Rejected**:

- *TOML config file (previous)* — race conditions when multiple
  instances write concurrently; split source of truth between
  config.toml and state files (sessions); no atomic multi-key
  updates. (Agent definitions are read-mostly and not subject to
  concurrent writes, so they remain in TOML — see ADR-19.)
- *JSON* — verbose for config, no atomic writes without
  temp-file-rename pattern.
- *CLI flags only* — doesn't scale to multiple sessions and
  long-lived configuration.
- *Embedded in CLAUDE.md* — mixes repo-specific AI guidance with
  application configuration; wrong separation of concerns.

---

## ADR-8b: Automations fire with or without the TUI

**Choice**: Automations fire from three places that all funnel
through one headless entry point, `thurbox-cli automation tick`:
the TUI tick loop, a detached **tmux heartbeat keeper** window
(`automation-heartbeat`, armed on TUI startup and on `automation
create`, looping `tick` every 60 s), and optional systemd/launchd
units (`packaging/`) for reboot-proof firing. Concurrency is made
safe by **claim-based firing** — `Database::claim_due_automation`
advances `next_run_at` with an atomic compare-and-swap, so exactly
one firer wins per due automation.

**Why**: The previous one-shot "scheduled command" fired even with
the TUI shut down by riding tmux's `run-shell` timers; the new
model must keep that durability for recurring + spawn automations.
A live keeper window both runs the heartbeat and keeps the tmux
server alive (a bare pending `run-shell` job does not), so even
spawn-only automations fire with no other sessions. Claim-first
ordering gives at-most-once semantics (a crash loses a run rather
than double-firing), the right default for agent prompts. tmux is
local-only; the send/spawn dispatch sits behind a seam so a future
remote/SSH `SessionBackend` (ADR-2) slots in without changing the
scheduler.

**Rejected**:

- *Per-automation `run-shell` timers (old style)* — precise to the
  second but require bookkeeping + re-arming N timers on startup; a
  single polling keeper is simpler and naturally handles
  create/edit/delete.
- *A bespoke long-running daemon* — duplicates what tmux (already
  required) and systemd/launchd provide; more moving parts.

---

## ADR-9: Flat session list (no project grouping)

**Choice**: The sidebar is a single flat list of sessions. There
is no "project" layer above sessions: each session picks its own
agent and repo selection at creation time.

**Why**: Earlier versions grouped sessions under projects (one
project → many sessions, with shared repos). In practice users
created one session per task, so the project layer was pure
overhead — an extra navigation level, an extra creation step, and
an extra deletion guard. Storage migration v16 dropped the
`projects`, `project_repos`, `project_vm_config`, and
`project_container_config` tables and removed `project_id` columns
from `sessions`, `vms`, and `containers`.

**Rejected**:

- *Two-section sidebar (projects on top, sessions on bottom)* —
  the previous design. Cost a navigation level and a creation
  step for no gain in the typical one-session-per-task workflow.
- *Modal/popup project selector* — hides context while working,
  forces re-opening to switch.
- *Tabs for projects* — horizontal tabs consume vertical space
  and don't scale well past 4-5 entries.

---

## ADR-11: Trait-based session backends

**Choice**: Session lifecycle is abstracted behind a
`SessionBackend` trait (`src/agent/backend.rs`). The `Session`
struct wraps the trait and manages reader/writer loops once,
regardless of which backend is active.

**Why**: Keeping session lifecycle behind a trait boundary leaves
the app layer completely backend-agnostic. The backends today are
local tmux and one SSH backend per configured host (both
`TmuxBackend` over a `TmuxTransport`; see ADR-13), and the seam means
the transport can evolve without touching `App`, `Session`, or any UI
code.

**Trait methods**: `check_available`, `ensure_ready`, `spawn`,
`adopt`, `discover`, `resize`, `is_dead`, `kill`, `detach`.

**Key design decisions**:

- `spawn()` returns `(backend_id, output_reader, input_writer)`.
  The `Session` struct owns the reader/writer loops.
- `adopt()` reconnects to an existing session and returns initial
  screen content for parser seeding.
- `discover()` lists existing sessions for restore-on-startup.
- `detach()` stops streaming without killing the session.
- `kill()` permanently destroys the session.

**Rejected**:

- *Async trait methods* — added complexity for no benefit since
  the tmux backend uses synchronous `Command::new("tmux")`.
  Can be added via `async-trait` if a future backend needs it.

---

## ADR-12: Local tmux as default backend

**Choice**: The default `SessionBackend` is `TmuxBackend`
parameterized over its `Local` transport (`TmuxTransport::Local`)
and registered as `local-tmux`, using a dedicated tmux server
(`tmux -L thurbox`) with session name `thurbox`. All I/O goes
through tmux control mode (`-C`). (The transport abstraction that
also enables remote SSH backends is ADR-13; here the choice is
simply that the out-of-the-box backend runs tmux locally.)

**Why**: tmux provides session persistence (survives crashes),
multi-instance support (multiple thurbox processes can independently
interact with the same sessions), and external recovery
(`tmux -L thurbox attach`). It handles terminal capability queries
(DA1/DA2) natively via `extended-keys on`, eliminating the need for
thurbox to intercept and respond to these sequences.

Control mode (`-C`) supports multiple concurrent client connections,
each receiving independent output streams. Each thurbox instance
establishes its own control mode connection, allowing all instances
to simultaneously monitor and interact with the same tmux sessions.
Output arrives as `%output` notifications (octal-encoded), input is
sent via `send-keys -H` (hex-encoded). This eliminates the previous
`pipe-pane` + FIFO approach which suffered from tmux data-loss
bugs (#641, #2989), required 3 external deps in the data path
(`mkfifo`, `stdbuf`, `cat`), and had no flow control.

**Configuration on init**:

- `remain-on-exit on` — keeps panes alive after process exit
- `status off` — no tmux status bar (thurbox renders its own)
- `default-terminal xterm-256color` — standard terminal type
- `history-limit 5000` — reasonable scrollback
- `extended-keys on` — enhanced key reporting
- `extended-keys-format csi-u` — the modern, unambiguous format some agents
  (e.g. `pi`) probe for at startup; thurbox injects keys via `send-keys` so this
  only sets the reported format, not the bytes agents receive. Best-effort: the
  option is tmux 3.3+ while thurbox's floor is 3.2, so a 3.2 host silently skips it
- `window-size manual` — windows size independently
- `pause-after 5` — flow control (auto-resumed by reader)

**Window naming**: `tb-<session-name>` prefix for discovery.

**Output streaming**: `%output` notifications from control mode,
demultiplexed by pane ID into per-pane broadcast channels. Multiple
instances can simultaneously register the same pane; output is
broadcast to all registered channels via `HashMap<String, Vec<SyncSender>>`.
Each channel feeds a `ControlModeReader` (implements `Read`) consumed
by the existing `Session::reader_loop`. This allows multiple instances
to independently parse and render terminal state in real-time.

**Input**: `send-keys -H <hex>` through the shared control mode
stdin, wrapped in a `ControlModeWriter` (implements `Write`).

**Command synchronization**: All commands that precede a
`send_command` (waited) call must themselves be waited. A
fire-and-forget (`send_command_nowait`) leaves an unclaimed
`%begin`/`%end` response in the stream that can steal the next
waiter. `send_command_nowait` is only safe when nothing follows
(e.g., `detach`) or when issued from the reader thread itself
(e.g., pause resume).

**Session restore**: On reconnect (`TmuxBackend::adopt`),
`capture-pane -e -p -J -S -<scrollback_lines>` seeds the fresh
vt100 parser with the pane's scrollback history **and** visible
screen (text + colors; `-J` rejoins wrapped lines so they re-wrap
at the new width). Without this seed the parser starts empty and
a session's pre-restart history cannot be scrolled in the UI —
the `%output` stream only carries bytes emitted after connect. A
forced resize then triggers SIGWINCH, causing the TUI application
to repaint its visible screen through the normal `%output` stream
— this delivers pixel-perfect rendering of the live region on top
of the seeded history. Seeding is best-effort: a failed capture
logs a warning and adoption proceeds with an empty seed.

**Rejected**:

- *`pipe-pane` + FIFO (previous)* — intermittent data loss from
  tmux bugs #641/#2989, required `mkfifo`/`stdbuf`/`cat` in the
  data path, no flow control, timing race on initial capture.
- *Screen/dtach* — less widely available, fewer features.

---

## ADR-13: Off-local sessions via an SSH / WSL tmux transport

**Choice**: Run agent sessions on a remote host (over SSH) or in a
local WSL distro (via `wsl.exe`) by launching the same tmux
control-mode protocol behind a launch prefix. `LocalTmuxBackend` is
generalized into `TmuxBackend { transport, socket, session, name }`
where `transport: TmuxTransport` is `Local` (a bare
`Command::new("tmux")`), `Ssh { destination, ssh_opts, mux }`
(`ssh <dest> <mux> …`), or `Wsl { distro, mux }`
(`wsl.exe -d <distro> <mux> …`). `mux` is the host multiplexer binary
(`tmux` by default, or `psmux` for a Windows SSH host; a WSL distro
runs `tmux`). The transport's *only* job is to build the `Command`;
everything downstream — the control-mode reader/writer threads, pane
registration, `send-keys`/`%output` — is byte-for-byte identical
(`control_mode.rs` was already transport-agnostic). The SSH and WSL
arms share `TmuxTransport::prefixed`, since both join + shell-interpret
the trailing POSIX-quoted tokens identically; only the launcher prefix
differs.

Hosts are declared as data in `~/.config/thurbox/hosts.toml`
(`session::HostDef { kind: HostKind {Ssh, Wsl}, … }`/`HostRegistry`),
and WSL distros are additionally **auto-discovered** on Windows
(`agent::host_config::discover_wsl_hosts` via `wsl.exe -l -q`). The
combined set is loaded by `agent::host_config::load_all`, each
registered as a backend named `ssh:<host>` / `wsl:<distro>` via
`TmuxBackend::from_host`.

**Why WSL = "SSH without the ssh"**: `wsl.exe` runs `tmux`, `git`, the
agent, and the worktrees all *inside* the distro at native Linux paths,
so there's no Windows↔Linux path translation (`wslpath`) and the
worktree layout matches the SSH path exactly. Modeling WSL as a host
kind (rather than a per-session "run in WSL" flag wrapping a native
psmux pane) reuses the entire remote-host subsystem — picker,
persistence/restore, `git::*_on`, headless `--host` — for free.

**Why** (general): The local-vs-off-local difference is exactly one
line (how the tmux process is launched). The per-session control
commands travel over the stdin pipe, not the launcher argv, so only the
one-time `attach-session` launch crosses the boundary. SSH relies on
the system `ssh` binary + `~/.ssh/config` for auth/keys/multiplexing;
WSL needs no credentials at all.

**Key design decisions**:

- **Lazy registration**: off-local backends are registered but *not*
  connected at startup (`check_available`/`ensure_ready` deferred to
  first use), so a down host (or slow WSL discovery) never blocks the
  TUI. `App::select_backend` only resolves the backend from the
  registry; the blocking `ensure_backend_ready` runs on the spawn
  worker, never on the UI thread (ADR-P12).
- **Auto-discovery**: WSL distros appear with zero config; an explicit
  `kind = "wsl"` entry of the same name wins (for overrides like
  `worktrees_dir`). `discover_wsl_hosts` decodes `wsl.exe`'s UTF-16LE
  output and is a no-op off Windows / without `wsl.exe`.
- **Selection**: `SessionConfig.backend` (`ssh:<host>` / `wsl:<distro>`
  or `None`); `is_remote_backend` covers both. The TUI shows a host
  picker as the first new-session step (skipped when none configured/
  discovered); `thurbox-cli session create --host` is the headless
  equivalent.
- **Persistence/restore**: `backend_type` round-trips in SQLite;
  restore discovers windows **per backend** so off-local sessions
  re-adopt against their own host's tmux.
- **Off-local worktrees**: `git::*_on(host, …)` run git via
  `git::host_launcher` (`ssh …` or `wsl.exe …`). Worktree paths resolve
  under the host's `worktrees_dir` (or `$HOME/.local/share/thurbox/…`
  resolved + cached, keyed by backend name since a WSL host has no
  `destination`).

**Module placement**: `HostDef`/`HostRegistry`/`HostKind` live in
`session/` (the dependency sink) so both `agent` (builds the backend)
and `git` (runs git on the host) can depend on them without violating
the module-isolation rules.

**Riskiest area**: SSH reconnect on a flapping link — `reconnect_control`
reopens the ssh connection; ControlMaster + keepalives mitigate
stalls. Worth the most manual testing.

**Rejected**:

- *A `TmuxTransport` trait with `Box<dyn>`* — an enum with two
  variants is simpler; promote to a trait only if a third transport
  (e.g. container exec) appears.
- *Embedded SSH library (russh, etc.)* — reimplements `~/.ssh/config`,
  agent forwarding, and multiplexing that the system `ssh` already
  provides.

---

## ADR-7b: Multi-Instance Sync — SQLite with PRAGMA data_version

**Choice**: Multiple thurbox instances synchronize all state
(sessions, worktrees, automations)
via a shared SQLite database
(`~/.local/share/thurbox/thurbox.db`). Each instance polls
`PRAGMA data_version` to detect external changes. SQLite's WAL mode
handles concurrent access safely. Deletions use soft delete
(`deleted_at` column).

*This supersedes the original TOML file-based approach. The migration
to SQLite resolved race conditions where concurrent `save_state()` calls
could overwrite each other's writes.*

Session **I/O is NOT coordinated** via the database. Instead, each
instance independently connects to tmux and adopts all visible sessions.
Tmux natively handles concurrent clients: output is broadcast to all
connected clients, and input commands are serialized. This enables true
multi-instance collaboration without application-level locks or
ownership restrictions.

**Why**: This approach is:

- **Atomic**: SQLite transactions prevent torn writes and race conditions
- **Portable**: Works on Linux, macOS, any system with a filesystem
- **TEA-compatible**: External changes flow through the message pipeline
- **Graceful**: Single instance has zero polling overhead
- **Collaborative**: All instances can interact with the same sessions
  simultaneously (like tmux attach with multiple clients)
- **Single source of truth**: No split-brain between state files and DB

**Multi-Instance I/O Model**: Rather than using an ownership model
to prevent duplicate I/O, each instance maintains its own control mode
connection to tmux. Tmux's architecture already supports this:

- Each control mode client receives independent output streams
- Output is duplicated by tmux to all connected clients
- Input commands (`send-keys`) are serialized by tmux
- No application-level coordination needed

This design choice (post-ADR) was made to enable true collaboration while
avoiding the complexity of application-level locks or message-passing for
I/O coordination.

**Trade-offs**:

- **Not human-readable**: Unlike TOML, users cannot directly edit state.
  The TUI provides all editing UI (session creation, scheduling, theme
  selection). Agent definitions are the deliberate exception and remain
  hand-editable TOML (ADR-19).
- **Independent terminal state**: Each instance maintains its own
  `vt100::Parser`, so concurrent updates may briefly diverge. Instances
  converge quickly as output is replayed.
- **Concurrent input interleaving**: When multiple users type
  simultaneously, characters arrive in order at tmux but may display
  interleaved (same as `tmux attach` with multiple clients). This is
  **expected behavior** for multi-user terminal sessions.

**Rejected**:

- *Event-based sync (inotify/kqueue)* — platform-specific, requires
  different implementations for Linux/macOS/BSD, more complex error
  handling (file deletion, permission issues), adds monitoring
  overhead even for single-instance deployments.
- *gRPC/REST daemon* — requires deploying and managing a persistent
  service, adds operational complexity, increases failure surface area
  (daemon crashes, socket issues), incompatible with offline usage.
- *Git-based sync* — requires git repo for state, introduces gc/
  rebase issues, incompatible with non-repo environments.
- *TOML file-based sync (previous approach)* — race conditions when
  multiple instances write concurrently; no atomic multi-key updates;
  split source of truth between config.toml and state files
  (sessions) caused sync bugs.

---

## ADR-15: Headless CLI as Separate Binary

**Choice**: Headless automation lives in a separate binary
(`thurbox-cli`) that shares the same SQLite database as the TUI.
It exposes `session`, `automation`, `task`, `message`, `editor`,
`config`, `extension`, `version`, `update`, and `notify` management
as subcommands, printing JSON results.

**Why**: A separate binary keeps scripting/automation out of the
TUI's event loop. The TUI already polls `PRAGMA data_version`
on every tick (~10 ms event-loop cadence) (ADR-7b), so changes
made by `thurbox-cli` appear
automatically — no new synchronization mechanism is needed. The
`cli` module imports `storage`, `session`, `session_ops`, `sync`,
and `agent::tmux`, but never `app` or `ui`, so it can operate
without a terminal UI.

**Rejected**:

- *Embedded in the TUI binary* — would force the TUI to multiplex
  a non-interactive command path alongside its crossterm event
  loop.
- *A long-running daemon* — adds operational complexity; the
  shared SQLite DB plus tmux already provide the coordination a
  one-shot CLI needs.

---

## ADR-14: Centralized Theme Module

**Choice**: All UI colors are defined as associated constants on a
`Theme` struct in `src/ui/theme.rs`. Widget files import `Theme::*`
instead of using `Color::Cyan`, `Color::Gray`, etc. directly.

**Why**: ~50 hard-coded color values were scattered across 13+ widget
files. This made visual consistency difficult to maintain and made
any color scheme change require editing every file. Semantic names
(`ACCENT`, `STATUS_BUSY`, `TEXT_MUTED`) clarify intent at each call
site and enable future theming (dark/light/custom) with a single
module swap.

**Design**: `Theme` uses `const` associated items rather than a
global singleton or trait. This keeps it zero-cost (no runtime
dispatch, no initialization), works in const contexts, and is
trivially testable. Composite styles (e.g., `focused_title()`) are
`const fn` methods that combine colors with modifiers.

**Rejected**:

- *Global singleton / `lazy_static`* — runtime overhead, mutex
  contention in render path, unnecessary for static color values.
- *Trait-based theming* — over-engineering for the current need.
  Can be layered on top later if user-selectable themes are added.
- *CSS-like stylesheets* — no Rust TUI framework supports this
  natively; would require a custom parser and resolver.

---

## ADR-19: Declarative agent definitions

**Choice**: Each session runs exactly one coding-agent CLI chosen
at creation time; each agent runs with its own default config.
Agents are described as **data** in `~/.config/thurbox/agents.toml`
(sibling of any other config), seeded with built-ins (claude,
codex, antigravity, opencode, aider, copilot, vibe, pi, omp) on first run via
`agent::agent_config::load_or_seed`. An `AgentDef` carries a
`command`, `args` (always passed — bake in flags like a model
here if you want), and argument-template groups (`resume_args`,
`fork_args`, `new_session_args`), plus a `resume_latest` flag. A
single `agent::GenericProvider` (an `AgentProvider`) launches any
defined agent by substituting `{id}` and appending each group only
when its driving value is present. Only `claude` can be addressed by
the thurbox-generated id (`--session-id {id}`); the other built-ins
can't pin or report a session id, so they set `resume_latest = true`
and use id-less, cwd-scoped flags (`codex resume --last`, `opencode
--continue`, …) that make the agent resolve "the last session in this
directory" itself. `resume_latest` only governs *when* the resume
group fires at restart (`session_ops::resume_trigger_for`): for these
agents restart always resumes; claude still defers to an on-disk
transcript check.

**Why**: Thurbox started as Claude-Code-specific, with a hard-coded
`ClaudeProvider` plus roles, skills, profiles, and an MCP/plugin
surface tied to one agent's permission model. Generalizing to "run
any coding agent" meant the launch contract had to be data, not
code: users add or tweak agents by editing TOML, with no recompile
and no per-session permission/prompt/tool configuration. The
`session::AgentDef` / `AgentRegistry` types are pure data (no
filesystem, no local imports) so they satisfy the `session/`
isolation rule; the TOML loading and the provider bridge live in
`agent`.

**Group precedence**: fork wins over resume, which wins over a
fresh `new_session` id; static `args` follow. A group with no
value is simply omitted — no "unresolved placeholder" heuristics.

**Config, not DB**: Agent definitions deliberately live in TOML
rather than SQLite (ADR-8). They are read-mostly, hand-editable,
and shared across instances by re-reading the file — there is no
concurrent-write hazard that would justify moving them into the
database.

**Rejected**:

- *Hard-coded providers per agent* — the previous `ClaudeProvider`
  approach; adding an agent meant a code change and release.
- *Per-session roles / permissions / prompts / tools* — removed
  with the pivot. They were Claude-specific and did not generalize
  across agents; a session now configures only its agent.
- *Agent definitions in SQLite* — overkill for read-mostly,
  user-authored config; TOML keeps them inspectable and diffable.

## ADR-20: Agent-agnostic extensions in `extensions/`

**Choice**: Opt-in workflows that *compose* thurbox (rather than
extend the binary) live in `extensions/<name>/` as data + shell:
a plain-markdown behavior spec, portable scripts built on
`thurbox-cli` + `jq`, and a curl-able, idempotent `install.sh` —
the same distribution model as `scripts/install.sh` and
`packaging/`. The first extension is **flow** (an experimental
focus-protecting triage agent; see FEATURES.md). Extensions reach
agents only through `agents.toml` **aliases** (e.g. `flow-worker`)
that the user maps to any CLI, and surface their spec through
context-file symlinks (`CLAUDE.md`/`AGENTS.md`/`GEMINI.md` → the
spec), so no vendor is named anywhere.

**Why**: ADR-19's pivot made thurbox agent-neutral; an opinionated
LLM workflow (prompts, triage rubrics, tick cadences) would undo
that if baked into core, and it iterates on a much faster cadence
than the binary (editing a markdown spec vs. cutting a release).
Keeping extensions as data over the public surface (`thurbox-cli`
plus `agents.toml`) also makes that surface's stability a tested,
load-bearing contract.

**Rejected**:

- *Vendor plugin formats* (e.g. a Claude Code plugin) — couples
  the workflow to one agent's ecosystem; the same agent brain must
  be runnable by codex, antigravity, opencode, vibe, ….
- *A `thurbox-cli flow init` subcommand with embedded assets* —
  puts one opinionated workflow inside the agent-neutral core and
  ties spec iteration to the release cycle.
- *A separate repository* — the extension scripts against
  `thurbox-cli`'s JSON surface and should version and CI alongside
  it.

## ADR-21: Declarative extension manifests + first-class lifecycle

**Choice**: Extend ADR-20 by teaching the core a single declarative
**manifest format** (`extension.toml`, `session::ExtensionDef`) and a
first-class lifecycle on the public surface:
`thurbox-cli extension install/uninstall/activate/deactivate/list/status`
(`session_ops::*`, `agent::extension_config`). The manifest has an
*install* half (`home`, `[[agents]]`, `[[files]]`, `[[symlinks]]`) and a
*runtime* half (`[[sessions]]`, `[[automations]]`). `install` resolves a
source (a bare name → the official repo pinned to the binary's release
tag; a path; or an `http(s)://` base — fetched via `curl`/`wget`), lays
down the payload, registers agents (append-only, comment-preserving),
writes the home-resolved manifest to the discovery dir, and activates.
Active extensions are recorded in SQLite `metadata` and **self-healed**
(missing sessions/automations recreated) at TUI startup and on every
`automation tick`. The core still knows the *format*, never a specific
extension; flow's `install.sh` becomes a thin shim over the CLI.

**Why**: ADR-20 left each extension to reimplement bootstrap in bespoke
shell, and gave no way to recover from a half-removed extension. Folding
the mechanics behind one data-driven command makes install reproducible
and uninstall symmetric, and self-heal makes an active extension robust
against accidental deletion — all while staying extension-neutral
(reusing `spawn_session_headless`, `db.create_automation`, `AgentDef`).
Pinning the fetch to the binary's release tag keeps a fetched extension
in sync with the binary that reads it.

**Rejected**:

- *Embedding extension assets in the binary* (the option ADR-20
  rejected) — still rejected; `install` fetches **data** at runtime, it
  does not bake assets in, so the agent-neutral core is preserved.
- *Adding an HTTP client dependency* — `curl`/`wget` shell-out matches
  the existing installer and keeps the dependency tree small.
- *Re-serializing `agents.toml` to add/remove agents* — would drop user
  comments/formatting; the installer edits text (append on install,
  block-removal by name on uninstall) instead.

## ADR-22: `App` decomposition — coordinator + per-domain sub-modules

**Choice**: Keep the single `App` model (ADR-1, TEA) but split its
~11.7k-line `app/mod.rs` into per-domain sub-files under `src/app/`,
relocating cohesive `impl App` method clusters out of `mod.rs` while the
state they own lives in small per-cluster sub-structs. `app` stays one
**EXEMPT** module in `tests/architecture_rules.rs` (the coordinator that
imports every layer), and governance is directory-level, so the new
`app/*.rs` files introduce **no** new cross-layer edges and need no
allowlist entries — the split is entirely intra-`app`.

Two halves:

- *State* — already mostly done: `task_ui: TaskUiState`, `automation_ui:
  AutomationUiState`, `new_session: NewSessionWizardState`,
  `global_search: GlobalSearchState`, `worktree_sync: WorktreeSyncState`,
  `metrics`, `notification_state`. Two remain to extract: a new
  `PointerState` (text-selection / click-target / scrollbar / hover
  registries) and a `SpawnController` holding **only** the
  background-task machinery (`worktree_create`/`session_spawn` + their
  `pending_*`).
- *Behavior* — relocate the method clusters into domain files:
  `app/tasks.rs`, `app/automation.rs`, finish `app/search.rs`,
  `app/mouse.rs`, `app/worktree_sync.rs` + `app/git_stats.rs`, and
  `app/spawn.rs`. Methods stay `impl App` (they coordinate side effects);
  only pure state/logic lands on the sub-structs.

**The spine stays on `App`** (clusters borrow it, never own it): the
session vector + selection cursor (`sessions`, `active_index`), the
backend registry (`backends`), per-session render views
(`session_terminal_views`), the render-loop flags (`needs_redraw`,
`last_draw_at`, `last_output_gen`), the status/order caches
(`cached_hook_states`/`hook_states_version`, `cached_session_order`,
`last_active_session_id`, `spinner_frame`), and
`metrics`/`db`/`session_counter`/`terminal_rows`. The TEA methods
(`update`, `tick`, `view`, `handle_key`/`dispatch_action`, `new`,
`shutdown`), session restore/adopt, and all navigation/status/ordering
stay too — navigation *is* manipulation of the shared cursor. Two
cross-cluster handoff slots stay explicit and `pub(crate)`:
`pending_task_prompt` (tasks↔spawn) and `deferred_inputs`
(spawn/sync/paste).

**The spawn boundary**: `SpawnController` owns only its background tasks
and exposes `poll() -> SpawnEvent` (`WorktreesReady`/`Spawned`/`Failed`);
`App` applies the event via the existing `finalize_spawned_session`. The
controller never owns session *adoption* — that body touches `sessions`,
`active_index`, `focus`, `db`, `deferred_inputs`, `metrics`, and
`task_ui` in one place, and pushing it into a sub-struct would re-create
the god-object through a `&mut App` parameter.

**Order** (each its own PR, green throughout; `app/acceptance.rs` is the
safety net): (1) tasks → (2) automations → (3) search — the safe
relocations, state already extracted — then (4) mouse (first new
sub-struct), (5) sync, (6) spawn (machinery only; last and hardest).
Because all relocations carve from the same `mod.rs`/`key_handlers.rs`,
they are **sequenced**, not run in parallel, so each rebases onto the
prior cleanly.

**Why**: `mod.rs` is the repo's hottest merge-conflict file and
interleaves spawn/mouse/task/automation/sync/metrics, so no single flow
can be read without scrolling past four others. The split shrinks
`mod.rs` toward a coordinator + spine (~5–6k lines) with each domain's
invariants local, and *strengthens* the TEA spirit — side effects stay
concentrated at the coordinator, pure state/logic gets isolated — rather
than bending it. The state half is already underway, so most of the work
is mechanical relocation against existing tests: low risk, high
readability gain.

**Rejected**:

- *Splitting `App` into multiple models / TEA loops* — breaks ADR-1's
  single `update`/`view` and the `data_version`-driven redraw; the
  coupling is real (every cluster reads the selection cursor), so one
  model with a borrowed spine is correct.
- *Owning the spine in sub-controllers* (e.g. a `SessionController`
  owning `sessions`/`active_index`) — every other cluster borrows it, so
  this merely relocates the god-object and forces `&mut App`-style
  params everywhere.
- *Pushing side-effecting methods onto the sub-structs* — would drag
  `db`/`sessions`/`deferred_inputs` into each cluster and reintroduce the
  coupling; behavior stays `impl App`, only pure logic moves.
- *One big relocation PR* — unreviewable and merge-hostile; the value is
  in independently-reviewable, test-green increments.

---

## ADR-23: The v2 teardown inventory is a test, not a document

**Choice**: What v2's final phase deletes — the v1 extension system
(ADR-20, ADR-21) and each native pane — is recorded in
`tests/teardown_gate.rs` as two tables: one row per v1 capability that
must survive, carrying the v2 home it is promised, a recorded verdict on
whether that home exists, and a **probe** that re-derives the verdict from
the source tree; and one row per deletion unit, listing the paths and
in-source markers it comprises plus the capability ids that must be ready
first. A listed path or marker may not disappear while any of its unit's
requirements is unready, and a recorded verdict that disagrees with its
probe fails.

**Why**: the teardown's dangerous failure is silent. A half-deleted pane
is caught by the compiler; a *cleanly* deleted pane, or a deleted built-in
hooks installer, compiles and ships — and what stops working is agent
status reporting (`working`/`blocked`/`done`) for every agent, which is
core product behavior delivered *by* the installer the same teardown
removes. A readiness verdict is also a fact about a build, and a fact in a
markdown table expires without telling anyone: probing the tree means
implementing a replacement forces the row that depends on it to be
revisited, so the inventory cannot decay into a rubber stamp. The gate is
a source-level check, so it needs no plugin feature and means the same
thing in both Cargo configurations — the same allowlist shape
`tests/architecture_rules.rs` already uses.

**Rejected**:

- *Prose in the migration plan* — the analysis lives there either way; the
  gate is what survives contact with a session that reads a phase label
  first and starts deleting.
- *Per-file requirement mapping* (e.g. `json_merge.rs` needs only the
  hooks and config-dir rows) — defensible, but it is a judgement made on
  behalf of whoever does the deletion. The extension system is deleted as
  one unit, so the unit requires the whole set and a narrower claim has to
  be argued in the table, with its reasons attached.
- *A checklist in the PR template* — unenforced, and invisible to the
  agent sessions that do most of this work.

---

## ADR-24: Pane geometry is a workspace tree; slots are a preset over it

**Choice**: `ui::layout` divides the screen with a **tree of splits**
(`session::workspace_tree`) rather than a fixed set of named rects. A branch
splits its rect along one axis and carries its children in order; a leaf names
one `RegionId`. `compute_layout` is three stages — `default_preset` synthesizes
the tree from `LayoutParams`, `solve` divides the rects, and `PanelAreas` is the
projection the view reads. The five v1 panel slots become the **default
preset**, and the right column holds one region per *visible* plugin pane rather
than a single one.

**Why**: the fixed-rect model answered "where does a pane go" with one field per
panel, so every pane thurbox has ever added widened `PanelAreas` and threaded a
branch through the split. Concretely it also seated exactly **one** plugin pane
(`RightSlot::Plugin`, drawn as `plugin_panes.iter().find(|p| p.visible)`), so a
second bundled plugin was invisible however it was configured — the wall
`docs/PHASE4-PANE-READINESS.md` §5 recorded. A tree answers full-width regions,
grids, nested splits and runtime reordering with the same structure instead of a
slot name each.

**Geometry is preserved by construction, not by inspection.** `Sizing` has
exactly three variants and they map 1:1 onto `Length` / `Percentage` / `Min` —
the only three `Constraint` kinds the previous code used — so every branch hands
ratatui the byte-identical constraint list. That extends to keeping a hidden
vertical band as a zero-cell child rather than omitting it: a shorter constraint
list is a different input to the solver, and the *projection* is what reads zero
extent as "not shown". The evidence is the ~115 pinned `insta` acceptance
snapshots plus 41 pre-existing layout assertions, all unchanged.

The tree data lives in `session` (pure data, no crate-internal references)
because a loadable layout file would be parsed under `agent`, which may
reference `session` but never `ui` — the same placement as `AgentDef`,
`HostDef`, and `theme_config::CustomThemeDef`.

**Rejected**:

- *Add a second plugin slot* — cheapest per request, and it ends with a slot
  name per position, which is the model being replaced.
- *Write a flex solver instead of calling `Layout::split`* — the honest "real
  layout engine", and the wrong trade here: ratatui's rounding is exactly what
  the snapshots encode, so re-deriving it would turn a behaviour-preserving
  refactor into a hunt for off-by-ones. The tree's value is the structure; the
  arithmetic was never the problem.
- *Replace `PanelAreas` with a `RegionId → Rect` map at every call site* — the
  end state, but 30-odd consumers would churn for no behavioural gain, and named
  fields catch a typo that a map lookup answers with `None`. The map exists
  behind the projection.
- *A `min_width` node key, with the solver hiding any region under its minimum*
  — the general rule, and it hides the wrong region: when the right column
  over-subscribes, the starved region is the **center**, which is the fallback
  view and must never be hidden. The count of plugin columns that fit is
  therefore decided in the preset (`CENTER_MIN_COLS`), and the node key lands
  with the config file that can set it.
- *Gate the first plugin column on the center's width too* — symmetric, and it
  would change a layout that already ships (four occupants at the wide
  threshold already leave a single-digit center). Panes past the first are new
  capacity and can be gated without a regression.

---

## ADR-25: Anchored overlays instead of a floating-element ban

**Choice**: a rect may be positioned against **another rect** — its *target* —
instead of taking a share of a split. `session::overlay::Overlay` declares the
side, whether flipping is allowed, and the extents; `Overlay::place` resolves it
against the target and the owning pane's rect. `ui::overlay::OverlayLayer` holds
one pane's declarations in order and hands them back **topmost first**, which is
the order click hit-testing consults them in. The code-review compose box is the
first consumer, replacing the bespoke placement in `render_compose_inline`.

**Why**: the base layer's "nothing overlaps" invariant was never the whole
truth. v1 already floats one element and did it with eleven lines local to one
function — prefer below the selected diff line, else above it, else pin to the
bottom edge, inset a column — reachable by nothing else. Every other surface
that wants that shape (a completion dropdown, a context menu, a tooltip) had no
route to it. The v2 design had answered with an `inlineAt` slot on the diff
node, which blesses the one case that already existed and leaves the second
consumer to re-open the question; retiring it *before* anything depends on it is
what stops a pane being migrated twice.

What actually changes is one invariant, narrowed rather than dropped: the
**base** layer never overlaps; the **overlay** layer may, and is strictly
ordered by declaration. Focus is untouched — an overlay belongs to its pane and
is not a focus target, so exactly one pane still holds focus.

**The port is exact by construction.** v1's three branches are the resolver's
three steps (prefer the side, flip, dock to the clip's far edge), and its
one-row target rect makes "below" mean `target.y + target.height`, so no offset
is needed. `compose_anchor_reproduces_the_legacy_inline_placement` sweeps every
anchor row across pane heights 3–23 and widths 2–60 against the old formula kept
as an oracle.

**The one divergence, named**: v1 computed the box's height as
`area.height.clamp(3, 6)`, so a diff area of one or two rows got a three-row box
docked at `bottom - 3` — *above* the pane, painting over its neighbour. The
resolver clamps every extent to the clip first, so the box shrinks instead.
Reachable only while composing in a pane under three rows tall, pinned by
`compose_anchor_clamps_a_pane_too_short_for_the_box`.

**Rejected**:

- *Keep the ban* — it pushes every plugin toward a centered modal (wrong shape
  for a dropdown) and keeps the diff's special case permanently.
- *A `z-index` property* — familiar and unbounded. Declaration order is enough
  for menus and compose boxes and cannot be escalated into a layering war.
- *`anchor.to = "<node-id>"` now* — there is no id space to resolve against: a
  pane's contents are Rust render functions, so callers pass the rect they
  already hold. The lookup, and the "a dangling `to` renders nothing, logged
  once" rule that belongs with it, land with the plugin node tree.
- *A missing-target policy (`hide` vs `dock`)* — `hide` exists for a dangling
  id, and nothing can dangle yet. One rule, dock, covers the only reachable case
  (the target scrolled out of view) and is what v1 did.
- *An `offset` nudge* — no surface wants one, and it needs three more specified
  interactions (does the fit test use the shifted rect? the flip? the dock?) to
  serve a hypothetical gap.
- *A nesting cap of three* — nesting means anchoring to an anchored subtree,
  which needs the same node ids. A cap on something that cannot happen is
  unenforceable.
- *Intersecting the resolved rect with the pane afterwards* — same containment,
  worse result: a six-row box in a two-row pane would render its border and lose
  its content, where clamping the extent first yields a coherent two-row box.
- *Escaping the pane rect* — wanted for a dropdown at a narrow pane's edge, and
  it needs cross-pane z-ordering plus an answer for the owning pane being hidden
  mid-interaction.

## ADR-26: The view tree is the kernel's rendering IR, not plugin surface

**Context.** Phase 0's last exit criterion asked for the info panel to render
through `session::view_tree` with byte-identical snapshots. Attempting it
surfaced a fact the plan had not stated: `session::view_tree`, `session::motion`
and `ui::plugin_pane` were all `#[cfg(feature = "plugins")]`. A pane that
rendered through the tree would therefore have had *two* renderers selected by a
Cargo feature — which is precisely the divergence the byte-identity criterion
exists to prevent, and it would have been invisible in the default build that
users install.

Porting also settled two gaps `docs/PHASE4-PANE-READINESS.md` had predicted and
found a third the audit had missed.

**Decision.**

1. **Ungate the tree.** `session::view_tree`, `session::motion` and
   `ui::plugin_pane` compile in every build. They reference neither `mlua` nor
   `crate::plugin`, so `cargo tree --edges normal | grep -c mlua` stays `0` and
   the dependency graph is byte-identical — the feature gated *visibility*, not
   cost. `ui::plugin_pane` keeps its name: renaming carries no behaviour and
   would have obscured the diff.
2. **A `gauge` node** (audit §3 §4's "prefer the node"). Label, percentage,
   optional suffix; the kernel resolves the flush-right placement and the bar
   length from the area. Rejected: reporting the resolved rect back to a plugin —
   it makes rendering width-dependent, so a resize must re-enter the VM before
   the frame that needs it, which ADR-V11 forbids.
3. **A `paragraph` node** that soft-wraps, beside `line` which clips. It is the
   gap the audit missed. Rejected: a `wrap` flag on `line`, because `line`'s
   specification *is* "clips rather than wraps" and a flag negating that makes the
   requirement meaningless. Its height is the one in the catalogue that depends on
   width, so `height_of` takes one.
4. **Eleven more style tokens**, each named for and resolving 1:1 onto the
   `ThemePalette` field it addresses, including one per session status. Rejected:
   letting a node name a colour, which would end the property tokens exist for.

**Consequences.**

- The catalogue's rule is now explicit: a node may depend on its area (`gauge`,
  `paragraph`, `divider`, `row`), but never on a *number a plugin was told*.
- Byte-identity is a **test**, not a claim: the pre-port line builders are
  retained under `#[cfg(test)]` as an oracle, and
  `view_tree_render_matches_the_legacy_paragraph_cell_for_cell` compares the two
  renderings cell by cell across 59 widths × 6 heights × 9 content variants. It
  earned its keep immediately — it caught that v1's *gauge header* wraps when
  label plus suffix overflow, pushing the bar down, which the first
  implementation clipped.
- One divergence is accepted and pinned: a handful of separator **spaces** now
  carry a theme foreground where v1 left theirs unset. A space has no glyph and
  neither sets a background, so the pixels are identical; reproducing it would
  need a token meaning "the terminal's default foreground", the one thing the
  token set exists to prevent.
- A second is a fix: agent-supplied text (an OSC title, a notification body) is
  now sanitized, where v1 passed a `\x1b` straight into a cell whose symbol
  ratatui writes to the terminal verbatim.
- `Paragraph::line_count` is behind ratatui's `unstable-rendered-line-info`
  feature, now enabled. Measuring with the same widget that paints is what makes
  the wrap identical by construction rather than by a reimplementation of
  word-wrapping that would have to be tested into agreement.
- **Audit §2 (no host binding reads kernel state) stays open**, and it is why
  this pane is still not portable to a *plugin*: it receives its `SessionInfo` as
  an argument. The proven claim is narrow — the catalogue can express this pane's
  rendering, not that a third party could have written it.

## ADR-27: Kernel state reaches a plugin as a published snapshot, gated per kind

**Context.** ADR-26 closed every *rendering* gap between the view tree and
thurbox's own info panel, and left one open which it named as the reason the pane
was still not a plugin: `plugin::capabilities::build_module_table` granted
`name`, `log`, the `state*` trio over the plugin's own namespace and the `ui`
constructors — and nothing through which a plugin could read a session, a task,
or anything else the kernel owns. A pane that renders kernel data could not be
written at all.

Phase 4 turns the native panes into bundled plugins, so the gap had to be closed
before the first one. `docs/PHASE4-PANE-READINESS.md` §2 had already named the
right precedent — `session::spawn_contribution` — and two properties that had to
be designed rather than assumed: the read is capability-gated, and publishing is
not per-tick work.

**Decision.**

1. **A published snapshot, not a binding that reaches into `App`.**
   `session::pane_context` holds `PaneContext` (pure data) plus a process-wide
   `RwLock<Option<PaneContext>>`. `app` builds and publishes it; `plugin` reads it
   when a plugin calls a reader. Rejected: a binding holding an `&App`, which
   needs the refused `plugin → app` edge and would put plugin code on the UI
   thread. Rejected: passing state as a second argument to `render`, which grants
   every plugin a session's name and activity text with no capability declared.
   The module is **ungated**, for ADR-26's reason: a kernel data type gated on a
   Cargo feature is how one pane ends up with two descriptions of its own state.
2. **Three capabilities, not one.** `sessions`, `metrics` and `automations`, each
   gating one reader (`thurbox.activeSession()`, `thurbox.systemMetrics()`,
   `thurbox.upcomingAutomations()`). The capability list is what an install prompt
   is written from, and "reads your sessions" is a different question from "reads
   this machine's CPU and memory". Rejected: a single `state` capability — it
   makes the smallest pane that wants a session name also demand host telemetry.
3. **The snapshot resolves what a plugin cannot compute, and nothing else.** A
   VM loads no `os` and no path library, so the kernel resolves the clock
   (`resets_in_secs`, `due_in_secs` — in *seconds*, the granularity the countdown
   is displayed at, so the value does not differ on every tick), path basenames,
   a parent session's name, and each status's glyph *and style token* (the token
   because `StyleToken::for_status` exists so two panes cannot disagree about it).
   Everything else is a **number**: the plugin composes every string it displays.
   Rejected: publishing formatted strings — it would have made the port trivial
   and worthless, since the pane would be arranging strings the kernel composed.
4. **Two gates on publishing.** Nothing is built unless a running plugin holds a
   state capability (`pane_context::readers_present()`, an `AtomicBool` the host
   sets from the grants it already computes); nothing is written unless the value
   differs from what was published. Rejected: an input signature on the
   `App::session_order_signature` pattern — over *these* inputs it must touch
   every field the snapshot touches, so it saves allocations rather than
   traversal, at the cost of a second description of the snapshot's dependencies
   that can drift from the snapshot. Publishing deliberately does **not** mark
   the UI dirty: a pane repaints when its own tree changes, and coupling the two
   would repaint the screen for a pane that is not on it.

**Consequences.**

- The bundled `info-panel` plugin (`src/plugin/bundled/info-panel/`) reproduces
  the native pane in Luau, and `tests/bundled_info_panel.rs` asserts its view
  tree **equals** `ui::info_panel::info_tree`'s across ten content variants. The
  same renderer paints both, so an equal tree is a byte-identical pane — and a
  failure names a node rather than a cell. Its manifest declares four
  capabilities and the test asserts it holds no fifth, so the pane is evidence
  about what a *third party* can build.
- `info_tree` takes `now` as a parameter. It read the clock to build the usage
  countdown, which made its output depend on wall time; a plugin has neither a
  width nor a clock, and the comparison above has to be exact rather than
  minute-boundary flaky.
- **The view tree needed no widening.** An independent consumer needed `list`,
  `paragraph`, `divider`, `gauge` and `text` with eight tokens, all of which
  ADR-26 had already added — which is the confirmation ADR-26 could not give
  itself.
- **Freshness is the cost that did not get paid.** The render worker polls on a
  ~1 s cycle, so the plugin's copy of a live gauge lags the native pane by up to a
  second. `docs/SPIKE-SESSION-LIST.md` already fixed event-driven render as a
  condition of the session-list port; this is the second pane to want it.
- **Every plugin will reimplement `format_bytes`.** A `thurbox.format.*` table
  would fix it and is deliberately absent: it should be designed from two or
  three panes' needs, and adding it now would destroy this port's evidence that a
  plugin can own its own presentation.
- The teardown gate is stricter. A pane's replacement verdict now requires
  *handover* — the plugin exists **and** `src/app/view.rs` no longer draws the
  native pane — because a plugin reproducing a pane alongside the native one has
  replaced nothing, and the old probe would have permitted deleting the renderer
  every user is looking at.

## ADR-28: One action reaches N panes, and the kernel tells the host what to skip

**Context.** ADR-27's port left `docs/PHASE4-PANE-READINESS.md` §5 half open, and
the half that was open had a consequence: `App::toggle_plugin_pane` mutated
`plugin_panes.first_mut()`, so with `hello` and `info-panel` both declaring a
pane, **the pane ADR-27 shipped could not be put on screen by any key** — only by
`thurbox-cli command run info-panel.info.show` or by editing the stored choice.
The same section recorded a second cost: `PluginHost::render_all_panes_collected`
rendered every declared pane and the view then discarded the hidden ones, so a
default install paid a Luau render per cycle for panes nobody could see. Phase 4
schedules seven panes, which turns both from untidiness into arithmetic.

**Decision.**

1. **One bound action, a picker only when there is something to choose.**
   `Action::TogglePluginPane` toggles directly with one declared pane and opens
   `Modal::PluginPanes` with two or more; with none it does nothing. The rule is
   the new-session **host picker**'s: a chooser over a single option is a question
   with one answer. Rejected: **one generated action per pane** (ADR-V21's shape
   for *commands*) — `session::Action` is a fixed enum that `keybindings.json`
   maps chords onto and whose order indexes the F1 editor's rows, so generating
   variants per discovered pane makes the keybinding namespace depend on which
   plugins are installed, and a plugin that fails to compile would silently drop a
   user's binding. The generated `<plugin>.<pane>.{toggle,show,hide}` commands
   already serve the name-addressed case, headlessly. Rejected: `F10` cycling
   which single pane is shown — two panes side by side is a configuration the
   workspace tree explicitly supports. Rejected: always opening the picker — it
   turns the one-pane case into three keystrokes.
2. **The picker's rows are plain `app` data.** `modals::PluginPaneRow` carries
   plugin, id, title and visibility, because `ui` may not reference
   `crate::plugin` (`tests/architecture_rules.rs`) and putting the plugin host in
   the view's type graph for four fields is a poor trade. Both routes write
   through one setter, `App::set_plugin_pane_visible`, so a keyboard toggle and a
   `hide` command leave the same stored choice by construction.
3. **The hidden set is published, and the host skips it.**
   `session::pane_visibility` holds the panes the kernel is keeping off screen;
   `app` publishes on the tick behind a change gate and `plugin` consults it
   before entering a VM. Same mechanism as ADR-27 and for the same reasons — no
   reference held either way, no plugin code on the UI thread, no new module edge.
   It publishes the **hidden** set rather than the visible one so that "a pane
   nothing was published about is drawn" is the structure rather than a rule to
   remember: a process that never publishes (a short-lived `thurbox-cli` command)
   renders exactly as before. Unlike `pane_context` the module **is** gated on the
   `plugins` feature: this is not kernel state a pane reads, it is scheduling
   input for the render worker. Rejected: letting the host read the stored choice
   (needs `plugin → storage`, the edge the host exists to avoid, and puts SQLite
   in the render loop); rejected: filtering `PluginHost::panes()` instead, which
   would hide a pane from the very picker that turns it back on.
4. **The skip is counted, not asserted.** A pane filtered out before the call is
   indistinguishable, in the returned results, from one rendered and discarded —
   which is how the discarding version survived. `PluginHost::render_calls` counts
   VM entries inside `render_pane`, so no caller can satisfy the rule by skipping
   in one path only, and `pane_visibility_publishes` joins the perf counters so a
   regression to per-tick publication is a failing test rather than a profile.

**Consequences.**

- The pane ADR-27 shipped is reachable from the keyboard, and every later Phase 4
  pane is reachable the moment it is declared — `migration/phase-4` now requires
  it, so the next port cannot repeat the omission.
- A hidden pane's tree goes stale while it is hidden, so unhiding shows its last
  tree (or `loading`) for up to one worker cycle. That is the ~1 s staleness ADR-27
  already recorded, now paid once on a keystroke instead of every second forever.
- `Modal::PluginPanes` is `#[cfg(feature = "plugins")]`, unlike the rows and the
  renderer beside it. Not a preference: rustc reports a variant no code constructs
  as dead code and `-D warnings` is a hard gate, so a stable build cannot carry an
  unconstructible variant.
- §5's remaining measurement is answered rather than deferred: the worker no
  longer renders panes the user cannot see, which was the cost the motion work was
  careful never to pay for a hidden pane.

## ADR-29: A list pane's rows are published state; its geometry is not

**Context.** The tasks pane is the second Phase 4 port and the first *list* pane,
which is the shape the four remaining ones have. Reading
`src/ui/tasks_panel.rs` rather than the node catalogue — the discipline
`docs/PHASE4-PANE-READINESS.md` §6 records — the pane is seven decisions, and
they split cleanly in two. Six are presentation (a status's glyph, its colour, the
selected > dimmed > status precedence, the emphasis on a matched run, the trailing
`⇄`, the empty-state line) and one is geometry (which rows are in view, how wide a
title may be, where the marker's room comes from). Two of the six could not be
expressed at all: `TextStyle` carried a colour token and `bold`, so a dimmed row
and an underlined matched run had no spelling. And none of the pane's state was
reachable — the task list is in SQLite and `plugin` may not import `storage`.

**Decision.**

1. **The task list is a section on ADR-27's published snapshot, not a database
   seam.** `PaneContext::tasks` carries one row per task with its title, its
   status *wire name*, and the four view facts the kernel owns. Rejected: a
   `TaskReader` trait in `session` implemented by `storage`, handed to a VM as a
   factory the way `session::plugin_store::PluginStore` is. It is the obvious
   reading of the architecture rule and it reads the wrong thing: `selected`,
   `dimmed`, `match_positions` and `linked` are not in the database, they are view
   state in `App`, so a plugin reading rows through it could not draw a row that
   looks like the pane's. It would also put a `SELECT` on the render worker per
   cycle, and it would be a second mechanism for "kernel state a pane reads".
   `PluginStore` stays what it was designed for: a plugin's *own* durable state.
2. **The section publishes a status's name, and deliberately not its rendering.**
   The opposite of `StatusSnapshot`, which publishes a session status's glyph
   *and* style token because `StyleToken::for_status` is one mapping shared by two
   native panes and a plugin re-deriving it would be an unchecked second copy. A
   task status is drawn in exactly one place, so publishing `☐` would hand the
   plugin the thing the port exists to prove it can own.
3. **Emphasis joins the view tree: `dim` and `underline` beside `bold`.** They are
   text attributes applied over whatever colour the token resolves to, so the tree
   still admits no way to name a colour. Rejected: a semantic `selected` /
   `matched` flag the kernel styles — it would make the kernel own the look of
   every plugin's list rows, and thurbox's own three list panes would still have
   to match it by hand. Rejected: an options table as `ui.text`'s third argument —
   `bold` cannot stop being the third positional argument, so it would be a second
   spelling of one node.
4. **Geometry stays in the kernel, and the pane splits on that line.**
   `ui::tasks_panel::visible_rows` resolves the window, fits each title and folds
   focus into a per-row `selected`; `tasks_tree` takes those rows and carries no
   geometry, and it is what `render_tasks_panel` paints — so the tree is what
   users see, which is the only thing that makes a tree-equality test evidence.
   Rejected: publishing rows already fitted and windowed, which would make the
   equality total. It needs a width the publisher does not have (the snapshot is
   built on the tick; a pane's rect exists only during a frame, and the native
   pane is hidden by default), and it would fit the plugin's rows to *another*
   pane's size — the plugin's pane is a different rect in the same layout.

**Consequences.**

- The bundled `tasks` plugin reproduces the pane from two declared capabilities,
  `render` and `tasks`, and `tests/bundled_tasks_panel.rs` asserts tree equality
  across eleven content variants — including multi-byte titles and a match offset
  landing inside a character, since the UTF-8 walk over published byte offsets is
  the plugin's own.
- **Two divergences are pinned rather than hidden**, both geometry: a title wider
  than the column is fitted by the kernel and clipped in the plugin's copy, and a
  list longer than the pane is windowed by the kernel and drawn from the first row
  in the plugin's copy. Closing them wants an ellipsizing clip with a flush-right
  run, and a list node carrying a selected index. The second is a precondition of
  the session-list port, not a nicety: a session list that cannot scroll to its
  selection is not one.
- The native pane now renders through the view tree, and the span-building
  renderer it replaced is retained as a `#[cfg(test)]` oracle asserting the two
  paint identically — the same arrangement ADR-26 made for the info panel, and the
  reason "the pane is unchanged" is a check rather than a claim.
- The port needed **no** new style token, no new container node, and no formatter:
  `PHASE4` §7's prediction that every pane would reimplement `format_bytes` is
  still made by one pane, so a `thurbox.format.*` table remains undesigned on
  purpose.

## ADR-30: A list scrolls to a row it names; the file tree is not the filesystem

**Context.** The file viewer is the third Phase 4 port, and it is the pane that
could not accept the gap ADR-29 recorded. Its whole interaction is moving a cursor
through a tree taller than its column, so the tasks pane's compromise — the kernel
windows the list, the plugin's copy draws from row 0 — would have produced a copy
that never shows the row that matters. Reading `src/ui/file_viewer.rs` rather than
the node catalogue (`PHASE4` §6's discipline) turned up two other things the tree
could not say: the pane draws its cursor's row with a **background**, and its
marker glyphs depend on a display **setting** no binding reported. And the port was
specified as needing a filesystem capability, which turned out to be the wrong
reading of the pane.

**Decision.**

1. **A list may name the row its cursor is on, and the kernel windows it.**
   `ViewNode::List { children, selected }`; when `selected` is present the renderer
   picks the visible slice through `ui::file_viewer::visible_window` — the helper
   the native panes already shared. This is the `gauge` trade of ADR-26 applied to
   height, and because both sides go through one function the claim strengthens
   from "the trees are equal" to "the frames are equal when the pane scrolls".
   Rejected again, for ADR-26's reasons: reporting the resolved rect into the
   plugin. Rejected: a second node kind for a selectable list — two spellings would
   force every pane that later grew a cursor to migrate. Rejected: windowing by
   accumulated child heights, which is more general and speculative — `selected`
   means "this list is a list of rows", and matching `visible_window` exactly is
   what makes the frame equality testable.
2. **A run may declare that it belongs to the selected row.**
   `TextStyle::selected` resolves to the theme's `selection_fg` on `selection_bg`,
   *replacing* the token's colour rather than layering over it as the three
   emphases do. The plugin names a role; the theme owns both colours, so the tree
   still admits no way to name one. Rejected — and this is the interesting
   rejection — letting the **list's** selected index drive the appearance, which
   the kernel could trivially do. It cannot, because thurbox's two list panes
   disagree about what a selected row looks like: the tasks pane draws it in the
   accent and bold, the file viewer in the selection pair. An appearance inferred from the
   anchor would have made one of them unreproducible. So the anchor is the list's
   and the appearance is the run's.
3. **`Capability::Files` reads the tree the pane has open — not the filesystem.**
   The published section is basenames, depths, expansion state, each row's search
   verdict, the cursor's index, and whether nerd glyphs are on. It grants no
   `read_dir`, no file contents, no path, and causes no I/O. Rejected: the
   directory-listing capability the port was specified with. Of the five facts a
   row draws only its name comes from disk — depth and expansion are the *user's*
   navigation, `matched` is a search the kernel runs, the cursor is the keyboard's
   — so a plugin holding `read_dir` could draw *a* tree but not *this pane*, and
   the equality test that is a Phase 4 port's deliverable could not exist. It would
   also put blocking, unbounded I/O behind an instruction budget that does not
   measure it. The name is `Files` and not `Fs` on purpose:
   `tests/teardown_gate.rs` reserves `Capability::Fs` for v1's "place a file in an
   agent's own config dir" power, and a filesystem binding here would have advanced
   that verdict as a side effect of drawing a tree.
4. **The kernel publishes `nerdFont`, not the glyph.** ADR-29's rule — publish a
   rendering only when two panes must agree about it — and `src/ui/file_viewer.rs`
   is the only reader of `nerd_font_enabled` outside the theme. It rides on the
   file section because that is its only consumer; a second consumer should lift it
   to its own section rather than a copy appearing.

**Consequences.**

- `ui::file_viewer::file_tree` carries **no geometry at all** — not even a window,
  which `tasks_tree` still receives. The pane resolves the same window itself for
  its click hitboxes and its scrollbar, from the same helper, and a test asserts
  the clickable rows are the rows that were drawn.
- The bundled `file-viewer` plugin reproduces the pane from two declared
  capabilities, and `tests/bundled_file_viewer.rs` asserts tree equality across ten
  content variants (both glyph sets, a running search, a selected row the search
  excluded, deep nesting, wide names, no cursor at all) **and** frame equality at a
  height that forces a scroll.
- **The search sub-mode's bar is out of scope, stated rather than omitted.** It
  needs three things the surface lacks — a bordered container node, a cursor
  appearance, and a bottom-anchored fixed-height region — and its match counter
  would need the query text the capability withholds. The search's *effect on the
  rows* is ported and tested, so the record distinguishes "cannot be drawn" from
  "was not attempted". The scrollbar is likewise pinned as a divergence: it is
  chrome the native pane reserves *outside* the tree, so moving it in would change
  the native pane's layout.
- One limitation of the section, pinned by its own test: the published tree is the
  tree **the pane has open**, filled lazily by the pane that owns it, so a plugin
  sees `No folders` until the native viewer has been opened once for a session.
  Filling it from the publisher would let the presence of a plugin decide when
  thurbox reads directories.
- Still no formatter. After three ports `PHASE4` §7's `thurbox.format.*` case is
  made by exactly one pane, which is now evidence rather than an absence of it.

## ADR-31: A diff row's tint and fill are roles; the plugin owns its highlighter

**Context.** Code review is the fourth Phase 4 port and the largest pane in
thurbox: unified *and* paired side-by-side diffs, syntax-highlighted bodies,
classified comments, reviewed marks, a find sub-mode, a target picker, horizontal
scroll, a wrap toggle, a footer and a floating compose box. Porting all of it
would have answered nothing well. What it can answer, and what no pane before it
had asked, is whether a plugin can style the **inside** of a line thousands of
times: every earlier pane draws one row per record with two or three runs on it,
while a diff row is a gutter, one run per syntax token, and a background that has
to reach the pane's right edge.

**Decision.**

1. **Port the core, itemise the rest.** The reproduced surface is the unified
   stream's *lines*. Everything else is named in the change's proposal with the
   reason it is unported, and `PHASE4` §11 carries the same list. Rejected:
   approximating what is omitted — a plausible-looking file header would agree
   with nothing, and the record would describe a pane that does not exist.
2. **A run may declare its row an insertion or a deletion.**
   `TextStyle::tint: Option<DiffTint>` resolves to `diff_added_bg` /
   `diff_removed_bg`, with `selected` winning — the precedence
   `ui::code_review::row_bg_fn` already encodes, since the cursor's row is one
   appearance whatever it contains. It is a role like ADR-30's `selected`, but it
   leaves the *foreground* to the token, because a diff body's colours are the
   pane's while the tint is the only thing carrying add versus remove (the
   gutter's sign is one character). Rejected: a `StyleToken` per diff background —
   tokens are foregrounds everywhere else, and one that meant "use me behind the
   text" would let any run paint any palette entry behind itself. Rejected:
   inferring the tint from the list's selected row, ADR-30's rejection again — the
   kernel does not know which rows are insertions, and should not.
3. **A run may be a fill.** `ViewNode::Fill { glyph, style }` consumes whatever
   width the line has left after every other run has taken its own, resolved by
   the host at paint time. It exists because a background that stops where the
   text stops is not this pane's row. Same trade as `gauge` for a bar and `list`
   for a scroll window, applied to a line's residue — and it is half of the
   flush-right run `PHASE4` §8 has had open since the tasks port (put a fill
   *before* a run and the run is flush right). Rejected: extending the last run's
   background to the line's end automatically, which would need no node and would
   silently change every existing pane's last run.
4. **The plugin lexes; the kernel publishes no token stream.** The `review`
   section carries a line's raw text and its file's path, and the bundled plugin
   carries the lexer — `ui::syntax` ported to Luau. Rejected: publishing
   `{text, token}` runs, which is smaller, shorter and faster. ADR-29's rule
   decides it: publish a rendering only when two panes must agree about it, and
   `src/ui/code_review.rs` is the only reader of `ui::syntax` in thurbox. A
   published token stream would be one pane's presentation crossing the boundary
   and the "port" would be an arrangement of the kernel's decisions — ADR-27's
   `"8.0/16.0 GB"` objection, applied to the most obviously presentational thing
   in the pane. The cost is that the two lexers must agree token for token, and
   only the equality test makes them; `ui::syntax` was refactored so its
   `Vec<(String, Color)>` form resolves a single `highlight_tokens` against the
   theme, which keeps *one* lexer on the Rust side at least.
5. **The native renderer is not refactored to draw the tree.** The three earlier
   ports made the native pane paint its view tree, so tree equality was frame
   equality by construction. `unified_diff_line` cannot be a geometry-free tree:
   it windows the body to `[h_scroll, h_scroll + avail)`, slices that window by
   *character count* against a resolved width, and the wrap mode reflows one
   logical row onto several by the same arithmetic. So `diff_row_tree` is a new
   geometry-free builder pinned to the untouched renderer by a **frame**
   comparison, and the plugin is compared to the builder. Rejected: comparing the
   plugin to the new builder only, which is two functions written in the same
   change agreeing about a format neither is obliged to match. Rejected:
   refactoring the native path anyway — it would change what the pane draws for
   double-width text and leave the wrap and paired paths as a second
   implementation of the same row.
6. **`Capability::Review` reads the diff the pane has open — not a repository.**
   No diff of the plugin's choosing, no revision range, no file read, no command.
   Same shape and same argument as ADR-30's `Files`, and for the same reason: the
   rows are the review the *user* opened, so a git binding would be strictly more
   power for strictly less result.
7. **The sandbox loads `utf8`.** Pure computation, so admissible under the
   restricted-environment rule for the reason `math` is — and necessary, not
   convenient: a pane that styles inside a line must agree with the host about
   where a character ends, and `string.byte` counts bytes. Without it a plugin
   lexing any line containing a multi-byte character drifts for the rest of it,
   which is a silently wrong pane rather than a refused one. Found by the port.

**Consequences.**

- `tests/bundled_code_review.rs` asserts tree equality across eleven content
  variants (all three line kinds, every colour the highlighter assigns, four
  comment-marker languages, the cursor on a tinted row, a four-digit gutter, empty
  bodies, multi-byte bodies, a tab, no cursor) and **frame** equality at a height
  that forces a scroll; `ui::code_review`'s own tests paint `diff_row_tree` against
  `unified_diff_line` for twelve rows at two gutter widths.
- **The host's bounds are reached by an ordinary diff, and the tighter one is not
  the one the model documents.** A row of real code costs ~26 nodes, so `MAX_NODES`
  (4096) permits roughly 150 rows — but at that size the plugin is refused for its
  **execution** budget, reached while it is still building rows, before its tree is
  ever converted. `MAX_REVIEW_ROWS` (60) is therefore a cap on the *section*, and
  the pane is the first whose content the model cannot bound locally: the node
  budget is a whole-tree bound while a diff's cost is per row *and* per token.
  Both facts are asserted rather than described.
- The budget is spent on rows the kernel then windows away, since the plugin builds
  every row it publishes and the kernel chooses the visible slice afterwards. The
  honest closure — window before conversion, i.e. a lazy row source or a declared
  row budget — is `PHASE4` §11's open row, deliberately not designed from one
  consumer.
- Two divergences are pinned rather than smoothed: node content is sanitized on the
  way into the tree (a tab becomes four spaces) while the native span renderer
  passes the raw byte through, and the Luau lexer classifies letter case for ASCII
  only where Rust's `char::is_uppercase` is Unicode-aware.
- Still no formatter. After four ports `PHASE4` §7's `thurbox.format.*` case is made
  by exactly one pane.

## ADR-33: The session list is a plugin; its cursor and its clock are not

Numbered 33 because ADR-32 is reserved for the automations pane, whose port is in
flight on another branch.

**Context.** ADR-V1 says everything but six things is a plugin **including the
session list**. That clause is the one the whole model rests on: the session list
is the densest pane thurbox has, the most frequently redrawn, and the one whose
ordering, nesting and status rules the kernel owns — so a session list that could
only be kernel-drawn would demonstrate that the plugin surface is a second-class
one for decorations. `docs/SPIKE-SESSION-LIST.md` measured whether it could be a
plugin and answered *yes, on three conditions*: a styled-span line node, a cursor
that stays kernel state, and a render triggered by events rather than a poll.

**Decision.**

- **The rows are a view tree; the list widget is not.** `ui::project_list` splits
  into a geometry step (`resolve_items` — which rows exist, what a group header
  says, how much of the agent's text fits) and a presentation step
  (`session_item_node` / `session_list_tree`, geometry-free). The native pane
  paints its nodes through the *same* inline walk `ui::plugin_pane` uses
  (`line_spans`), so a `Fill`'s residue is resolved by one implementation in both
  panes. Its ratatui `List` is left alone, because its sticky scroll offset, its
  two-line items and its click hitboxes are derived from the offset that widget
  actually used.
- **The cursor stays kernel state.** Each published row says whether the cursor is
  on it; the plugin receives no keys and owns no selection. This is the spike's
  second condition, and it is what makes the third one survivable — see below.
- **The whole list is read under the `sessions` grant.** No new capability: the
  capability's sentence is already "read the sessions thurbox is running", both
  readers answer the one question a user is asked, and a pane that draws the
  session list must not have to demand two grants to draw one pane. The
  capability's documentation now states that the grant covers every session and
  not only the active one, because the disclosure widens and a capability list is
  only honest if it says what it discloses.
- **thurbox's own working spinner is declared motion (ADR-V18's first bundled
  consumer), in both trees.** The native pane declares the same ten-frame,
  8 fps, keyed `cycle` the plugin does and resolves its frame through a
  `FrameTable` filled from the clock it already ran on. The alternative — native
  keeps a resolved glyph, the plugin declares a motion, and the comparison is
  "equal up to the spinner" — would have exempted the one part of the pane that
  moves from the only oracle the port has.

**Consequences.**

- **The drawing surface needed nothing new.** No node kind, no style field, no
  style token, no capability — one reader was added, under the grant that already
  covered it, which is the shape every port has had. The pane is four node kinds — `list`, `line`, `text`,
  `fill` — every one of which predates the port, and
  `the_host_surface_needed_no_new_node` asserts it rather than leaving it as a
  claim in a document. `Fill`, added by ADR-31 for a diff row's tint, is what a
  selection bar and a group header's rule need here: its second consumer, and the
  first evidence it was a general node rather than one pane's escape hatch.
- **A plugin's view of kernel state is one render interval stale.** The render
  worker polls on a fixed 1 s cycle, so the spike's third condition does *not*
  hold and its 5 ms selection-latency bar is missed by ~200× — for the plugin's
  copy of the cursor. The cursor a user drives is kernel state, so the highlight
  they watch moves in the frame the key was handled; had the plugin owned its
  cursor, that would be the only number. This is a property of the host rather
  than of this pane, and closing it is a change to the render loop's contract with
  a rate policy attached (`PHASE4` §13 measures both candidate closures and why
  neither belongs to a pane port).
- **The native pane's animation does not take a lease.** Registering it in
  `App::motion` would put thurbox's own spinner into the bounded aggregate rate
  that plugin leases share, letting an installed plugin degrade it. The frame
  table gives equality without moving the budget.
- **Two new vocabulary rows, both chrome.** The empty state is drawn *centred* and
  no node carries an alignment; the pane's border carries a per-session status
  strip and clipped-row indicators, and nothing describes an overlay on a pane's
  frame. The second is the third consumer of the frame-node row `PHASE4` §9 opened.
- **One enumerated paint divergence**, pinned in both directions: a blank cell's
  foreground before the agent's text (a `Span::raw` leaves it unset; a token-less
  run resolves the theme's primary). `assert_same_ink` grants the latitude only on
  a cell with no glyph in it, and a second test fails if it stops being needed.
- Still no formatter. After five ports `PHASE4` §7's `thurbox.format.*` case is
  made by exactly one pane.

## ADR-34: A plugin pane's keys are addressed, not enumerated

**Context.** Phase 4 reproduced five native panes as plugins and every one is
read-only, because the input model is thinner than a native pane's keyboard.
Thurbox's own panes get scoped keys: `session::Action` carries a `KeyContext`, so
a plain `j` means "next task" in the tasks pane and "move down" in the file
viewer, both rebindable in the F1 editor and persisted to `keybindings.json`. A
plugin pane received a **raw key name** and decided for itself what it meant —
unrebindable, invisible in F1, absent from the keymap. Manifests already carried
`[[keybindings]]` entries; nothing read them.

The obvious closure is `KeyContext::Pane(String)`, and ADR-28 had already rejected
the neighbouring idea (one generated `Action` per discovered pane) because the
keybinding namespace, and the F1 editor's row indices, must not depend on which
plugins are installed.

**Decision.**

- **The binding's address is its scope.** A keymap entry is addressed
  `(plugin, pane, id)` and is active only while that pane is focused. No
  `KeyContext` member is added: the enum is `Copy` and matched by value in four
  places, no kernel action could ever carry the new variant, and the scope would
  then be stated twice — once by the variant, once by the address — with two
  places to drift. `contexts_overlap` is untouched; what is added is one rule in
  one function (a pane binding overlaps a **global** action and its **own** pane's
  bindings, nothing else). `pane:<id>` survives as the *displayed* scope: the
  editor's section title and the persisted key's prefix.
- **One keymap, two tables.** `KeyBindings` holds the pane bindings beside the
  `Action` map, so the F1 editor indexes one ordered list, one file has one
  writer, and conflict detection lives in the type that knows about conflicts.
- **The collision rule is asymmetric, and that is the point.** A user's rebind
  **steals** a chord in either direction. A plugin's **manifest default** that
  collides is **dropped**, leaving the binding unbound, and reported. Installing a
  plugin must not silently move a key the user already uses; a user asking for a
  key is an instruction. A kernel action also wins the lookup, deterministically,
  because those chords are how a user leaves a pane.
- **The keymap half is ungated; only delivery is behind `plugins`.** The editor
  rewrites the whole file on any edit, so a build with no plugin host that dropped
  the entries it did not understand would silently discard bindings set in another
  build. An override for a binding nothing declares is therefore *kept* rather
  than dropped — the one direction in which this differs from `from_json`'s
  existing treatment of an unknown action.
- **A manifest default is never persisted.** Writing it would freeze it: a plugin
  changing its own default would never reach a user who had once opened the
  editor. The file holds the user's choices, and registration consults them.
- **Delivery carries the binding id beside the raw key.** `onKey(paneId, key,
  binding)`, `binding` nil when the chord resolved to none. Not a second handler
  (two answers about consumption for one keypress) and not *instead* of the key (a
  pane collecting text needs the keypress, and a plugin declaring no binding must
  keep working). A plugin switching on `"delete-row"` rather than on `"d"` is what
  makes a rebind cost no plugin change.
- **Not the command registry.** ADR-V21's `{ command, key, context }` shape
  dispatches to the plugin's **service** half — a different VM, with no pane state
  and no "was it consumed" answer — so `j` would move a cursor living in the other
  VM. The command surface stays the answer for a name-addressed invocation.
- **The chord grammar is validated in the manifest.** `session::keybindings` is a
  sibling module of the same pure-data layer, so a typo becomes a discovery error
  naming the plugin, the binding and the chord, rather than a warning in a log no
  plugin author reads. A binding naming an unknown pane, or declared without
  `input`, is refused the same way `PaneWithoutRender` already refuses a pane that
  could never draw.
- **A dropped default is reported by `plugin doctor`, not by a toast.** It happens
  while plugins start, before anyone is watching, and it is a property of a
  configuration rather than an event. `doctor` re-derives it from the manifests
  plus the user's file, starting no VM — the same rule the spawn section follows,
  and it keeps "why does my key do nothing" answerable with no TUI running.

**Consequences.** A binding still *does* nothing on its own: it tells a plugin its
key fired, and what a plugin may change is the capability question. No bundled
plugin declares `input` or a keybinding in this change, so every insta snapshot is
byte-identical and no native pane moved.

## ADR-35: A plugin may change five things, and the kernel still fires automations

**Context.** Every host power a plugin held was a read. The published snapshot
carries sessions, metrics, automations, tasks, files and the open review, and the
only writable thing in a plugin's environment was its own key/value namespace. So
the five panes reproduced in Phase 4 can *draw* thurbox's panes and none of them
can *be* one: the tasks pane cycles a status and deletes with two keys, the
automations pane toggles, runs and deletes with three, and a plugin had no binding
for any of it — not a refusing one, none at all.

This is the widest grant the host has added, so the shape matters more than the
mechanism.

**Decision.**

- **A closed list of five operations**, chosen by one rule: *one operation per key
  a native pane performs with a single keystroke.* Set a task's status, delete a
  task, set an automation's enabled flag, run an automation, delete an automation.
  Each addresses one existing record by the id its reader already published and
  reports whether that record was there.
- **The three keys with no binding are the ones that open a kernel surface.**
  `n`/`e` open the central-pane editor, and tasks' `r` opens the trigger-time
  picker that spawns a session. A `createTask(title)` binding would let a plugin do
  something the key does not do while still not reproducing the key, so it waits
  for whatever ports the editor. Recorded here rather than discovered at port time.
- **Two capabilities, per record kind** (`tasks-write`, `automations-write`), for
  the reason the readers are split: the declared set is what an install prompt is
  written from. Neither implies the matching read, or the reverse — a pane that
  only draws the task list must not hold the power to delete.
- **A plugin asks the kernel to run an automation; it never runs one.**
  `runAutomation` marks it due and returns, exactly as the native pane's `r` does,
  and the kernel's scheduler fires it under the claim-CAS that already
  de-duplicates a TUI and a headless tick. So the plugin thread spawns no process,
  and parity is by construction rather than by imitation. Marking a pending run
  again is idempotent, because the write is `next_run_at = now` and not a queue.
- **The residual reach is stated, not glossed.** An `Exec` automation runs a shell
  command **the user wrote**, and this capability can cause one to run. The bound
  is that a plugin can neither author nor edit an automation, so the set of
  programs it can trigger is exactly the set already scheduled — a real mitigation,
  but not "this cannot run code". One asymmetry survives: a user presses `r` once
  and a plugin may call at its render cadence; the ceiling is one fire per kernel
  pass. A rate limit was rejected for now — it would put a clock and per-plugin
  state into a seam that has neither, and if one is added it belongs with the
  host's other execution bounds.
- **The seam mirrors the plugin store**, because the same two constraints apply:
  `plugin` may not import `storage`, and a connection cannot cross threads. A
  trait in `session::plugin_mutations`, one implementor in `storage::plugins`, a
  factory each VM invokes on its own thread. **One trait for both capabilities**:
  the trait is host-side plumbing and the grant surface is which *bindings* are
  inserted, so two traits would double the factory threaded through five call
  sites to express a distinction the capability check already makes.
- **Rejected: routing a mutation to the UI thread** as a request/reply, the way a
  key is routed. It would make a write depend on a frame happening, for an answer
  the UI does not need. **Rejected: `thurbox-cli` as the mutation surface** — it
  would grant process execution to get a status change.
- **Enabling shares one implementation with the native pane.**
  `Database::set_automation_enabled` deliberately leaves `next_run_at` to its
  caller, so a plugin calling it directly would leave an *enabled* automation with
  no occurrence: subtly dead. The recompute moved into
  `set_automation_enabled_rescheduled`, which the pane now calls too — otherwise "a
  user toggled it" and "a plugin toggled it" are two behaviours with one name.
- **A pane's VM holds a host power for the first time.** `PluginThread::spawn`
  passed `None` for the store, so a view half had readers and nothing else; the
  writer factory is threaded into `PluginHost` as well. Worth noticing rather than
  doing quietly: the review question for a bundled pane changes, even though the
  grant is still per manifest and still absent by default.

**Consequences.** A plugin's write is recorded exactly where the kernel's own is —
a task's audit entry, an automation's run history — with no plugin-specific trail
to drift, and it reaches the panes through the `data_version` poll that already
carries every external change. No bundled plugin declares either capability, so
snapshots do not move and the teardown gate is untouched. A pane replacement can
now reproduce `Space` and `d` (and automations' `r`); `n`, `e`, tasks' `r` and `o`
are still open, and `docs/PHASE4-PANE-READINESS.md` §10's wall — a plugin cannot
move the cursor, take focus or switch sessions — is unchanged.
