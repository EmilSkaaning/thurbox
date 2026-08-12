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

## ADR-36: A click on a plugin pane is a row, and focus names the pane it hit

**Context.** Every native pane is clickable by one mechanism: a renderer returns
`ui::RowHitbox`es, `App::view` records them as `ClickAction`s, and
`handle_mouse_click` hit-tests them, with the pane's whole-rect `FocusPane`
recorded after the rows. A plugin pane recorded **no click target at all** — a
click did not focus it, did not reach the plugin, and did not even highlight under
the pointer. `plugin-host/input` shipped with "no mouse" as a stated non-goal, and
a replacement has to behave like the pane it replaces.

**Decision.**

- **A click resolves to a row of the pane's outermost list**, 1-based — the same
  numbering `ui.list`'s `selectedRow` uses, so the number a plugin sends out to say
  where its cursor is, is the number it gets back when a row is clicked. A nested
  list contributes nothing (one click would have two answers) and a tree with no
  list has no rows (a column of lines is not a list of rows).
- **The rows come from the paint, not from a second walk.** `render_tree_rows`
  reports the rects `render_stacked` actually drew, because the kernel windows a
  list that names its cursor (ADR-30) and a recomputing hit-test could resolve a
  click against a layout other than the one on screen. The reported index is
  therefore **list-space**, not screen-space: a plugin never learns the window, so a
  screen position would be a number it cannot interpret.
- **A click carries a row and nothing about geometry** — no coordinate, no rect, no
  width, no height. The model has refused a plugin its geometry four times
  (ADR-26, ADR-29, ADR-30, ADR-31) and a click is not the reason to stop.
- **It rides the key channel.** `PluginInput` is `Key { key, binding } | Click { row }`
  on one bounded request, because the two have identical timing requirements: the UI
  thread must know *now* whether the event was consumed, and a second channel would
  double the worker's select arms and shutdown paths for no difference.
- **`onClick` is a separate handler, unlike a key's binding.** That looks
  inconsistent with ADR-34 and is not: a binding is *the same event* as the key that
  produced it, while a click is a different event with no key to report — folding it
  into `onKey` would need a sentinel key name, the shape ADR-34 rejected.
- **Focus names the pane it landed on.** `InputFocus::PluginPane` says *a* plugin
  pane holds focus and cannot say which, so `App::focused_plugin_pane` records the
  pair and `focusable_plugin_pane` consults it. Two consequences: a click sends the
  keys after it to the pane the user pointed at, and a second focusable pane becomes
  usable at all. Rejected: **a payload on the `InputFocus` variant** — it is a
  `Copy` enum compared by value in dozens of places, for a distinction only the host
  cares about. The remembered pair is **validated on every read**, so a pane that
  vanishes under the pointer (hidden, reloaded away, its plugin stopped) cannot keep
  focus and take the keys with it.
- **`ClickAction` gives up `Copy`.** The pane is addressed by *name*, because the
  pane set is replaced whenever a plugin reloads and an index recorded last frame
  could mean a different pane by the time it is clicked — the same reason every
  other pane write is keyed by name.
- **A pane whose plugin never declared `input` is not focused and told nothing.** A
  click is input, gated by the capability that gates input rather than by one of its
  own.

**Consequences.** Hover highlighting follows for free, since it runs off the same
recorded targets. What stays open is recorded rather than implied: the **wheel** (a
plugin's list has no offset the kernel owns, only a selected row it declares, so a
wheel tick would have to become "the plugin was asked to move its cursor" — a
keyboard question in a mouse costume), the **scrollbar** (still no track,
`docs/PHASE4-PANE-READINESS.md` §9), and the **focus ring visiting each pane in
turn** (a keyboard decision that belongs with ADR-28's picker; before this change
the distinction could not even be expressed, because nothing named a pane). No
bundled plugin declares `input`, so no bundled pane is clickable yet and every insta
snapshot is byte-identical.

## ADR-37: A pane is handed over only in the build a user installs

**Context.** Phase 4 has reproduced five native panes as bundled plugins, each
asserting tree equality against the pane it copies. The next step is the handover:
stop drawing the native pane, delete its renderer, let the plugin be the pane. The
info panel was picked to go first because it is a pure display surface — no
selection, no keys, no mouse — so nothing about interaction can confound the
question "does dropping a native renderer leave every frame identical?".

It cannot be done, and the reason is not about the info panel. A bundled pane is a
Luau program; running one needs `mlua`; `mlua` is optional (`Cargo.toml` reads
`default = []`); the `plugins` CI job asserts the default dependency tree contains
no `mlua`; and `release/workflow-invariants` **specifies** that the release
workflow never builds with the feature, enforced by
`scripts/dev/lint-workflows.sh`. So no installed binary can draw a bundled pane.
Deleting `src/ui/info_panel.rs` would leave `F2` opening an empty column on every
release while `cargo nextest run --all --features plugins` stayed green — the
failure absent from the build that ships and invisible in the build that is tested
hardest.

Worse, the gate built to prevent this permitted it. `tests/teardown_gate.rs`
derived a pane's readiness from two conditions — the plugin exists, and
`src/app/view.rs` no longer names the native renderer — and the deletion above
satisfies both. The row would have been recorded ready and
`every_listed_path_survives_until_its_unit_is_ready` would have stopped protecting
the renderer. That is the silent case the gate's own module note says it exists to
catch.

**Decision.**

- **Handover has a third condition: the runtime that draws the replacement
  reaches the build a user installs.** A pane whose replacement only runs behind a
  compile-time feature releases do not enable is not a pane a user has, so
  deleting its native renderer removes what users see — the outcome the inventory
  exists to prevent.
- **The condition is read from `Cargo.toml`'s default feature list**, not from
  `cfg!(feature = "plugins")`. The `cfg!` answers "was this test binary built with
  the feature", which under `--features plugins` is `true` — exactly the answer
  that would permit the deletion. Reading the manifest keeps one verdict in both
  configurations, which is the premise the whole gate rests on.
- **It applies to every pane row, not per pane.** It is a fact about the build, so
  one release decision blocks all seven rows together. Stating it once makes that
  dependency visible instead of letting it read as seven independent pane
  problems; `docs/PHASE6-TEARDOWN-READINESS.md` §4's worklist is corrected to
  match, since it previously ordered the seven handovers *before* the Cargo
  default flip they each require.
- **No pane is handed over and no renderer is deleted.** The info panel's plugin
  stays `default_visible = false` and the native pane stays what `src/app/view.rs`
  draws.

**Rejected.** *Flip `plugins` into the default feature set* — the honest
resolution, and a release-engineering change with its own measurement: it raises
the effective MSRV from 1.86 to 1.88 (`mlua`'s floor, which cargo cannot express
per feature), puts vendored Luau C sources in the path of four release targets
including a cross-built `musl` and a cross-compiled `aarch64-apple-darwin`, and
contradicts a specified release invariant plus a required CI assertion. Rejected
*for this change*, not on the merits, and taken with that measurement in
[ADR-40](#adr-40-the-plugin-runtime-ships-in-the-build-a-user-installs-stage-b) —
which also corrects two details of this paragraph: the sources are C++ rather than
C, and `aarch64-apple-darwin` is built natively on an arm64 runner rather than
cross-compiled. *Ship the
pane's replacement ungated* — vacuous: an ungated Luau program needs an ungated
VM, and rewriting the plugin in Rust to dodge the gate reproduces
`src/ui/info_panel.rs` under a new name while destroying the only thing the port
measures. *Select between the two renderers on the feature* — nothing is handed
over, and it leaves two renderings of one pane that differ by build, comparable as
trees but never as the frames a user sees. *Delete the renderer and accept the gap
until the flip* — that ships a release whose `F2` opens an empty framed column
whose settings flag toggles nothing.

**Consequences.** The seven pane rows stay blocked, now for a checked reason
rather than a remembered one, and the block names a release decision instead of
pane work. Three pane-level handover requirements the release blocker would have
hidden are recorded in `docs/PHASE4-PANE-READINESS.md` §14 rather than closed
speculatively: a plugin pane cannot be seated in the info panel's region
(`PaneSlot` has only `Right`), cannot answer `Action::ToggleInfoPanel` or ride
`[features] info_panel`, and renders on a fixed 1 s poll — which for the pane with
live CPU gauges and countdowns would make the *user's* pane the stale one. And one
correction to the proof a handover is expected to offer: the acceptance snapshots
cannot witness this pane, because all seven were captured with no active session
and the pane renders nothing without one. The oracle that can fail is
`tests/bundled_info_panel.rs`'s tree equality, which is what the port already
relies on.

## ADR-38: A list's cursor is an anchor, not an appearance — and a pane's keys need both

**Context.** The tasks pane was to be the first native pane *replaced*: bring the
bundled `tasks` plugin to parity with all ten of its `KeyContext::Tasks` actions,
then delete `src/ui/tasks_panel.rs`. Two things came out of attempting it.

The first is a rendering gap that could be closed. `docs/PHASE4-PANE-READINESS.md`
§8 left this pane two geometry divergences; ADR-30 closed the second one's
*mechanism* for every pane — a list node names the row its cursor is on and the
kernel windows it — and the tasks pane never took it up, because the published
task section carries no cursor index. Its per-row `selected` flag cannot serve as
one: that flag is gated on the cursor being *visible* (the pane focused, or a
search preview moving it), so a pane anchored on it would jump back to its first
row whenever thurbox's own pane lost focus.

The second is that the pane's **keys** cannot be ported, and not for the reason
the attempt expected. The hard cases were assumed to be the two separate surfaces
its keys open — the central-pane editor (`n`, `e`/`Enter`) and the trigger-time
action picker (`r`). Those are unportable, for three walls each. But the port
fails on the *first* key: `j`/`k` move `App::task_ui.task_panel_index`, which is
view state, and the kernel-state channel is read-only by construction. And the two
keys that need no new host power — `Space` and `d`, both already expressible as
the record writes ADR-35 granted — cannot name the row they would act on: a plugin
receives keys only while one of its own panes holds focus, and
`App::build_tasks_snapshot` marks a row as the cursor's only while the **native**
pane holds focus. The input path and the cursor path are disjoint.

**Decision.**

- **The task section publishes a cursor index, separate from the per-row selected
  flag.** The index is the *anchor* — which row a list scrolls to — and is
  published whatever holds focus. The flag is the *appearance* — this row is drawn
  as the cursor's — and stays focus-gated. This is ADR-30's rule (the anchor is
  the list's, the appearance is the run's) with its second consumer, so it is a
  reuse rather than a design.
- **The native pane's tree carries every row plus that cursor**, and the
  *renderer* resolves the window through `ui::file_viewer::visible_window`. Both
  panes therefore scroll through one implementation; the pane calls the same
  helper again for its click hitboxes, which need the window as numbers rather
  than as a paint. Mirrors `ui::file_viewer::render`.
- **Out-of-range is two cases, resolved differently.** A cursor past the end of a
  *shortened list* clamps, because that is what the native pane has always done
  and the two must window alike. A cursor past the *published bound*
  (`MAX_TASK_ROWS`) is not published at all, because an anchor into rows a pane
  never received would window it to nothing — the rule the file section already
  states.
- **No key is declared and the native pane is not replaced.** The bundled plugin
  keeps `capabilities = ["render", "tasks"]` and `default_visible = false`.
- **The input verdict is a gate, not a paragraph**
  (`tests/tasks_pane_input_gap.rs`), one probe per missing host power, each tagged
  structural or vocabulary, in the shape `tests/global_search_pane_gap.rs`
  established.

**Rejected.** *Declare `input` plus the two record-write keys anyway* — the pane
would answer `Space` against a row it draws no cursor on, and a key that acts on
an invisible row is worse than a pane that takes none; `plugin::keymap` already
refuses to publish a binding the host could not deliver, for the same reason.
*Publish the row as selected while a plugin pane holds focus* — it would mark a
cursor no key can move, and the rule would apply to every plugin pane reading
tasks, which is designing an appearance rule from one blocked consumer. *Let the
plugin own its own cursor in its own state* — its cursor and the kernel's would
disagree, so `o`/`r`/`e` would act on a different row than the one highlighted,
and the tree equality that is a Phase 4 port's deliverable could not be written.
*Publish rows already windowed* — refused for the fourth time (ADR-26, ADR-29,
ADR-30): the publisher has no height, and the plugin's pane is a different rect in
the same layout. *Report the resolved rect into the plugin* — the same request, and
the same answer: rendering would become width-dependent, so a resize must re-enter
the VM before the frame that needs it.

**Consequences.** The plugin's copy of the pane now scrolls where the native one
does, so its claim is the file viewer's stronger one — the same painted **frame**
at a height where the pane scrolls, not only an equal tree. `ui::tasks_panel`
consults a width and never a height. One rendering divergence is left, and it is
the same one three ports have now recorded: no node clips with an ellipsis, so a
title too wide for the column loses its `…` in the plugin's copy. And the pane's
keys stay the kernel's, with the reason checked rather than remembered: the
tasks-plugin teardown row stays blocked — for a second, independent reason on top
of ADR-37's — and the next person to try a handover meets the finding here rather
than rediscovering it at `j`.

## ADR-39: A scroll track is a list's declaration; the file viewer's keys are not portable

**Context.** The file viewer was to be the next native pane *replaced*: bring the
bundled `file-viewer` plugin to parity with all seven of its
`KeyContext::FileViewer` actions, then delete `src/ui/file_viewer.rs`. As with the
tasks pane (ADR-38) the attempt produced one closable rendering gap and one wall,
and here the wall is higher.

The rendering gap is the scroll track, recorded as *divergence 2* by
`tests/bundled_file_viewer.rs`. The native pane reserved its rightmost column
through `ui::scrollbar::reserve_track`, painted its rows into what was left, and
drew the thumb itself — all *outside* the tree — so a plugin pane had no track at
all. That test recorded the closure as Phase 6's business because the reservation
sat outside the tree; the objection turns out to be answerable, because moving the
reservation into the renderer leaves the native pane's rows in the rect they were
already painted into.

The wall is that **every one of the pane's seven keys writes view state**, and two
of them need powers the vocabulary does not define at all: expanding a directory
*reads it* (`FileViewerState::activate` → `read_dir_sorted`), and expanding a file
*launches the configured editor*. Unlike the tasks pane there is not even a partial
key surface — no file-viewer key is a record write. And the `/` sub-mode cannot
meet the parity bar in principle: `App::focus_key_context` returns `Global` while a
search is active so that every character types into the query, which is the
opposite of "rebindable, and in the F1 editor".

One structural fact is new, and it changes what deleting the pane would mean:
**`src/ui/file_viewer.rs` is the pane's model, not only its renderer.**
`FileViewerState` lives there, `App` owns one, and `App::build_files_snapshot`
reads it — as does `visible_window`, the rule every *plugin* list is scrolled by
(ADR-30) and four other native panes window with.

**Decision.**

- **`ViewNode::List` carries a `scrollbar` flag.** A list declares *that* it
  scrolls; the kernel reserves the rightmost column through the same
  `reserve_track` every native pane reserves with, draws the thumb at the declared
  cursor, and lays the rows out in what remains. This is ADR-26's trade (the kernel
  resolves the geometry, the plugin declares the intent) applied to one column, and
  it is the fifth time reporting the resolved rect into a plugin was refused.
- **It is not inferred from `selected`.** `ui::tasks_panel`,
  `ui::automations_panel` and `ui::project_list` all draw selectable lists that
  overflow *without* a scrollbar, so inferring a track would put one into three
  panes that deliberately have none — and would move their frames.
- **The drawing lives in one place.** `ui::scrollbar` gains `draw_into` (buffer,
  for the tree renderer) with `render_into` as a `Frame` wrapper over it, and
  `geom_for` (the recorded drag target, without drawing). The native pane keeps
  calling `reserve_track` for the numbers only it can answer — its row hitboxes and
  that drag target — which is the arrangement it already had for `visible_window`,
  and for the same reason.
- **A track with no cursor draws at position 0** rather than being refused: whether
  a cursor is published belongs to whatever the pane reads (the file section drops
  its cursor past `MAX_FILE_ROWS`), and a node shape that changed with it would
  break the equality a port is measured by.
- **`Capability::Files` is not widened.** The port was specified as needing more,
  and the measurement says the missing parity is *powers, not facts*: a path is
  only needed in order to act on a file and acting is a process launch; contents
  are only needed to preview one, which this pane never does; and the query is
  drawn only inside a bar the host surface cannot describe.
- **No key is declared and the native pane is not replaced.** The bundled plugin
  keeps `capabilities = ["render", "files"]` and `default_visible = false`, and the
  input verdict is a gate (`tests/file_viewer_pane_input_gap.rs`), one probe per
  missing power, tagged structural or vocabulary.

**Rejected.** *Two `reserve_track` call sites, one per pane* — the arrangement
ADR-30 rejected for the scroll window, one column over: nothing would force them
to agree. *Draw the track as host chrome around a plugin's tree* — the host does
not know a list's length or its cursor without walking the tree it was just
handed, and every plugin pane would get a track whether its author wanted one or
not. *Grant a filesystem capability so a plugin could expand a directory* — it
would be the widest grant in the host, `tests/teardown_gate.rs` reserves the name
for a different v1 power, and it is still not sufficient (ADR-30: expansion state,
cursor and search verdict are the user's and the kernel's, so a plugin holding
`read_dir` could draw *a* file tree but not *this pane*). *Let the plugin own its
own cursor, expansion set and query* — two file trees with two cursors in one
interface, whose `/` would filter nothing, and no equality test could be written.
*Lift `FileViewerState` out of `ui` now as preparation* — motion without a
destination: the pane cannot be handed over even with the model moved, so moving
a state machine between modules to enable a blocked deletion is churn whose only
proof is that the tests still pass.

**Consequences.** The plugin's copy of the file viewer is now byte-identical to the
native pane inside the frame the host draws — asserted as the same painted frame,
thumb column included, and shown non-vacuous against a render without the
declaration. The one divergence left inside the pane is the search **bar**, which
is drawn *outside* the pane's block and so needs the pane-chrome row PHASE4 §13
records rather than a node. A plugin pane's track is an **indicator**: the thumb
reports a cursor the plugin does not own, so no drag target is recorded for it —
one more consequence of the missing view write, pinned in the gate beside the
others. And the file-viewer teardown row stays blocked for three independent
reasons now: ADR-37's build, the view write, and the module that is the model.

## ADR-40: The plugin runtime ships in the build a user installs (Stage B)

**Context.** ADR-37 stopped the first pane handover on a finding that was not
about the info panel: nothing a plugin draws reaches a released binary.
`Cargo.toml` read `default = []`, a required CI step asserted the default
dependency tree carried no `mlua`, and `release/workflow-invariants` *specified*
that `cd.yml` never builds with the plugin feature. A bundled pane is a Luau
program, so handing any pane over would have deleted it from every install while
the `--features plugins` test run stayed green.

ADR-37 recorded flipping the default as its rejected alternative, "the honest
resolution, and a release-engineering change with its own measurement", and
`docs/PHASE6-TEARDOWN-READINESS.md` §4 placed it upstream of all seven handovers.
This is that change, with the measurement.

**Decision.** `plugins` joins the default feature set. Four consequences follow,
each handled rather than discovered:

- **`rust-version` rises 1.86 → 1.88**, with `clippy.toml` following. `mlua`
  declares 1.88 and cargo cannot express a per-feature minimum; with the runtime
  in `default`, its floor *is* the crate's floor, so the workaround the manifest
  comment described is gone rather than restated. Four documents claimed 1.75,
  which `ratatui 0.30` had already made false. An MSRV rise is also a **lint**
  change: `clippy::manual_is_multiple_of` is msrv-gated (`u64::is_multiple_of`
  stabilised in 1.87), so eight `% N == 0` tick-cadence checks began failing
  `-D warnings` and were rewritten rather than allowed. That, not the runtime, is
  the whole `src/` logic diff.
- **The CI assertion inverts and stays required.** The `plugins` job asserted the
  default tree *excludes* `mlua`; it now asserts it *includes* it, which is the
  fact `tests/teardown_gate.rs` reads from `Cargo.toml` to decide whether a pane
  may be handed over. The job also gains the configuration nothing else covers
  any more — `--no-default-features`, the fallback for a platform where the
  vendored C++ will not build — and its pinned 1.88 toolchain stops being a
  workaround and becomes a real MSRV floor check.
- **Release invariant 2 is replaced, not deleted.** "cd.yml never builds *with*
  the plugin feature" is not merely obsolete: after the flip no release job asks
  for the feature and every release binary contains it, so the check would report
  `ok` about exactly what it claimed to forbid. It is removed with its reason and
  replaced by its inverse — never build *without* the runtime, rejecting
  `--no-default-features` and a manifest edit to the default list — because the
  hazard reversed direction. A handed-over pane is drawn by the runtime, so a
  release that drops it ships an empty column.
- **The bundled example pane is seeded hidden.** `PaneDecl::default_visible`
  defaults to `true`, which is right for a plugin an author installed and wrong
  for one that arrives inside the binary. `hello` omitted the seed, which nobody
  could see while no installed binary ran a plugin at all; in a default build it
  would open a demo pane in every fresh install's right column.
  `tests/bundled_manifests.rs` holds the rule for the whole bundled set.

**The measurement**, since a release that fails to build on one platform is the
worst outcome available here. `mlua` vendors Luau as **C++17** sources
(`luau0-src`, `.std("c++17").cpp(true)`), so every release target needs a C++
compiler and, cross-compiling, a C++ standard library for that target — where
before it needed only the C compiler `rusqlite`'s bundled SQLite already required.

| Target | Verified | How |
|---|---|---|
| `x86_64-unknown-linux-gnu` | yes | local release build |
| `x86_64-unknown-linux-musl` | yes | local release build against a `musl-cross` GCC with C++, the toolchain `cross`'s image supplies as `CXX_x86_64_unknown_linux_musl`; static-pie, Luau linked in |
| `x86_64-pc-windows-msvc` | no | no MSVC off Windows. The nearest proxy — a local `x86_64-pc-windows-gnu` cross-build with mingw `g++` — passes, so the sources survive a Windows ABI; `cl.exe`'s dialect and standard library stay untested |
| `aarch64-apple-darwin` | no | needs macOS and the Apple SDK. It is a **native** build on the `macos-14` arm64 runner, not a cross-compile as ADR-37 stated |

On the default target the runtime costs **+2.44 MB** on `thurbox` (12.02 → 14.46
MB) and **+2.40 MB** on `thurbox-cli` (9.85 → 12.24 MB), and **~60 s** of
build-script time compiling vendored Luau — next to the ~72 s the release already
spends on bundled SQLite. So the artifact-size premise the Luau choice rested on
holds: a few megabytes, not the tens a bundled JavaScript runtime would have cost,
and no packaging channel (`packaging/`: brew, AUR, Chocolatey, winget) changes.

**Why now, and why as its own change.** Stage B was always a release decision
rather than pane work, which is why ADR-37 refused to take it inside a pane port.
Taking it separately means the MSRV rise, the inverted assertion, the replaced
invariant and the four-target measurement each get argued once, in a diff whose
only `src/` edit is one manifest line — so a bisect over a pane handover never
also lands a release-engineering change.

**Rejected.**

- *Keep `plugins` optional and hand panes over anyway* — the deletion ADR-37
  refused: an empty column on every install, green in the only build that can
  draw the replacement.
- *Select between the native and plugin renderer on the feature* — nothing is
  handed over, and two renderings of one pane that differ by build are comparable
  as trees but never as the frames a user sees.
- *Add a runtime `[features] plugins` flag defaulting to `false`*, as the earlier
  prose design set had it. It would gate nothing a user can see: with every
  bundled pane seeded hidden the host is additive — discovery over a directory
  that usually does not exist, no VM until a plugin is found. A switch whose only
  effect is to skip work that already costs nothing is a settings row that has to
  be explained.
- *Drop the `plugins` feature entirely* — that is Stage C. Keeping it is what
  makes `--no-default-features` a real answer for a platform whose toolchain
  cannot build the vendored C++, and the CI leg above keeps that answer compiling.
- *Delete release invariant 2 outright*, as its own header instructed for Stage C.
  Correct about the mechanism — a check should be removed in the diff that
  retires it, not switched off — but it would have dropped a property that had
  merely reversed. The removal is recorded with its reason and its inverse takes
  its place.

**Consequences.** Every install gains the plugin host: discovery, the
`thurbox-cli plugin`/`command` verbs, `F10`, and the bundled plugins materialized
under the data dir. Nothing is taken away — all seven native panes stay, drawn by
the same renderers — and no bundled pane appears unasked, so a fresh launch looks
like the launch before it. The seven teardown pane rows are no longer blocked by a
release decision: each is now blocked only by its own pane-level reason, which
`tests/teardown_gate.rs`'s
`the_build_condition_holds_and_still_gates_a_handover` asserts row by row. The
build condition stays checked rather than retired, because it is what a later
change removing the runtime from `default` would violate — at which point every
pane already handed over would empty with nothing failing. Two release targets
remain unverified by anything but the release build itself, which is stated here
rather than assumed away.

## ADR-41: An automation's summary crosses as parts; a plugin pane's cursor is its own

**Context.** Phase 4's sixth pane port is the automations pane — the last native
pane with no plugin at all — and it is the first that is not simply another list.
Three of its properties had no precedent:

1. its rows show a **composed string**, `<schedule> · <action> · <when>`, built
   from a schedule, an action and a countdown;
2. its **scroll anchor and cursor appearance come apart** — it windows to the
   cursor whether or not it holds focus and highlights it only when it does; and
3. its **keys act on records**. ADR-38 had recorded a pane's keys as kernel-owned
   because a plugin "cannot name the row the user is looking at", and
   `Capability::AutomationsWrite` had existed since ADR-35 with **no consumer**:
   no bundled plugin declared it, so nothing exercised the write seam from a pane.

**Decision.** Three rulings, one per property.

- **A composed display string is published as its parts.** The section carries the
  schedule's resolved **label**, the action's **wire name**, `enabled` and
  `due_in_secs`; the plugin composes the separator, the ordering and the three-way
  `when` precedence. Exactly two parts are resolved by the kernel, and for the
  reason `session::pane_context`'s header already gives: a cron expression's
  meaning is thurbox's own vocabulary, and a VM has no clock. This sharpens ADR-29's
  line — "publish the rendering only when two panes must agree about it" — which
  read literally said to publish this one, since `format_automation_summary` is
  shared by the pane and the `Ctrl+P` list modal. The rule's purpose is to stop a
  plugin re-deriving a mapping whose drift would be **invisible**; here the second
  consumer is a modal a plugin cannot reproduce and the plugin's composition is
  compared against thurbox's rule on every test run, so drift is loud. Restated:
  **publish a rendering when a plugin's copy of it would be unchecked.** The one
  rule lives in `ui::automations_panel::row_summary` and both native surfaces call
  it.
- **A list section's anchor and drawn cursor are two published facts**, confirming
  ADR-38 from a pane where the answers differ by design. `AutomationsSnapshot`
  carries `cursor` and `cursor_visible` — one flag on the section rather than a
  `selected` per row, which is where it diverges from `TasksSnapshot`: for this
  pane the whole cursor appears and disappears with focus, so a per-row flag would
  be one fact in `n + 1` places and a publication could highlight one row while
  scrolling to another. The index is **clamped**, which settled a pre-existing
  inconsistency: the pane clamped the index it windowed on and compared the
  unclamped one to pick the highlighted row, so a stale selection scrolled to the
  last row and highlighted nothing. The host refuses a list whose cursor is not an
  index into its children, so that state is not expressible by a pane at all; the
  appearance now follows the anchor.
- **A plugin pane's cursor is the plugin's own, and that is what makes keys
  portable.** ADR-38's "the input path and the cursor path are disjoint" is right
  about the *kernel's* cursor and too strong in general. One VM per plugin, retained
  across render and key calls, so a cursor is ordinary plugin state: `onKey` moves
  it, `render` hands it to `ui.list`, and the row the user is looking at is the row
  the plugin drew. No view write is involved — thurbox's cursor is untouched. So the
  bundled pane declares `input` and `automations-write` and ships five of the pane's
  seven keys, addressing each row by the **id** the section publishes. The two that
  do not ship are recorded with the power each needs: creating an automation (the
  write seam has no creation binding, by construction) and the central-pane editor
  (a seat `PaneSlot` does not offer, a focus a plugin cannot take, and text
  authoring the capability excludes).

**Two boundaries this port draws rather than crosses.**

- **The left column's circular wrap stays kernel-owned.** The native pane and the
  session list read as one list; every edge of that wrap is `App` assigning
  `self.focus`, which no capability writes. The plugin does the half it can — it
  **declines** the key at its edge — and the kernel's half is not implemented, so a
  plugin pane is a discrete focus stop and the key visibly does nothing there. That
  is the right answer rather than a hole because **a wrap is a claim about
  adjacency, and adjacency is layout**: the plugin's pane is in the right column,
  and a `j` there that jumped into the left one would be a lie about what is on
  screen. Rejected: having the plugin wrap its own cursor, which would ship a
  behaviour the native pane does not have under the word parity.
- **A pane is told nothing about its own focus.** Every published `focused` field
  describes the *native* surface being reproduced. Right for a read-only copy,
  wrong for a pane with keys: once the plugin has a cursor it draws it whether or
  not its pane is focused, and it cannot learn that focus left. It also costs
  **behaviour**: with no drawn cursor a write would act on whichever row thurbox's
  cursor was left on, so the plugin **refuses a write until a movement key has given
  it a cursor of its own** — the one interaction that differs from the native pane,
  and the honest answer to "never act on a row nobody is looking at". The closure is
  one published fact through `session::pane_visibility`'s existing mechanism, and it
  is deliberately not done here because it changes what *every* pane is told — a
  host change inside a pane port.

**Rejected alternatives.**

- *Publish the finished summary string.* Cheaper, and it would have made the port
  measure only that a plugin can concatenate. The summary is this pane's most
  information-dense element; if the kernel composes it, nothing is learned about
  what a third-party pane can own.
- *A second capability for the pane's list.* Two readers of the same records —
  "the due ones" and "all of them" — is not a distinction a user is protected by,
  and it would force the pane that draws the list to demand two grants. One
  capability, two readers, stated in its doc.
- *Have the keys act on the kernel's published cursor.* It reads as tighter
  coupling and is worse: that row is wherever thurbox's pane left it, so a user
  driving the plugin's pane would toggle a row they are not looking at — the shape
  ADR-38 was right to refuse.
- *Widen `PaneSlot` so the reproduction sits where the native pane does.* A change
  to the file that owns every pane's geometry, gated by ~40 layout tests, arriving
  in the commit meant to be evidence about a pane. Its cost is tabulated in
  `docs/PHASE4-PANE-READINESS.md` §17 and a test keeps a `left` manifest refused.

**Consequences.** `automations-write` has a consumer, so its central property is
asserted rather than only stated: `runAutomation` marks an automation **due** and
the kernel fires it, which matters because an `Exec` automation the *user* authored
runs a shell command — no plugin thread executes anything, and a plugin can neither
author nor edit one, so the reachable set is exactly what is already scheduled. Two
smaller findings came out of the keys: a `KeybindingDecl` carries one chord where a
kernel `Action` carries a list (the plugin declares the letter and handles the
arrows as raw key names), and the chord grammar could not spell the **space bar** at
all — `display` emitted a literal `" "` that `parse` trims away, so the default
chord of `AutomationsToggle` and `TasksCycleStatus` could not round-trip through
`keybindings.json`. That is fixed here, named in both directions. The native pane is
unchanged on screen and still what `src/app/view.rs` draws, so
`tests/teardown_gate.rs` keeps its row blocked.

## ADR-42: A port's oracle is recorded before the handover, or it dies with it

**Context.** The info panel is the pane chosen to be handed over first — pure
display, no keys, no mouse, and since ADR-40 the runtime that draws it is in the
build a user installs. `docs/PHASE4-PANE-READINESS.md` §14 had already found that
the *proposed* proof of such a handover could not fail (the seven acceptance
snapshots are all captured with no active session, and this pane needs one, so
none holds a cell of it) and pointed instead at the oracle that could:
`tests/bundled_info_panel.rs`, which asserts the plugin's view tree **equals** the
one `ui::info_panel::info_tree` builds.

That pointer was right about today and wrong about the day it is needed. The
assertion is **differential** — it names `info_tree`, which lives in the module
the handover deletes. So it can fail before the handover and not after it: with
the right-hand side gone, the repair that compiles is to drop the comparison, and
what remains is a test that the plugin renders without erroring — satisfied
equally by a pane drawing one wrong row and by one drawing twenty. Every one of
the six bundled pane oracles has this shape.

The failure mode is the same silent class ADR-37's gate exists for, one level up:
not a build that cannot draw the pane, but a **proof that stops constraining it**
at the moment it is relied upon, with nothing red anywhere.

**Decision.** A pane whose handover is planned must have its oracle **recorded**
before the handover, in a change that does not also perform it.

- The expectation is generated from the **native** builder, never from the plugin.
  A recording taken from the plugin — or taken after the native builder is gone —
  freezes whatever the plugin does as correct, defect included, and can never fail
  for the reason it exists.
- While both sides exist, **both edges are asserted**: the recording equals the
  native tree, and the plugin equals the native tree. The first is what gives the
  recording its provenance and is establishable only now; their conjunction is the
  fact the handover inherits.
- The recording is a **line-per-node rendering**, not `{:#?}`. Faithfulness was
  never the scarce property here — legibility was. A structural dump of this pane
  is thousands of lines of defaulted style fields, and an expectation nobody can
  read is one every update rubber-stamps, which converts the oracle into a record
  of whatever the code last did.
- Compactness is bought by omitting, so the renderer **destructures every
  view-tree variant and every style field by name**, with no rest pattern and no
  wildcard arm. Adding a field to the IR fails to compile in the oracle (E0027)
  until it is accounted for. The compiler keeps the format honest rather than a
  reviewer's memory.

**Rejected: keep the native builder alive as a test-only oracle.** Move `info_tree`
behind `cfg(test)`, delete only the renderer, and compare against it forever.
`migration/phase-4` already forbids this in terms — "A port MUST NOT satisfy this
by keeping both renderers", because that leaves two renderings of one pane — and a
builder nothing paints drifts in the direction that cannot be caught: a change
making it wrong makes the *oracle* wrong while the test keeps passing. It also
keeps 2,000 lines of pane alive to serve one test, which is the deletion the
handover exists to perform.

**Rejected: hand-write the expected tree.** A 25-row pane with six gauges, written
twice, drifting against nothing — and never *derived* from the native pane, so it
has no proven baseline at all.

**Consequences.** The info panel's handover gains a fourth requirement and
immediately loses it: §14's table now reads seat, toggle-and-flag, latency, and a
durable proof, with the last closed. Nothing else moved — `src/` is untouched, the
interface is byte-identical, and `tests/teardown_gate.rs` keeps the row blocked,
which is deliberate: the recording had to land in a change that does not also
delete what it records, or its provenance would be unprovable. The other five
bundled oracles keep their differential shape; each will need the same recording
captured while *its* native builder exists, which is work belonging to its own
handover, and `migration/phase-4` now states the rule so a port cannot miss it.

## ADR-43: A refused handover records what it needed, as a gate

**Context.** The left column's two panes were to be the next two handovers: the
**automations pane**, whose port had already shipped five of its seven keys
(ADR-41), and the **session list**, the pane ADR-V1 hinges on and the one
`docs/SPIKE-SESSION-LIST.md` measured, answering *yes, on three conditions*. Both
attempts stop, and the interesting part is that neither stops where its own prior
analysis expected.

The session list's spike named the right conditions and drew the wrong conclusion
from one of them. Its second condition — **the cursor stays kernel state** — is
correct, and is precisely what makes the handover impossible: a handed-over pane is
focused as `InputFocus::PluginPane`, `App::focus_key_context` names no arm for it,
so all six `KeyContext::SessionList` actions resolve in the global scope and none
fires. A plugin cannot substitute for them either, because `j`/`k` move the
**active session** — what the central pane, the info panel, the file viewer and the
code review are all showing — and no capability writes kernel view state. The
cursor cannot be kernel state *and* be driven by a plugin pane's keys. That is a
fact about the handover, which a spike about the port could not have seen.

The automations pane's port stopped at its **seat** and read as five-sevenths done.
It is not. Focusing the native pane is what turns the **central** pane into the
automation editor plus its run history, and `App::render_central_pane` selects that
view by testing `self.focus` against three *native* focuses. A plugin pane is a
fourth focus the branch does not name, so a handover removes the editor, the run
history and `Enter`-opens-that-run's-session — surfaces the pane does not draw and
a plugin cannot take. The two unported keys are therefore not a shortfall of two;
they are the pane's whole authoring surface, and the shortfall is a seat rather
than a key. It is the same central-seat coupling that blocked the tasks pane
(ADR-38), found on the pane whose keys were supposed to be the hard part.

**Decision.** Both native panes stay, and a refused handover records every
requirement it could not meet as a **gate that re-derives each one from the
source** — not as prose.

The reason is the one `tests/global_search_pane_gap.rs` established: a verdict
written in markdown is a fact about a build that expires without telling anyone.
"The session list cannot be handed over" stops being true the moment someone adds a
view write for an unrelated reason, and nothing would say so. So
`tests/session_list_pane_handover_gap.rs` (9 rows) and
`tests/automations_pane_handover_gap.rs` (10 rows) each hold one row per unmet
requirement with its probe, derive the verdict from the rows rather than stating
it, and assert both directions — today's answer, and a table where every row
landed. (The automations half was retired when that pane was handed over; its rows are
preserved in ADR-56, because none of the powers they named was granted.)

Each row is tagged by **why** it is missing, and a third kind joins the two the
earlier gates used:

- **structural** — a power a plugin is not given on purpose, whose reversal changes
  what a plugin is (both cursors, the central seat, the left seat, record creation,
  text authoring, the modules that are models);
- **vocabulary** — something the drawing catalogue cannot say and could (a centred
  line, an ellipsizing clip, chrome on a pane's border, a pending-spawn row);
- **wiring** — something the host could do today with no new plugin-facing
  concept: when a plugin is asked to render, which facts it is told, or how the
  host draws a pane it already knows is focused.

The third kind is not a courtesy. Filing the render trigger as structural would
claim the model forbids event-driven rendering, which it does not; filing it as
vocabulary would say the catalogue is short a word, which it is not. And the
ordering of the work follows from the kinds — wiring is cheapest, and the session
list's 1 s staleness stops being cosmetic the moment the pane is the one a user
navigates with.

**Two capabilities deliberately not added.** The brief for this work expected
`Shift+J`/`Shift+K`/`Shift+S` to be unblocked by a session-write grant, and the
automations pane's `n` by a creation grant. Both are the right *shape* — one
operation per single-keystroke effect, ADR-35's rule — and both are recorded as
rows instead. A session-reorder grant would be the **third** capability in the host
with no consumer, joining `input` before ADR-41 and `tasks-write`: the key it
enables still acts on the row the user is looking at, which for this pane is the
kernel's cursor, so the grant would widen a plugin's reach over the database while
the pane it exists for still could not use it. A creation grant has no id to
address at all, which is why ADR-35 excluded it. A capability whose consumer cannot
work is reach without parity.

**Consequences.** No `src/` change: both native panes are still what
`src/app/view.rs` draws, both bundled plugins keep exactly the capabilities they
had, and `tests/teardown_gate.rs` keeps both rows blocked — now for reasons that
fail a test when they stop being reasons. The gates read source text, so they run
and mean the same thing with or without the `plugins` feature, which is what lets
them sit beside the teardown gate rather than inside the feature-gated oracles.
Five of the nineteen rows are shared between the two panes (the left seat, the
render trigger, a pane's unknown focus, the module-as-model class), which is the
first evidence that the remaining handovers are blocked by a small number of host
decisions rather than by per-pane work.

What would unblock each, as an ordering rather than a list. **The automations
pane** needs its two seats: a `left` slot, whose load-bearing requirement is a
decision the protocol has so far refused — whether plugin content may size a
kernel region, since `ui::layout`'s `left_column` sizes this pane as
`(count + 2).clamp(3, 10)` — and then the central seat, which is the same question
the tasks pane's editor raised. Its keys are the least of its problems. **The
session list** needs the cursor question answered first, and there are only two
answers: either kernel view state becomes plugin-writable under a capability, which
makes "the active session" a plugin-writable thing and is the widest grant in the
host, or the pane keeps a kernel-owned cursor and a plugin supplies only its rows —
the retreat the spike named, which is not a plugin pane at all. That choice is
ADR-V1's, not a pane port's, so the gate states it rather than picking one.

## ADR-44: A partial port's remainder is document or behaviour, and the document half is closable

**Context.** ADR-31 ported the code-review view **in part** — the unified diff
stream's lines — and itemised ten unported behaviours, which is what
`migration/phase-4` requires of a partial port. The list sat unchanged for six
sections, read as ten things each waiting on its own decision.

Revisiting it produced a finding about the *list*, not the pane. Five of the ten
entries — file headers, hunk headers, comments with their badges, the review
summary, informational rows — are rows the native pane **lists**, drawable from
facts the kernel already holds. They were absent because
`App::build_review_snapshot` published `ReviewRow::Line` and skipped every other
row. The other five need a host power the plugin surface does not have: a write, a
`git` invocation, keys, or a resolved width.

A diff stream without its file headers is not a smaller reproduction of the review.
It is a different document: the reader cannot tell which file a line belongs to,
that a file is folded, or that a hunk has been reviewed.

**Decision.** A partial port's remainder is classified **document** or
**behaviour**, and the document half is closed by publishing the facts its rows are
drawn from rather than left on the list. For this pane:

- the published review section carries the review's **rows** — a tagged, ordered
  list of the six kinds — rather than its diff lines. The order is kernel view
  state (folding, comment interleaving, the summary section's position), so it
  crosses rather than being recomputed by a pane from a projection of it;
- a row carries **facts, not glyphs**: a file's status as `"modified"`, `folded`
  and `reviewed` as booleans. The pane derives `M`, `▸`/`▾` and `✓`, exactly as it
  already derives a diff line's `+`/`-` sign;
- **one exception**, stated as a rule: a row whose native text names a **kernel
  keystroke** is published as text. The review summary's heading reads `── Review
  summary (s to add) ──`; a plugin pane never receives `s`, so a pane composing
  that string would advertise an action it cannot perform and a pane omitting the
  hint would draw a different row. Only the kernel can honestly author it;
- a **new style token** is added when the palette field a row needs has none, rather
  than a near-miss token reused. The header's counts want `diff_added`/
  `diff_removed`; the vocabulary's `added` resolves `tool_allowed`, a separate field
  a custom theme sets independently;
- the **clip-versus-ellipsis** divergence is enumerated and attributed to the
  absent resolved width — the same fact side-by-side, wrap and horizontal scroll
  need — rather than recorded as a fourth gap or closed by publishing a width.

**Rejected alternatives.**

- *Parallel sections (files, hunks, comments) with the pane interleaving them.* The
  interleaving is the kernel's decision; a pane rebuilding it would get folding
  wrong the next time `is_file_folded` moved.
- *Publishing each row's composed text.* That is the general case the keybinding
  exception is carved out of, and it would turn the review section into a rendering
  channel — the thing the whole snapshot model exists not to be.
- *Publishing a comment's whole body.* Bounded at 64 KiB for one rendered line, and
  `str::lines` strips a trailing `\r` where a Luau split on `\n` does not, so a
  comment written on Windows would render differently in the two panes.
- *Publishing the pane's width to close the ellipsis.* The width is what wrap,
  pairing and horizontal scroll need; spending that decision — every published pane
  becomes a geometry problem — on an ellipsis pays the model's largest price for its
  smallest symptom.
- *Renaming a line row's `kind` so the row tag could be `kind`.* A published wire
  name with a shipped reader. Two tag fields cost a line of documentation; renaming
  one costs every reader.

**Consequences.** The section's bound now counts headers and comments too, so a
review of many small files publishes fewer diff lines at the same bound — the bound
doing its job, since a header costs nodes. The cursor is the review's own row rather
than the nearest published diff line, so the plugin's copy follows a cursor sitting
on a header. The pane's capability list is unchanged at two: the closure is entirely
read, which is what keeps it evidence about what a third party can build. What is
left of the ten is exactly the behaviour half, and ADR-45 is the attempt to hand the
pane over against it.

## ADR-45: The code review is not handed over — two seats, no bindable keys, a click that means a column

**Context.** ADR-44 closed the code review's document half: the bundled plugin
reproduces every row kind the native pane lists, pinned to the untouched renderer row
by row. The next step was the handover — drawing the plugin's pane instead of
`src/ui/code_review.rs`. It does not happen.

Three of the reasons are already recorded (a central seat, ADR-38 and ADR-43; a cursor
the kernel owns, ADR-43; the resolved width, ADR-31). Three are this pane's own, and
together they make it the **furthest** pane from a handover rather than the closest,
which is not what "the document is done" suggests:

1. **It is two panes.** The diff owns the central pane; the changed-files list owns the
   file-viewer column, with its own focus, its own keys and a selection that scrolls the
   diff — and `App::layout_for` forces that column present for as long as a review is
   open. The workspace tree seats the list as `RegionId::FileViewer` and a plugin pane
   as `RegionId::Plugin(n)`, a separate region, while `PaneSlot` offers a plugin only
   the right column. Every earlier refusal needed one seat; this needs two at once, as
   one surface.
2. **Its keyboard is not in the keybinding system.** `KeyContext` declares six scopes
   and no review; `handle_code_review_key` and `handle_review_files_key` are captures
   keyed on `self.focus`, run ahead of the lookup. So — unlike the tasks, automations
   and session-list refusals, each of which could name the scoped actions a plugin
   binding would replace — there is nothing to name. The keys are not rebindable today,
   so no configuration file restores them after a handover either.
3. **Its mouse surface exceeds the row channel in two different ways.** Eleven footer
   buttons, a scrollbar, the wheel and picker entries are missing target *kinds*, which
   a wider event carries. `App::cr_click_row`'s `rel_x`/`width` is a missing
   *coordinate*: on a paired row the half clicked decides which side a comment attaches
   to, and "the old side" is not a row, so no extra target kind expresses it.

**Decision.** The pane stays native, and the verdict is a gate —
`tests/code_review_pane_handover_gap.rs`, eleven rows re-derived from the tree, tagged
structural / vocabulary / wiring like its siblings — with the three findings pinned as
their own tests so a failure carries the argument rather than only the rule.

One row is recorded as **narrower** than the row it shares an id with. `no-cursor-write`
appears in the session list's gate, where the cursor *is* the application's active
session and writing it is the widest grant in the host. Here it is a row inside a view
the user already opened, read by the diff and the changed-files highlight and nothing
else. Two rows spelled alike with very different prices should not read alike, so the
gate states which is narrower and names it as where the work starts.

The ordering the table implies: the pane's keys become scoped actions (a keybinding
change, no plugin involved), then the narrow cursor write, then the two seats.

**Rejected alternatives.**

- *Hand over the diff and leave the changed-files list native.* The list is forced
  present by an open review, so the result is a plugin diff beside a native navigation
  aid that scrolls it — a half-handover whose seam is visible to the user.
- *Add `review-write` with the verdict.* The fourth capability in the host with zero
  consumers, which is the defect the earlier gates identified in `input`,
  `tasks-write` and `automations-write`. It is also premature: a review write without
  the seats and without the cursor lets a pane mark a file reviewed while unable to say
  which file the user is looking at.
- *Declare `input` on the bundled plugin so its pane can be focused.* The pane is not
  focusable precisely because it declares none, and hand-driving confirms `Ctrl+L` never
  lands on it while every review key still reaches the native pane. Keys with nothing
  to act on are a pane that takes a keystroke and drops it (ADR-38).
- *One "seats" row instead of two.* They close differently — a central slot is an
  addition to `PaneSlot` plus a branch that names a plugin pane, while the second seat
  is a second *pane* focused and navigated as part of the first. Collapsing them makes
  the harder one look like a detail of the easier.
- *One row per missing mouse target.* Four rows that all close with one wider event
  plus a fifth that does not reads as five problems where there are two.
- *Record the verdict only in the readiness document.* A verdict in markdown is a fact
  about a build that expires without telling anyone — the reason every refusal on this
  branch is executable.
- *Fold the rows into `tests/teardown_gate.rs`.* That table answers whether the native
  renderer may be deleted, which is already no and stays no. One table answering two
  questions produces failures that do not say which question moved.

**Consequences.** No source file changes, so nothing in the interface moves. A later
change adding a central slot, a review write, a cursor write, a review key context or a
wider click event fails the gate and is told which row moved and what to revisit. The
teardown inventory is unchanged: `src/ui/code_review.rs` was already protected, for the
same reason as before.

## ADR-46: A plugin pane's slot names a kernel region, and the plugin wins the seat

**Context.** `PaneSlot` had exactly one member, `Right`. So a plugin pane was
placeable *as a pane* and not placeable *where any of thurbox's own panes are*: the
session list **is** the left column, the automations pane is the band beneath it,
the info panel is its own `Percent(15)` column left of centre, and the code review
owns the central pane. Six rows across five handover gates recorded that one fact
(`no-left-seat` twice, `no-central-seat` twice, plus the review's second seat and
global search's band), and `docs/PHASE4-PANE-READINESS.md` §14 lists "the same
seat" first among the five requirements a handover has. It is the one requirement
shared by five of the six remaining handovers, so closing it per pane would be
closing it five times.

Nothing in the geometry needed inventing. The workspace tree (ADR-24) already
places `RegionId::SessionList`, `Automations`, `Info` and `Center`; what was
missing was a way for a **manifest** to name one of them, and a rule for what
happens when a plugin pane and the kernel's own pane both want it.

**Decision.** Four seats, one table, and the plugin wins.

1. **`PaneSlot` grows to five members** — `right` (unchanged default), `left`,
   `left-bottom`, `center-left`, `center` — named **geometrically**. A slot says
   *where*; naming one `info` would freeze the pane a seat exists for at the exact
   moment the point is that any pane may sit there.
2. **`PaneSlot::seat() -> Option<RegionId>`** is the single mapping from the
   plugin-facing vocabulary to the kernel's region names, with `Right` mapping to
   `None` because it is a *column* of `RegionId::Plugin(i)` regions rather than one
   seat. One table means no two consumers can disagree about where a slot is — and
   it is what the gates now probe, so "no slot reaches `RegionId::GlobalSearch`" is
   a statement about *this* region rather than about how many slots exist.
3. **A visible plugin pane takes its seat and the kernel's pane for it is not
   drawn.** The kernel keeps its own pane's visibility state, so hiding the plugin
   pane hands the seat straight back. Two claimants are decided rather than
   undefined: the first in publication order is drawn and the second is not drawn
   at all, the rule the right column already applies when it runs out of columns.
4. **A claim carves the seat.** `App::layout_for` ORs each claim into the flag that
   carves that seat, so a pane in `center-left` appears whether or not the user has
   the info panel open. With no claim every expression is what it was, which is why
   `compute_layout` gained no branch, no geometry test moved and no snapshot
   changed.
5. **The one content-derived height stays the kernel's.** The lower-left band is
   sized `(rows + 2).clamp(3, 10)`, and a plugin is never told its rect (ADR-26,
   ADR-29, ADR-30, ADR-31) — so the kernel keeps the policy and counts the rows
   itself, from `ViewNode::stacked_row_count` (the outermost stack's child count).
6. **The centre carries no kernel chrome.** A `center` pane is drawn with the frame
   every plugin pane gets; the tab strip and the F9 collapse chevron are not drawn
   over it, because both select surfaces that are then not on screen.

The three bundled reproductions whose native seat is not the right column now
declare it (`session-list` → `left`, `automations` → `left-bottom`, `info-panel` →
`center-left`). All three still seed hidden, so no install's screen changes;
showing one compares the two panes in the **same** rect, which is what
`tests/bundled_automations_panel.rs`'s placement divergence asked for and why that
divergence is retired.

**Rejected alternatives.**

- *The native pane wins while it exists.* The conservative reading, and it makes
  the whole change unexercisable: no seat could be occupied until the renderer it
  replaces had already been deleted, so the first handover would also be the first
  test of the seating — the big-bang shape every gate in this phase exists to
  prevent.
- *Let a manifest name a `RegionId` directly.* It would expose the header, the
  footer, the search strip and the status band as addressable, a far wider surface
  than four seats, and would tie the plugin-facing vocabulary to the kernel's
  region names forever.
- *Name the slots after the panes that occupy them today* (`info`, `session-list`).
  See above: after a handover "the info slot" names the plugin's own pane.
- *Add slots for the tasks panel and the file viewer.* Both are right-column
  occupants and `right` already seats a plugin pane in that column. The review's
  changed-files list wants `RegionId::FileViewer` *specifically* — the column an
  open review forces present — and that row stays blocked rather than being
  approximated.
- *A band slot for global search.* The strip is a mode, not a pane (ADR-31's
  neighbour finding, §10 of the readiness doc), and `tests/global_search_pane_gap.rs`
  keeps its verdict.
- *Let the manifest declare the lower band's height, or measure the pane's rendered
  height.* The first is geometry by another name and cannot track a list that
  grows; the second needs a width, and the width is downstream of the height it
  would feed.
- *Suppress the kernel action that toggles the native pane.* Then the seat would be
  empty and unreachable whenever the plugin pane was hidden or not placed. Binding
  a pane to a kernel action is a separate declaration (§14's second row) and a
  separate change.

**Consequences.** Five of the six remaining handovers no longer need a seat; what
they still need is input (a focus, and the scoped keyboards it silences) and, for
three of them, a module that is also the kernel's model. `no-left-seat` closes in
both left-column gates and `no-central-seat` in both central ones, each re-verdicted
with a probe that reads the seat table and the guard together — a slot with no guard
would be a seat a plugin could name and not be drawn in. The centre's missing chrome
and the untouched focus story are recorded as gaps, not absorbed: a `center` pane
today loses the tab strip, and a seated pane is still focused as
`InputFocus::PluginPane`.

## ADR-47: A pane declares the action that toggles it and the flag that gates it

**Context.** `docs/PHASE4-PANE-READINESS.md` §14's second row. A native pane answers
a kernel action and rides a kernel feature switch: `Action::ToggleInfoPanel` flips
`App::show_info_panel`, and `[features] info_panel = false` hides the pane and blocks
the chord. A plugin pane had neither — its visibility was `TogglePluginPane` (`F10`,
one action for every pane, ADR-28) plus a stored per-pane choice, and no `[features]`
flag reached it at all. ADR-46 gave a pane its seat; a seat whose key and switch are
gone is a pane in the right place that the interface no longer controls.

The declaration is also the mechanism by which the `[features]` flags eventually
retire: a flag whose only consumer is a pane can move into that pane's manifest.

**Decision.** Two optional `[[panes]]` fields, each validated against a closed set.

1. **`toggle_action`** names the kernel action that shows and hides the pane,
   spelled the way `keybindings.json` spells an action (`"ToggleInfoPanel"`) — one
   name for one action wherever a user meets it, rather than a second kebab-case
   vocabulary to keep in step.
2. **The set is curated, not "any action".** `Action::pane_toggles()` is the six
   whose *job* is showing or hiding a pane. A name that is no action is a serde
   error; a real action that is not one of the six is a manifest error listing
   them. Three exclusions carry reasons: `TogglePluginPane` (already reaches every
   pane, so binding it would flip a pane twice), `GlobalSearch` (a mode in a band no
   slot seats), and the modal/overlay openers (`OpenAutomations`, `ToggleHelp`,
   `OpenSettings`, `TogglePerfHud`). Two panes of one manifest may not bind the same
   action; across plugins both flip, because the host cannot arbitrate between
   manifests written independently.
3. **`feature`** names the `[features]` switch that gates the pane, spelled as
   settings.toml spells it, closed by a new `session::settings::FeatureFlag` with
   `FeatureFlags::enabled` as the single lookup. Every flag is accepted — unlike the
   action set there is no nonsense case — and an exhaustiveness test writes every
   `FeatureFlag::all()` key as `false` and requires every `FeatureFlags` field to be
   false, so a field with no member is a failing test rather than a switch no
   manifest could name.
4. **Both occupants toggle.** `App::dispatch_action` flips every pane bound to the
   action and then runs the kernel's own dispatch. Pressing the key twice returns
   every occupant to where it started, and the kernel never loses track of its own
   pane's state — ADR-46's reversibility rule, extended to the key.
5. **A gated-off pane is not a pane**: not shown, no seat, no column, not
   focusable, not rendered (published hidden, so its VM is not entered), and not
   offered by `F10` or its picker. Its **stored visibility survives**, because the
   switch answers "is this available" and the stored choice answers "does the user
   want it" — collapsing them would erase a choice when a flag went off and back on.
   The switch is read live, from `App::features`, so the settings panel and the
   mtime poll both apply immediately.

**Rejected alternatives.**

- *The plugin steals the action* (the kernel's half suppressed while a pane declares
  it). It is what the end state looks like, and while both panes exist it would
  leave the native pane unreachable by any key — a third-party plugin declaring
  `ToggleInfoPanel` would remove the user's info panel with no way back.
- *A free-form action string resolved at dispatch.* A typo would be a key that
  silently does nothing, the failure every other manifest field is validated to
  prevent.
- *A kebab-case action vocabulary* matching `slot` and `capabilities`. Two names per
  action, and the one place a user compares them is where the mismatch would show.
- *Accept any `Action`.* A pane bound to `QuitApp` would toggle when the user quits;
  the field exists for a handover, not for arbitrary key adoption.
- *Hook each of the six actions' handlers.* Six edits that must each remember to do
  the same thing, versus one funnel every action already passes through.
- *Resolve the feature gate when the pane set is published.* `[features]` is
  live-reloadable, so a baked-in value would be stale until the next plugin reload.
- *Collapse the gate into `visible`.* It would silently erase the user's choice the
  moment a flag went off.
- *Declare either field on a bundled plugin.* A bundled reproduction that answered
  `F2` would toggle **both** panes for every user who pressed it — a behaviour
  change in a change whose point is that nothing changes yet. The fields ship
  exercised by tests, ready for the handover that needs them.

**Consequences.** §14's five-row table is down to one open row, the render latency.
A handed-over pane can now answer the key its native counterpart answers and be
hidden by the switch its native counterpart rides, which is what makes deleting that
renderer a change a user does not notice. `Action::pane_toggles()` is also the list a
later change edits when a new kernel pane gains a toggle — forgetting it means the
pane cannot be handed over, not that something breaks silently.

## ADR-48: The recorded oracle is owed by the port, and the gate enforces it

**Context.** ADR-42 decided that a pane's oracle must be **recorded** — an
expectation generated from the native builder, so the proof survives the deletion of
that builder — and applied it to the info panel. A later change applied it to the
session list and the automations pane and made the recorder shared. The rule it left
in `migration/phase-4` was: *a pane whose handover is attempted is owed its recording
before the attempt concludes, whichever way it concludes.*

That rule fired for nobody. Three panes had a handover attempted and refused — the
tasks pane, the file viewer, the code review — and all three attempts concluded with
the oracle still purely differential, comparing the plugin's tree against
`ui::tasks_panel::tasks_tree`, `ui::file_viewer::file_tree` and
`ui::code_review::review_stream_tree`, every one of which lives in the module its
handover deletes.

Two properties of the trigger explain it, and they compound.

It fires **too late**: all three attempts concluded before the rule was written, so
the rule's first act was to describe a debt it could not collect. And it is
**unobservable**: "an attempt concluded" leaves no artefact in the tree, so nothing
can fail when it is skipped. It was a convention, and this phase has now watched a
convention fail three times in the same shape — a probe that permitted what it was
written to forbid (ADR-37's two-condition handover), a gate row read as agreement
(PHASE4 §10, §11), and this.

**Decision.** The obligation moves from an *attempt* to a **reproduction**, and the
teardown gate carries it.

- **Every pane a bundled plugin reproduces carries a recording**, whether or not its
  handover has been attempted. Reproduction is observable — a bundled plugin
  directory plus an oracle file — and it is the earliest moment the recording is both
  owed and provable: the plugin exists, so there is something to constrain, and the
  native builder exists, so the baseline can be shown to be the pane's.
- **`tests/teardown_gate.rs`'s pane probe gains a fourth conjunct.** A pane is handed
  over only when its oracle holds a recorded expectation: the oracle file uses the
  shared `tests/view_tree_record` recorder, asserts an `insta` snapshot, and has at
  least one recording checked in. Re-derived from the tree like the other three
  conditions, and read as source text so the verdict is identical under
  `--no-default-features`, where no pane oracle compiles at all.
- **Two tests, deliberately separate.**
  `a_pane_whose_oracle_is_differential_is_not_handed_over` is pure over the four
  conditions, because the tree cannot exhibit the case (a native renderer that is no
  longer drawn is one someone already deleted).
  `every_reproduced_pane_records_its_native_tree` is positive and per pane, so a
  missing recording fails **now** — the conjunct alone only fires once someone also
  stops drawing the native pane, which is the change least able to add a recording.
- **A row may name no oracle.** Global search is recorded structurally unportable and
  has no bundled plugin, so it has no reproduction to constrain; its `oracle` is
  `None` and condition 1 already blocks it.

**Why condition 4 is not simply more of conditions 1-3.** Those protect the *pane*:
violate one and a column is empty, which someone eventually notices. This protects
the *evidence*: violate it and the pane looks perfect while nothing constrains it any
more. It is therefore the quietest of the four, and the only one whose window closes
— a recording is provable only while the native builder is present, so a handover
that skips it cannot be repaired afterwards, and the repair that compiles is to
delete the assertion.

**Rejected alternatives.**

- *Leave it a spec requirement with no probe.* That is the convention that already
  failed for three panes; the person who reads a requirement is not reliably the
  person who writes the handover.
- *A separate test file for pane oracles.* It would pass while the gate permitted the
  deletion, and the gate is what a handover author runs. Two gates disagreeing about
  whether a pane may go is worse than one gate that is complete.
- *Assert on the recordings' contents.* `insta` already fails when a recording moves;
  the gate's question is structural — does a recording exist, and is the oracle wired
  to it.
- *Derive the oracle's path from the pane id.* It is not derivable (`tasks-plugin` →
  `bundled_tasks_panel`, `session-list-plugin` → `bundled_session_list`), and guessing
  would turn a renamed test into "file not found" instead of a verdict. It is a table
  field beside `native_module`, which the row already carries for the same reason.
- *Have the gate verify the recording's provenance.* It cannot: the tree holds a
  `.snap` file and a call site, not where the bytes came from. That is covered by the
  recorded edge being asserted against the native tree in the same loop, and by
  perturbing each pane's **native** side and observing the recorded edge fail.

**Consequences.** Six panes are reproduced and six carry recorded oracles; of ADR-37
and PHASE4 §14's five handover requirements, four are closed and only the render
latency remains. A future bundled plugin for a seventh pane fails
`every_reproduced_pane_records_its_native_tree` until it records its tree, which is
the intended behaviour: the gate now asks for the evidence at the moment the evidence
is cheap to produce.

## ADR-49: A plugin pane renders when a source it reads moves

**Context.** The plugin render worker rendered every visible pane and then waited
out a **fixed 1 s interval** in ten 100 ms slices, serving key requests. Nothing
told it that kernel state had moved. Two consequences, in opposite directions:

- a plugin pane's copy of any published state — the session-list cursor, a
  countdown, a status glyph a key had just written — trailed the kernel's by up to a
  second; and
- an idle TUI entered a Luau VM once per visible pane per second to rebuild a tree
  that had not changed (0.87 ms/s at 20 sessions, 14.5 ms/s at 200, against v1's
  zero for the same pane).

The session-list spike fixed a bar of 5 ms of added latency on a selection change
and made its verdict *conditional* on the render being event-driven; PHASE4 §7 and
§13 recorded the gap and argued the staleness was tolerable **because a plugin pane
is a hidden reproduction**, so the surface a user watches is still the kernel's. With
the seat (ADR-46) and the toggle (ADR-47) closed, a handover inverts that argument
entirely: the stale pane becomes the only pane. It was the last of PHASE4 §14's five
handover requirements.

Two closures were rejected when the gap was filed, and the first rejection rested on
an estimate that measurement contradicts. "The snapshot carries host CPU and memory,
so it changes on nearly every tick: a 1 Hz poll becomes a ~100 Hz one." The snapshot
is already change-gated and its values move at their *collection* cadence
(`METRICS_REFRESH_TICKS` is 100 ticks; countdowns are whole seconds), so the measured
publish rate is ~1 Hz. The objection is not empty, though: an agent emitting activity
text moves the session source on consecutive ticks, so an unbounded nudge needs a
rate policy.

**Decision.** A pane is rendered when a **source it reads** moves, bounded by a rate
ceiling.

- **Sources are named.** `session::plugin_manifest::PaneSource` has seven members —
  `sessions`, `metrics`, `automations`, `tasks`, `files`, `review`, `plugin-state` —
  with `Capability::source()` mapping each state-reading capability to exactly one,
  exhaustively and with no wildcard arm, and a `SourceSet` bitset. It sits beside
  `Capability` rather than beside the snapshot because a source is a property of the
  *grant*, and because `plugin-state` is a source that is not in the snapshot at all.
- **A publication says what moved.** `PaneContext::changed_sources` is both the
  publisher's change gate and what the worker is told. One derivation, not two: a
  field belonging to no source would publish by inequality and nudge nobody, and its
  pane would go stale with nothing failing. Both snapshots are destructured by name
  with no `..`, so a field added to `PaneContext` fails to compile until it is
  assigned to a source (ADR-42's device, at the other end of the same data), and a
  table-driven test pins `changed_sources(a, b).is_empty() == (a == b)`.
- **The nudge shares the input channel.** `PluginWorkerRequest` is `Input`,
  `StateMoved(SourceSet)` or `RenderAll` (sent when a pane's visibility moved: a pane
  the worker was skipping has *no* tree, so it is missing rather than stale). One
  channel because `std::sync::mpsc` has no select, and every message means "act on
  this before you next sleep".
- **The policy is pure.** `plugin::render_trigger::RenderTrigger` decides what to
  render and when to look again, with `now` and the pane list passed in. The loop
  that drives it is in `src/main.rs` — a **binary** — so anything decided inline
  there is a decision no test can reach, which is how a fixed cadence survived three
  ports without a failing test.
- **The ceiling is 100 ms and coalesces rather than delays.** A change arriving at
  rest renders immediately; changes arriving inside the interval merge into one pass
  at its end. 100 ms because it is the spike's own bar 1 ceiling (≤10 Hz sustained),
  because it is tighter than the kernel's 250 ms `FORCE_REDRAW_INTERVAL` so a plugin
  pane can never be more than one forced frame behind, and because it is the number
  the gap's filing named.
- **One timer survives, and it is named.** A pane whose plugin holds `state-read`
  draws from `plugin_kv`, which its own service half writes from another process —
  no event announces it. That pane keeps a periodic re-render on the source-file
  poll's existing 1 s cadence, raised only when a running plugin declares the
  capability, so the bundled set pays nothing.
- **The property is asserted on counters.** `plugin_renders_applied` /
  `plugin_renders_changed` make "a re-render producing the same tree costs no
  repaint" a failing test rather than a claim.

**What is not claimed.** The spike's 5 ms bar is **not** met: a change arriving just
after a pass waits out the remainder, up to 99 ms. Under a ~1 Hz publish rate that is
rare and the typical added latency is zero, and the two gate rows' original wording —
"in the frame the key was handled" — is unreachable by construction rather than by
wiring, since `plugin-host/panes` forbids the kernel calling a plugin during a frame.
Both rows were re-worded to the achievable bar with the reason recorded in the row.

**Rejected alternatives.**

- *Keep the interval and shorten it.* A 100 ms cadence is ten times the idle VM cost
  for a pane that changes nothing — the cost the spike's bar 3 already fails.
- *A dirty flag the worker polls between slices.* The wake costs up to a slice **on
  top of** the ceiling, and a flag cannot carry which source moved, so every pane
  renders for every change.
- *Nudge on any publication, without sources.* A timer wearing a different hat: a
  session-list pane would re-render every time host CPU resampled, at the same ~1 Hz.
  Measured, that is 20 renders per 20 s where the source-aware trigger does 0.
- *A second channel for nudges.* No select in `std`, so two receive arms and two
  disconnect paths for messages with identical timing requirements.
- *A condition variable shared with the publisher.* The UI thread would take a lock
  the worker holds while inside a plugin VM.
- *Send the snapshot rather than the source set.* The worker already reads the
  published slot when it renders; a clone per change duplicates the state and leaves
  the worker to diff it again.
- *Resolve which panes to render on the UI thread.* It would need the grants, which
  live in the host on the worker's side, and it puts a policy decision on the render
  loop's thread for no gain.
- *An API letting a plugin ask for a frame.* Hands a plugin the demand-driven loop;
  ADR-V18 refused the same thing for motion frames.
- *Drop the periodic render entirely.* A pane reading its plugin's own state would
  freeze until something unrelated moved — silent, and worse than the cadence.

**Consequences.** All five of PHASE4 §14's handover requirements are now closed; what
blocks the six reproduced panes is focus and each pane's own recorded rows. Two of
those rows closed with this change (`render-is-not-event-driven` on the session list
and on the automations pane), and the session list has no outstanding *wiring* gap
left at all. A capability added to the vocabulary must now decide what it reads, and
a field added to the published snapshot must decide which source it belongs to —
both as compile errors.

## ADR-50: The info panel is deleted, and a plugin is the pane

**Context.** Phase 4 reproduced six of thurbox's own panes as bundled plugins and
handed over none of them. PHASE4 §14 listed five things a handover needs and they
closed one at a time: the build (ADR-40), the seat (ADR-46), the toggle and the
feature flag (ADR-47), the recorded oracle (ADR-42, ADR-48), the render trigger
(ADR-49). What still blocks the other five panes is **focus** — a seated pane is
`InputFocus::PluginPane`, so `KeyContext::SessionList` / `Automations` / `Tasks` do
not resolve — plus each pane's own recorded gap rows.

The info panel is outside both. It declares no `input` capability, so it has no
scoped keyboard, no cursor and no mutation, and it is the only reproduced pane with
no gap file at all. Every change since §14 was made for it.

**Decision.** `src/ui/info_panel.rs` is **deleted** — 2018 lines, including the
pre-port line builders that were the view-tree port's byte-identity oracle. The Info
column is `src/plugin/bundled/info-panel/init.luau`, drawn from the `center-left`
seat, bound by its manifest to `ToggleInfoPanel` and gated by `[features]
info_panel`. Five decisions the deletion needed:

**The kernel's own occupant of the seat is deleted, not switched off.**
`App::show_info_panel` goes with the renderer. Keeping the `bool` compiles and passes
every existing test, and is wrong in a specific way: `layout_for` carves a seat when
**either** occupant wants it (ADR-46), so a flag nobody paints from still carves the
column. F2 in a build with no plugin host, or with a broken `info-panel` plugin,
would have produced a bordered 15% column containing nothing — the exact failure
`tests/teardown_gate.rs` exists to prevent, reached from inside the change meant to
honour it. With the flag gone the seat's condition is `seat_taken(CenterLeft)`, so
the impossible state is unrepresentable rather than merely untested. Consequences,
both stated rather than absorbed: the panel's visibility is now **persisted** (plugin
pane visibility lives in `metadata`) where the flag reset every launch, and a resize
below 120 columns no longer *destroys* the choice — the layout declines the seat and
widening brings the pane back.

**The seed stays hidden.** `default_visible = false`, because `show_info_panel`
initialised to `false` and F2 showed the panel: a handover changes which code draws a
pane, not whether it is on screen. `tests/bundled_manifests.rs`'s
`PANES_DRAWN_IN_A_NATIVE_PANES_PLACE` **permits** a handed-over pane to seed visible
— its argument is that "visible *and* duplicated" is the mistake — and does not ask
for it. Seeding visible would also have changed every ≥120-column install's first
launch with nothing failing: the acceptance snapshots render at 100 columns, below
the width at which this seat is carved, which is the same fact that made §14's
proposed proof vacuous. The pane is still added to that list, so the list answers
"which bundled panes are no longer reproductions".

**The empty state is decided, not discovered.** §14 found the one divergence no
oracle covered: with no active session `render_info_panel` returned *before* painting
its block, so the seat was a borderless gap, while a plugin pane's frame is the
kernel's and is always painted. The plugin's behaviour is **accepted** — a bordered
`Info` column showing System (host CPU, RAM, thurbox's data-dir size) and any
upcoming automations. An empty bordered box would be strictly worse than both, and
not carving the seat without a session would put a *content* condition into the
layout that no other seat has and that the kernel cannot evaluate without knowing
what the plugin will draw. Pinned as a frame, since the tree was already pinned.

**A pane that cannot receive input records no click target.** Found by driving the
handover rather than by a test. `handle_mouse_click` hit-tests the click registry
*before* the pane fallback that arms drag-select, and that fallback covers the info
column — which is how text is selected out of it. Every visible plugin pane recorded
a whole-rect `PluginPaneRow` target, and for a pane with no `input` both handlers
already refuse it (`focus_plugin_pane` will not focus it; `offer_click_to_plugin`
reads the *focused* pane), so the target's only effect was to consume the click. The
registry now agrees with the two guards. Registering a target whose only effect is to
swallow a click was the bug; the handover is what made it observable.

**The evidence is the recording, and it was not regenerated.** The oracle asserted
three edges; two named `info_tree`, which this change deletes. What is left is
`plugin == recording`, and the ten `.snap` files are **byte-identical** after the
deletion — which is the whole payoff of ADR-42, since a `cargo insta accept` here
would have converted ten statements about the pane into ten statements about the
plugin. `Case` carries the published `SystemSnapshot` / `UpcomingAutomationSnapshot`
instead of the deleted types, and the `SessionInfo → SessionSnapshot` derivation is
untouched because it mirrors `App::build_pane_context`.

**Consequences.** `SystemMetrics` moves to `src/app/metrics_state.rs`, whose
`MetricsState` owns the value — it is an input the collector produces for one
consumer, not shared vocabulary, and `session::pane_context::SystemSnapshot` is
already `session`'s spelling of the same five numbers. `AutomationEntry` is deleted
rather than moved: it carried a pre-rendered countdown *string*, and the snapshot
carries seconds. `ToggleInfoPanel` keeps its `[features]` gate and, when nothing
claims the seat, **reports which plugin provides the pane** — the honest surface for a
failed bundled plugin, for a user plugin of the same name that shadowed it, and for a
build with no plugin host. `set_plugin_pane_visible` now resizes the sessions, which
every kernel panel toggle already did and no plugin-pane path did; showing the Info
column narrows the agent's terminal, so its PTY has to be told.

A **new failure mode** arrives with the pane: a broken `info-panel` plugin costs the
Info column, where before it cost nothing. Driving it shows two distinct cases, and
the distinction is why the action's report had to exist rather than being a nicety:

- a **load** failure (a syntax error, a top-level `error()`) means the plugin has no
  pane, so nothing claims the seat and nothing is drawn — and F2 would have been
  *silent*. It reports `Info panel: provided by the \`info-panel\` plugin, which has
  no pane here (see \`thurbox-cli plugin doctor\`)`. Fixing the file recovers the
  column on the next source poll, without a keystroke, because the stored visibility
  is still the user's.
- a **render** failure (an error inside `render`) keeps the pane: its title becomes
  `Info (error)` and the body shows `failed: …` over the last good tree
  (`paint_plugin_pane`), which is the pane-level containment `plugin-host/panes`
  already required.

Not mitigated away: it is what "every pane a plugin" means, and both cases name
themselves.

**`--no-default-features` loses the pane, deliberately.** That build has no plugin
host. It gets no empty column (the seat is carved only by a claim, and no claim can
exist), no silent key (`toggle_panes_bound_to` is already a `false` stub there, so
the action reports the absence — in that build's own words, since it ships no
`thurbox-cli plugin` subcommand to send anyone to), and no orphaned state. Keeping the renderer under
`#[cfg(not(feature = "plugins"))]` was refused: `migration/phase-4` forbids exactly
that, and for a good reason — it produces two panes that differ by build, and the one
users install is the one nobody tests hardest. Stage B put `plugins` in the default
feature set precisely so no install is in this position, and the teardown gate fails
if it ever leaves.

**Not done: the code review.** It was the other pane proposed for this change and it
is refused. `tests/code_review_pane_handover_gap.rs` re-derives eleven rows and ten
are still blocked: two seats (its changed-files list wants `RegionId::FileViewer`,
which no slot names), a keyboard that is a `self.focus` capture rather than scoped
actions and therefore not rebindable at all today, and five operations no capability
performs (review writes, `git` retargeting, clipboard/agent export, cursor writes, a
resolved width). Deleting `src/ui/code_review.rs` would replace a mouse-first,
eleven-button, searchable, retargetable review with a scrollable read-only document.
ADR-45's ordering stands.

## ADR-51: A pane may answer one of thurbox's keyboards, and is focused as that pane

**Context.** ADR-50 handed over the first native pane and named what blocks the
other five: **focus**. A seated plugin pane is `InputFocus::PluginPane`, so
`KeyContext::SessionList` / `Automations` / `Tasks` / `FileViewer` never resolve. A
pane could sit exactly where thurbox's own pane sat (ADR-46), answer the key that
showed it and ride the flag that gated it (ADR-47), and still be a surface the
keyboard cannot reach. It is the one requirement shared by four of the five
remaining handovers.

There are two ways to close it, and choosing decides what a v2 pane *is*.

1. **The plugin gets the keys** — `input`, a binding per chord (ADR-34), and then a
   capability per effect: a view write for `j`, a filesystem read and a process
   launch for the file viewer's `l`, a record creation and a modal for the tasks
   pane's `n`/`r`, an agent reach for what that picker does.
2. **The pane gets the keyboard** — the pane declares which of thurbox's scoped
   keyboards it is the pane for; the kernel resolves those actions and performs them
   against its own state, as it already does, and the plugin draws the result.

Route 1 has been priced twice and refused twice. ADR-38 refused the view write (a
plugin's cursor and the kernel's would disagree, so `o`/`r`/`e` would act on a
different row than the one highlighted); ADR-39 refused the filesystem capability
(the widest grant in the host, and *still* insufficient, because the expansion set
and the search verdict are the kernel's). Neither refusal has weakened. Route 1 also
cannot reach half of what these keys do: the central-pane editor, the trigger-time
picker and the editor process are not panes, so no pane declaration touches them.

**Decision.** One optional `[[panes]]` field, and the focus it implies.

1. **`key_context` names a kernel key context**, spelled as the kernel spells it
   (`key_context = "Tasks"`), validated against `KeyContext::pane_keyboards()` — the
   four that scope a pane's keys. `Global` is refused (no pane's) and `Terminal` is
   refused (its keys are written to a PTY, so "the kernel dispatches the action" is
   false and a pane claiming it would receive nothing). Two panes of one manifest may
   not claim one keyboard.
2. **Such a pane is focused as thurbox's own pane of that name.** `InputFocus::TaskList`
   already meant "the interface's task list holds the keyboard"; that it also meant
   "and `ui::tasks_panel` is painting it" was a coincidence of there being one
   implementation. So `App::focus_key_context` is **untouched**, every scoped action
   resolves as before, and the central-pane workspaces, the editor return paths and
   `Esc` are correct by construction. `App::focus_for_keyboard` is the one table from
   a context to its focus. What moves is focus *entry*: a ring stop appears when
   **either** occupant of that pane's place is on screen.
3. **It is focusable without `input`, and never handed a key.**
   `PluginPane::is_focusable_with` becomes "on screen and can receive keys", of which
   there are now two kinds; `takes_plugin_input_with` is the narrower "focusable **as**
   a plugin pane", which a declared keyboard is deliberately not — landing there as
   `PluginPane` would silence the keyboard the pane declared.
4. **A pane may not declare a keyboard and bind its own keys.** A `[[keybindings]]`
   entry on such a pane is a manifest error. The alternative is a delivery order, and
   a plugin that shadowed `d` in the tasks keyboard would do it silently, with the F1
   editor showing both bindings and nothing saying which won. The plugin-wide `input`
   capability is *not* refused: a plugin may have an ordinary input pane beside an
   inheriting one.
5. **A focused pane is drawn as focused, from one rule.** `paint_plugin_pane` painted
   every pane `FocusLevel::Inactive` — invisible while every pane was a hidden copy,
   wrong for a pane that is the interface's task list, whose border is how a user sees
   where `j` is going. It now takes the level, and `App::pane_focus_level(context)` is
   the single rule **both** occupants use, including the three-level cases (a tasks
   pane is `Active` while the editor it opened holds focus). Nothing is published to
   the plugin: a frame is the host's.
6. **A click on such a pane is the kernel's row action.** `ClickAction::SelectTask(i)`
   and its siblings rather than `PluginPaneRow`, plus `FocusPane(<inherited focus>)`
   for the rest of the rect — so a click means in the plugin's pane what it meant in
   the kernel's, and nothing reaches the plugin. The hitbox index is one-based
   (`ui.list`'s numbering) and the kernel's actions are zero-based, converted once.
7. **The published `focused` flag becomes true for it, and that is not a widening.**
   `build_tasks_snapshot` already publishes `focused: matches!(self.focus,
   InputFocus::TaskList)`, and that focus can now be the plugin's pane. So
   `plugin-host/input`'s "a pane is told nothing about its own focus" is **narrowed**:
   it is a statement about a *reproduction*, whose focus is a different thing from the
   native pane's, and for a pane that is the surface itself the two coincide. Leaving
   it false would be the wrong purity — the tasks pane's empty state reads `no tasks —
   n to add` only when the key works, and in a handed-over pane it does.
8. **No bundled plugin declares the field**, for ADR-47's reason: a reproduction that
   inherited the keyboard while the native pane still draws would paint two panes as
   focused and put a cursor in one that a user moves in the other.

**Is this a fig leaf?** The objection is that the kernel keeps the pane's behaviour,
so "every pane a plugin" is only about drawing. Two answers. First, where the code
lives: a pane's state machine is `App` (`task_ui`, `automation_ui`) and its keyboard is
`session::Action` + `KeyContext`; what `src/ui/<pane>.rs` holds is the drawing, which
is what a plugin takes over. (The one exception, `FileViewerState`, is recorded as a
defect by that pane's gate rather than as the model's home.) Second, the phase's own
test still holds: what a bundled plugin can do is what a third party's can. A third
party may declare `key_context = "Tasks"` and draw the task list grouped by status
with its own glyphs, and `j`/`Space`/`d` keep working — because the keyboard is the
*pane's* identity, not the plugin's implementation. What it may not do is invent a
key; a plugin wanting keys of its own declares `input` and gets ADR-34's addressed
bindings, unchanged.

**Rejected alternatives.** *Keep `InputFocus::PluginPane` and teach
`focus_key_context` the focused pane's context* — then `render_task_workspace`, the
published flag, the editor's return path, `Esc` and the ring each become "the tasks
focus, whichever of the two it is today": an indirection at a dozen sites whose
failure mode is a site that forgets, which reads as a pane that half works.
*`KeyContext::Pane(String)`* — ADR-34 rejected its neighbour for reasons that hold:
the enum is `Copy` and matched by value, and the keybinding namespace must not depend
on which plugins are installed. *Infer the keyboard from the seat* — a third-party
pane may legitimately sit in a seat without being that pane, and inheriting `d`
(delete session) by virtue of geometry is the worst possible default. *Grant the view
write anyway* — ADR-38's refusal, and it is still insufficient. *Deliver the kernel's
**action** to the plugin (`onAction`)* — route 1 in route 2's clothes: every action
still needs a grant, and now there are two spellings for a key. *Let the plugin
override individual keys of the keyboard it declared* — a key's meaning would depend
on a plugin's return value, unpredictable per install and undiscoverable in F1. *Wire
the focus inside the first handover that needs it* — the shape ADR-46 rejected for the
seat, and worse here: four panes would land four incompatible bits of focus plumbing.

**Consequences.** Four of the five remaining handovers are unblocked on focus, and
each is now blocked only on its own remaining rows — which for three of them are
*drawing* rows and for two of them a module that is also the kernel's model. Three
gates re-scoped their recorded sentences, because "a handed-over pane is focused as
`InputFocus::PluginPane`" is no longer a fact about panes: the automations gate's
`focused-pane-draws-an-unfocused-border` row is **closed** by decision 5, its
central-seat row is now recorded as a wall for the *plugin-keys* route only (a pane
declaring `Automations` is focused as `InputFocus::Automations`, which that branch
names, so the editor and the run history do appear for it), and the session list's
`scoped-keys-silenced-by-the-handover` says the same about its six actions. The code
review is the one pane the route cannot reach, and for a reason worth having: its keys
are a `self.focus` capture rather than scoped actions, so there is nothing for a
declaration to name — ADR-45's ordering (make them actions first) stands.

## ADR-52: A run may yield its width, and the kernel ellipsizes it

**Context.** Three of thurbox's own list panes fit one run of each row to the column
and the plugin catalogue could not say it. The tasks pane reserved the trailing `⇄`
marker's room and ellipsized the title into what was left; the automations pane fits
a name against `width − prefix − tail`; the session list does the same. A plugin has
no width — refused five times (ADR-26, ADR-29, ADR-30, ADR-31, ADR-39) — so its copy
drew the whole title and the renderer clipped it at the pane edge.

The consequence was not cosmetic. On a 20%-wide column a long title lost its `…`
*and* the marker after it: the two panes showed **different information**, recorded
as the tasks port's last enumerated divergence and as its one vocabulary gap row. The
closure has been named since ADR-29 — *an ellipsizing clip plus a flush-right run* —
and `ViewNode::Fill` had already landed the second half.

**Decision.** `TextStyle::ellipsize`: the run **yields its width** to the other runs
on its line.

1. **The kernel resolves it.** Every other run on the line keeps its intrinsic width,
   the remainder goes to the yielding runs, and they are truncated with `…`. Same
   trade as every other node: the plugin declares the intent, the kernel resolves the
   geometry.
2. **Consecutive yielding runs share one budget.** A title split at its
   global-search match offsets is three runs and one string to a reader, so the cut
   falls where the concatenation would have been cut and later runs draw nothing —
   never one ellipsis per matched character.
3. **It fits with `ui::truncate_ellipsis`**, the function the native panes fitted
   with. One implementation, two callers, the arrangement ADR-30 chose for the scroll
   window and ADR-39 for the scroll track. The known corner is inherited on purpose:
   that function counts *characters* while a line is laid out in *cells*, so a run of
   double-width glyphs can exceed its budget exactly as it does in the native panes.
   Being "correct" here would make a plugin's copy differ from the pane it
   reproduces, which is the one thing a reproduction may not do.
4. **A style field, not a node kind.** It is not a container (nothing sensible
   ellipsizes a list or a gauge), a node kind multiplies across seven exhaustive
   walks, and both gates that asked for this probed `TextStyle` for exactly this flag
   — so they re-verdict themselves rather than needing rewritten probes. The doc
   comment states the tension plainly: it is the one field that is neither a colour
   nor an attribute but a rule for what happens when the line runs out.
5. **Yielding is resolved before a fill.** A yielding run is bounded by what the
   *fixed* runs leave; a fill is the residue of everything. So a full line gives its
   fill nothing, which is the right answer.
6. **A yielding run inside a motion does nothing.** A motion has already reserved its
   widest frame's width and pads to it, so there is no residue for a run in it to
   give up, and truncating one would leave the padding computing from a width that no
   longer exists.
7. **The native tasks pane declares it instead of fitting.** `task_rows` loses its
   `width` argument and its `truncate_ellipsis` call. This is the load-bearing half:
   a native pane that kept fitting in its tree while the plugin declared the flag
   would produce trees that differ *by construction*, and no width could make them
   equal. `ui::tasks_panel` now reads neither a width nor a height — the first pane of
   which that is completely true, and the shape a handed-over pane wants.

**The evidence.** `tests/bundled_tasks_panel.rs`'s enumerated divergence is replaced
by its opposite: at 18 columns the two panes paint the **same frame**, ellipsis and
marker included. The twelve recordings were regenerated from the *native* builder —
which ADR-42 requires (the tree genuinely changed) and permits (the builder is still
here) — and the diff was verified mechanically to be 35 lines, each the same line plus
the word `ellipsize`, with nothing else moved in any file.

**Rejected.** *Report the resolved width into the plugin* — the sixth request and the
sixth refusal: rendering would become width-dependent, so a resize must re-enter a VM
before the frame that needs it. *Publish the fitted title in the snapshot* — it bakes
one pane's geometry (including its own marker's width) into state any pane in any
column reads, and a plugin acting on a row would match a string the kernel invented.
*A `maxWidth` in cells* — geometry with extra steps; computing it needs the width the
plugin is not told. *Infer which run gives way* (the last, the longest) — three native
panes fit a *different* run of their row, so any rule would be wrong in at least one.
*One ellipsis per yielding run* — visibly wrong on a searched title. *Adopt it in all
three panes now* — each adoption carries that pane's re-recording, and landing three
in the change that introduces the mechanism would make any failure report "the
ellipsis change broke a pane" rather than which one. *Ellipsize in a paragraph* — a
paragraph wraps, so nothing overflows.

**Consequences.** The tasks pane's reproduction is **complete**: no drawing row is
left in `tests/tasks_pane_input_gap.rs`, whose remaining rows are all about what a
*plugin's own* keys could do — and ADR-51 already answered those by a different
route. The automations gate's `no-fitted-name` stays blocked and its probe narrows to
the reason that is actually left: that pane still fits its name in `resolve_rows`, so
its plugin's copy still loses its tail. Adopting the declaration there, and in the
session list, is one line plus that pane's re-recording, and belongs to each pane's
own handover.

## ADR-53: The tasks pane is deleted, and a plugin is the pane — keyboard included

**Context.** ADR-50 handed over the info panel and could do so *because* that pane
takes no input. The tasks pane is the opposite end: ten `KeyContext::Tasks` actions, a
cursor the central pane follows, and two surfaces its keys open (the task editor, the
trigger-time action picker). Two changes made it reachable — ADR-51 (a pane declares
the kernel's keyboard and is focused as that pane) and ADR-52 (a run yields its width,
closing the last drawing gap) — leaving one thing: the **seat**.

**Decision.** `src/ui/tasks_panel.rs` is **deleted**. The tasks column is
`src/plugin/bundled/tasks/init.luau`, drawn from a new `tasks` seat, bound to
`FocusTasks`, gated by `[features] tasks`, and declaring `key_context = "Tasks"`.
Five decisions the deletion needed:

**The seat is named, reversing part of ADR-46.** That ADR declined slots for this pane
and the file viewer because *"`right` already seats a plugin pane in that column"*.
True, and insufficient: the column's occupants are drawn in a fixed order (tasks, file
viewer, plugin panes), so a `right` pane lands to the *right* of the file viewer while
the tasks column is to its left. **A position within a column is part of the pane**, so
`PaneSlot::Tasks → RegionId::Tasks` is added rather than approximated. Letting a
manifest declare a *position* was refused for the reason `PaneSlot` exists: it makes
every pane's place depend on which panes are installed.

**The hint row stays the kernel's, and that makes seat chrome a concept.** The native
pane reserved its bottom row while focused for `e edit · r run · n new`. A plugin
cannot draw it honestly — those are *rebindable* chords, and no published section
carries a keymap, so a plugin printing the letters it happens to know prints a lie for
a user who rebound them. Dropping the row was the alternative and it is a real loss of
discoverability in a change whose claim is that a user does not notice. So the kernel
draws it **into the seat**, above the plugin's tree, exactly where it was: the same
subtraction the native pane made before rendering its list, so the plugin's content
area — and its row hitboxes — are the area that pane's content had. It is described as
data (`App::pane_hints`) rather than a painter, so what a seat may draw stays
enumerable instead of becoming "the kernel paints whatever it likes inside a plugin
pane". It is also the mechanism the file viewer's search bar will need (ADR-39 called
it "the pane-chrome row"), established here for one row instead of there for a bordered
block with a caret.

**`show_tasks_panel` is deleted, and the focus rescues it was doing are kept.**
ADR-50's rule (a handed-over seat's kernel occupant is *deleted*, because
`layout_for` carves a seat when either occupant wants it). But this flag was
load-bearing in a way `show_info_panel` was not: it answered *"is the pane on screen"*
for **focus**. Three callers keep that question with a new answer — the ring stop, the
`[features]` teardown, and the below-120-columns resize — and the monkey invariant
becomes *stronger*: `focus == TaskList` now implies *a pane provides the task list*,
which in a build with no plugin host is unsatisfiable, so the invariant asserts that
build can never reach the focus and a seeded random walk hunts for a counter-example.

**`TaskPaneEntry` moves; the rendering dies with the renderer.** The entry is what
`App` *builds* — for the pane and, unchanged, for the published snapshot — so it moves
to `src/app/task_state.rs` (the move `SystemMetrics` made in ADR-50). `TaskRow`,
`TaskPaneState`, `task_rows` and `tasks_tree` go: the plugin builds the tree, and the
recordings hold it to the pane.

**The oracle keeps the recordings and loses its other two edges.** `plugin ==
recording` is what survives, and the twelve `.snap` files are **byte-identical after
the deletion** — `git status tests/snapshots/` empty, which is the whole payoff of
ADR-42, since a `cargo insta accept` here would convert twelve statements about the
pane into twelve about the plugin.

**The retired gate, and what it recorded.** `tests/tasks_pane_input_gap.rs` is deleted.
Its question was *what would a **plugin's own** keys need to drive this pane?* — and
the answer is preserved here, because it is still true of any pane that wants its own
keys:

| Row | What it needed | Still refused |
|-----|----------------|---------------|
| `no-cursor-write` | `j`/`k`, the preview scroll, `o` — all move view state | yes: no binding writes a cursor, focus or the active session |
| `input-and-cursor-are-disjoint` | acting on the row the user sees, through the kernel's cursor | yes for a *reproduction*; ADR-51 dissolved it for a pane that **is** the surface |
| `no-record-creation` | `n`, `e` — creating a task and authoring its text | yes: the write seam changes existing records by id |
| `no-modal-surface` | `r` — a modal that captures input ahead of every binding | yes: a manifest declares panes, not the interface's input |
| `no-agent-reach` | what that picker does — prompt a session, or spawn one | yes: `Capability::Spawn` adds environment to spawns thurbox makes |
| `no-ellipsizing-clip` | a fitted title with room for the marker | **closed** (ADR-52) |

Nothing in that table was granted. The pane is handed over because the keys were never
the plugin's to hold.

**Consequences.** `Action::FocusTasks` flips the plugin's pane (ADR-47) and the
kernel's own arm does what the flip cannot: follow it with focus, and *report* when
nothing provides the pane. Global search's task result reveals the pane through the
same door and reports when there is none, which is the one search result that can
fail to open. `ClickAction::SelectTask` and `App::pane_hints` become
`#[cfg(feature = "plugins")]`, like `PluginPaneRow` before them, because the surface
that produces them is a plugin's.

**`--no-default-features` loses the whole task TUI surface**, deliberately. The pane is
the only door to `InputFocus::TaskList`, so the central preview, the editor and the
picker are unreachable there and global search's task scope has nowhere to land — each
reporting rather than silently doing nothing. `thurbox-cli task` is untouched. Stage B
made `plugins` a default feature precisely so no install is in this position, and
`tests/teardown_gate.rs` fails if it ever leaves. Keeping the renderer under
`#[cfg(not(feature = "plugins"))]` was refused for `migration/phase-4`'s reason, which
is stronger here than for the info panel: two task panes differing by build, and the
one users install is the one nobody tests hardest.

**Driven, not assumed** (150×26, and 150×16 for the chrome): `F5` opens the column in
the native pane's position with the hint row on its bottom line and the accent border;
`j` moves the cursor and the central preview follows; `Space` cycles the status
(confirmed by `task list`); `n` opens the central editor and `r` the trigger picker,
both kernel; `Esc` leaves; `F5` hides it. `[features] tasks = false` removes the column
within a poll and `F5` then names the switch; turning it back on restores the column
from the **stored** choice with no keystroke. The `--no-default-features` binary carves
nothing and reports `Tasks panel: provided by a plugin, and this build has no plugin
host` — its own words, since it ships no `thurbox-cli plugin` subcommand.

**Not done: the file viewer.** It was the other pane proposed for this work. ADR-51
removed its two dangerous grants from the argument — the kernel still reads the
directory and launches the editor — so what blocks it is now: its **search bar** (three
rows of kernel chrome with a caret and a match counter, a bigger version of the row
this change establishes), the fact that `src/ui/file_viewer.rs` is the pane's **model**
and the home of `visible_window`, which every plugin list scrolls by, and its column's
**second kernel occupant**, the code review's changed-files list, which ADR-45 records
as wanting `RegionId::FileViewer` specifically. Three decisions, none of them made
here.

## ADR-54: The file viewer is not handed over — three decisions, and not one is a capability

**Context.** The file viewer was proposed for handover beside the tasks pane, and the
brief asked for *the minimum widening of `Capability::Files` the pane needs*. Its gate
(`tests/file_viewer_pane_input_gap.rs`) recorded six rows, five structural: all seven of
the pane's `KeyContext::FileViewer` actions write view state, expanding a directory
**reads the filesystem**, expanding a file **launches a process**, the `/` sub-mode's
keys are not rebindable, nothing carries a query inward, and no node draws the search
bar. ADR-39 had refused a filesystem capability on the merits — the widest grant in the
host, and still insufficient.

**Decision.** The pane stays. `src/ui/file_viewer.rs` is not deleted, and the answer to
the brief's question is that the minimum widening is **none**.

ADR-51 changed the question. A pane may declare `key_context = "FileViewer"` and be
focused as `InputFocus::FileViewer`, so the kernel resolves those seven actions and
performs them itself: `FileViewerState::activate` keeps doing the directory read,
`App::file_viewer_expand` keeps launching the editor, the `/` sub-mode stays kernel
state, and the plugin draws. Five of the six rows stop being handover requirements
without anything being granted.

What is left is three **decisions**, and the gate now records them as rows:

1. **The seat.** `PaneSlot` names none for this column's first occupant. One line of the
   same table ADR-53 extended — except for (3).
2. **The module is the model *and* the window.** `src/ui/file_viewer.rs` is 1601 lines:
   `FileNode`, `FileRow`, `Activation`, `FileViewerState` (the expansion set, the cursor,
   the search, the reads) and `enumerate_paths`, which `App` owns and the published
   section derives from — plus `visible_window`, the rule **every plugin list** and three
   native panes scroll by (ADR-30). Deleting the renderer means relocating both, at five
   call sites in four modules. ADR-39 called that motion without a destination; the
   destination exists now, and the move belongs in the change that uses it, where the
   oracle and the seat are what prove it landed.
3. **The column has a second kernel occupant.** While a review is open, `layout_for`
   force-shows this column and `render_file_viewer` draws the review's *changed-files
   list* into it — its own focus, its own keys, and ADR-45 records it as wanting
   `RegionId::FileViewer` specifically. So ADR-46's precedence rule (a visible plugin
   pane takes the seat) is the **wrong** rule here: it would hand the column to the
   plugin while the review needed it. This is the first seat where a claim must not
   simply win, and the rule is not written.

**Why record it rather than do it.** Each decision is small; taken together with a
900-line relocation they are the shape of change that gets one of them taken carelessly
— and two of the three have a failure mode a passing test suite would not show: an empty
column whenever a review and the file viewer are used together, and a scroll window that
stopped working for every *other* plugin pane.

**What the gate now keeps, which is the part worth having.** The three rows that closed
did so *without a grant*, and "the widening was unnecessary" is indistinguishable from
"the widening happened" in a table that stopped looking. So `no-filesystem-read` closes
on a **conjunction** — the keyboard is declarable **and** `Capability::Files` still
publishes basenames, no binding lists a directory, and the published row still carries no
path. If someone adds `readDir` to the module surface, that row fails: not because the
pane became unhandoverable, but because this ADR's claim stopped being true.
`the_verdict_is_derived_from_the_blockers` asserts the headline directly — **nothing
outstanding is structural** — plus, separately, that no filesystem capability exists.

**Rejected.** *Widen `files` to close the rows honestly* — unnecessary, and closing a
checkbox with the widest grant in the host is what the brief for this work explicitly
warned against. *Hand it over and accept the review losing its column* — the failure the
teardown gate exists to prevent, reached from inside a change meant to honour it. *Delete
the gate as obsolete* — five of its rows are the only record that the grants were
unnecessary rather than absent; the tasks pane's gate could be retired because its pane
was handed over in the same change (ADR-53), and this one's verdict is still no. *Do the
relocation now as groundwork* — see (2). *Rename the file to match its new question* —
its module note says what it measures, and four documents reference the name.

**Consequences.** The file viewer is the **closest** remaining pane: its focus is closed
(ADR-51), its rendering is reproduced to the frame, its recordings are taken, and its
remaining work is three decisions with known mechanisms rather than a grant with a
security argument. The order the gate implies is (3), then (1), then (2) — decide the
review's precedence first, because it is the only one that changes what the seat *means*.

## ADR-55: The automations pane and its plugin converge on one frame and one fit

**Context.** `tests/automations_pane_handover_gap.rs` recorded ten rows for this pane,
and exactly one was about **drawing**: `no-fitted-name`. The native pane cut a row's
name to `width − marker − summary` with `ui::truncate_ellipsis`; a plugin has no width,
so its copy drew the name whole and the renderer clipped it at the pane's edge. ADR-52
closed the *vocabulary* — `TextStyle::ellipsize` says a run yields its width and the
kernel cuts the group — and the tasks pane adopted it. This pane had not.

A second difference was in nobody's table, because no gate looked at the frame: the
pane built its own `Block` (square corners, unstyled title, `border_focused` when
focused) while `App::paint_plugin_pane` draws `ui::focus_block` (rounded, focus-styled
title, `accent_bright`) — as it must, since a seat decides *where* a pane is drawn and
never *how*.

**Decision.** Both differences are closed in the **native** pane, in this change,
before any handover.

*The fit.* `resolve_rows` loses its `width` argument and keeps the name whole;
`row_node` marks every run of the name `ellipsize` and leaves the marker and the
`— <summary>` tail their intrinsic widths. The plugin declares the same through the
style-table form. Both halves had to land together: a pane that keeps cutting the
string in its own tree while its reproduction declares the fit produces trees that
differ **by construction**, so no width makes them equal — which is why the closed
row's probe reads `resolve_rows` rather than the catalogue.

The loss this repairs is not cosmetic. The left column is ~24 columns at 120, where
every one of this pane's rows overflows (the summary tail alone is ~31), so a clip at
the pane edge took the schedule, the action and the countdown with it — the row's whole
content. The oracle previously compared at a width where the fit was a no-op and
enumerated the narrow case as a known divergence, which means its equality claim said
nothing about the pane at its real size.

*The frame.* `render_automations_pane` builds `focus_block(" Automations ", focus)`.
Three visible consequences, recorded rather than absorbed: rounded corners (matching
the ` Sessions ` block directly above it, which has always drawn them),
`accent_bright` rather than `accent` while focused, and — the one that is not only
styling — an **accent border at `Active`**. `App::pane_focus_level` has always returned
three levels for this pane, `Active` being "the central-pane automation editor or its
run history holds the keyboard"; the native pane collapsed it into `Inactive` and its
own comment said so. `focus_block` draws it, which is the reading every other pane
gives it.

The convergence runs toward the kernel's frame and not the other way: `PaneDecl` gains
no border or title field. A plugin-declared frame would let a pane draw itself as
focused when it is not — the confusion ADR-51 closed by resolving the level from the
focus the kernel owns.

**Rejected.** *Publish the resolved width* — refused for the fifth time, and the
reasons compound: a width is resolved during a frame while the snapshot is published on
the tick, so a pane would cut to the wrong column for one frame after a resize; the two
panes' rects are not the same rect while both exist; and it does not generalise, since
the pane that genuinely needs geometry (the code review) needs wrapping and pairing
rather than an ellipsis. *Publish an already-fitted name* — it would make the trees
equal today with no new vocabulary, and would invert what a pane is: the snapshot would
carry the pane's rendering rather than the model's fact, which `session::pane_context`
exists not to do. *Leave the native pane fitting and enumerate the divergence* — the
status quo, and it is the row that blocks the handover; it also means the enumerated
case is the **normal** one. *Let a seated pane declare its own block* — see above.
*Fold both into the handover* — refused, and this is the ordering rule the change adds
to `migration/handover`: a handover claims that which code draws a pane changed and
nothing else did, and a commit that also restyles a border makes that claim
unverifiable. The cost is accepted — `empty_welcome_screen_renders` moves twice, once
for the corners here and once for the band at the handover — because each move then has
one reason.

**Consequences.** `no-fitted-name` closes, and the gate now asserts that **no drawing
row is outstanding at all** — the state the tasks pane's gate reached before its
handover. `the-module-is-a-model-too` narrows with it: `ui::automations_panel` no longer
owns a width step, so that row is now about the one function with a second consumer
(`row_summary`, which `src/app/automation.rs` calls for the `Ctrl+P` modal). The
oracle's last enumerated divergence is replaced by its opposite — at 44 columns the two
panes paint one frame, and at 30, where the name gets no columns at all, they agree on
dropping it. The thirteen recordings were regenerated from the native builder, which
ADR-42 requires and permits only while that builder exists, and the diff was verified as
a multiset: 49 lines, each the same line plus the word `ellipsize`.

This is also the first pane to depend on ADR-52's "consecutive yielding runs share one
budget" rule in practice. A name split at a global search's matched offsets is several
runs and one string to a reader, and this pane's recordings include three matched
offsets inside a multi-byte name.

## ADR-56: The automations pane is deleted, and a plugin is the pane — keys given back

**Context.** `tests/automations_pane_handover_gap.rs` recorded ten rows. Four had
closed (the seat, ADR-46; the focused border, ADR-51; the render trigger, ADR-49; the
fitted name, ADR-55). Of the six left, **five stopped being requirements** the moment
the pane took ADR-51's route instead of holding its own keys — and each row said so in
its own words, because the gate was written to measure the other route.

The pane's port (ADR-41) was the furthest any had got: `input`, `automations-write`, a
cursor of its own across renders, five of seven keys reaching the database. It could
never reach the other two. `n` creates a record, and the write seam has no creation
operation *by construction* (ADR-35: creation has no id to address, so a grant to
create is a grant to add rows without bound). `Enter`/`e` opens a **central-pane**
editor, which is text authoring `automations-write` is defined to exclude, into a focus
a plugin cannot take. And a pane focused as `InputFocus::PluginPane` loses the editor
*and* the run history outright, because `App::render_central_pane` branches on three
native focuses — which is what the gate's deciding row said.

**Decision.** `src/ui/automations_panel.rs` is deleted. The band beneath the session
list is `src/plugin/bundled/automations/init.luau`, drawn from the existing
`left-bottom` seat, gated by `[features] automations`, declaring
`key_context = "Automations"` — and declaring **neither** `input` nor
`automations-write`, nor any binding of its own.

All seven scoped actions resolve while the pane holds focus and the kernel performs
them: `j`/`k` move its cursor and wrap into the session list, `Space` toggles, `r`
requests a run, `d` deletes, `n` creates, `Enter`/`e` opens the central editor. The
editor and the run history appear because the pane is focused as
`InputFocus::Automations`, which the branch above already names.

**The finding is the reduction.** The pane that looked like it needed the widest grants
needs the fewest: four capabilities to two, and its five ported keys — the first
evidence that a plugin pane's keys *can* act — turn out never to have been how this pane
should be driven. This is the first handover that makes a shipped manifest's reach
smaller.

Five decisions beyond that.

**`row_summary` moves to `src/ui/automations_list_modal.rs`.** It was the pane's, shared
with the `Ctrl+P` list modal so two native surfaces could not disagree; the modal is the
surface that still composes it. Not `ui/mod.rs` (the layer's *shared* vocabulary, where
`format_countdown` belongs because three surfaces format a countdown) — this is one
surface's row format. Not `app::automation` beside the `format_automation_summary` that
calls it: it is display-text composition, and the coordinator is the wrong layer for it
with its own helper left behind in `ui`.

**`show_automations_pane` becomes a claim, not a flag.** `layout_for` reads
`seat_taken(PaneSlot::LeftBottom)`. ADR-50's rule for its reason: a flag nobody paints
from still carves a band.

**The pane seeds visible, and that generalised a rule rather than breaking it.** Every
previous bundled pane seeds hidden, and both earlier handovers replaced a kernel flag
initialised to `false`. This band was **always on screen** and had no toggle action at
all, so `tests/bundled_manifests.rs` now records the native default per entry and
asserts the replacement seeds *at* it — with "a pane that seeds hidden must bind an
action" as the consequence rather than a second rule. That is stronger, not weaker: it
also catches a pane seeding hidden when the band it replaced was visible, which the old
form silently permitted. The test's own doc had predicted this ("a later handover of a
pane that *did* default to visible (none does today) would want the opposite value for
the same reason").

**The band arrives after the first frame, and that is accepted.**
`docs/SPIKE-SESSION-LIST.md` predicted it: the host starts detached, the first frame
does not wait for it, so "a plugin session list either pops in a moment after the first
frame … or the first frame has to block on a VM". The info panel and the tasks pane both
seed hidden, so neither exhibited it. This one does — the left column is the session
list alone until the host publishes, then it splits. Carving the band from the feature
flag instead would leave it *blank* whenever the pane is absent for any other reason (a
plugin that failed to compile, a manifest with no such pane, a build with no host),
which is the empty-column failure the teardown gate exists to prevent; and blocking the
first frame is forbidden by `plugin-host/panes`. The residual cost is a startup
question — how soon the host publishes — and it is the same question for every pane that
follows.

**The wrap is decided: it stays the kernel's, and needs no owner.** The left column is
one circular list, and with both its panes becoming plugins "where does the wrap live"
looks like a new question. It is not. The wrap is four lines in two kernel handlers
moving `self.focus` between `InputFocus::SessionList` and `InputFocus::Automations`, and
on ADR-51's route a handed-over pane is focused *as* the kernel's pane of that name — so
both ends are kernel focuses whoever draws either pane. It survives one handover, both,
or neither. What changes is its **condition**: `features.automations` was a proxy that
held only while the kernel drew the band unconditionally, and it becomes "a pane
provides the automations list", or `j` at the last session would drop focus into a pane
nobody can see. The plugin's own declining half is deleted, since on this route the
plugin is never asked.

**The oracle keeps its recordings, loses the builder, and keeps one rule.** The thirteen
`.snap` files are byte-identical after the deletion (`git status tests/snapshots/` empty
— the payoff of ADR-42). The `automations_tree`/`resolve_rows` edges go, and the five
key tests go with the keys they measured. But
`the_plugin_composes_the_summary_thurbox_composes` **stays**: its right-hand side is
`row_summary`, which survives because the modal composes it, so that edge is not
differential — and it is the only assertion holding the pane to a *rule* rather than to
a fixed set of cases (192 combinations of schedule × action × enabled × countdown). The
general lesson, now in `migration/handover`: deciding what a handover deletes means
asking of each edge whether its right-hand side is going, not whether the change is a
handover.

**Rejected.** *Keep `input` and add the two missing operations* — two new grants, one
refused on its merits and one excluded by the capability's own definition, to reach a
strictly worse outcome: the pane would still lose the editor and the run history,
because those follow a focus. *Teach `render_central_pane` about a plugin pane* — the
same fact ADR-51 already encodes, and encodes better by reusing `InputFocus`, so the
key-context resolver, the ring, the editor's return paths and `Esc` need no arm; a
second mechanism for one fact is how a handed-over pane comes to *almost* work. *Carve
the band from the feature flag* — see above. *Move `row_summary` to `app`* — see above.
*Keep the pane's five bindings alongside the declaration* — the manifest refuses both
routes in one pane, and a pane holding capabilities nothing exercises is reach an
installed plugin should not have.

**Consequences.** `tests/automations_pane_handover_gap.rs` is retired. Its five
structural rows are preserved here rather than deleted, because **none of the powers
they named was granted** — the pane is handed over because the keys were never the
plugin's to hold, and these are still the answer for a pane that wants keys of its own:

| Row | What it named | Still absent? |
|---|---|---|
| `central-seat-follows-the-native-focus` | a plugin pane driving the central seat | yes — the branch names three native focuses and no plugin pane |
| `no-creation-operation` | a creation binding on the write seam | yes — ADR-35, by construction |
| `no-authoring-operation` | an operation writing a field the user typed | yes — `automations-write` excludes text authoring |
| `wrap-out-of-the-pane-is-unowned` | an action meaning "leave this pane downward" | yes — no such action; the kernel's own handlers do the wrap instead |
| `pane-is-not-told-its-own-focus` | a published per-pane focus flag | yes — `session::pane_visibility` publishes visibility only |

The teardown gate's `automations-plugin` row is the third `ready` pane row, and
`EXAMPLE_BLOCKED_PANE` moves to the **file viewer** — the closest of the four that
remain, whose refusal (ADR-54) is the record of what it still needs.

One new behaviour found while driving it, worth stating because nothing asked for it:
every pane gets generated `<plugin>.<pane>.{show,hide,toggle}` commands (ADR-32), so a
user can now **hide the automations band** with `thurbox-cli command run
automations.automations.hide` and the choice persists. Since the pane binds no toggle
action there is no keyboard way back — the same command with `show` is the way. That is
strictly more than v1 offered (which had only `[features] automations = false`, a config
edit), and it is not a trap: `thurbox-cli plugin list` names the pane and its visibility.
A pane that binds an action would not have this shape, which is the reason
`a_handed_over_pane_seeds_at_the_native_panes_default` requires an action of a pane that
seeds *hidden* and forbids one of a pane that seeds visible.

**Breaking:** a build with no plugin host (`--no-default-features`) has no automations
band, and with it no central automation editor and no run history — the pane is the only
door to `InputFocus::Automations`. It is a smaller loss than the tasks pane's:
`thurbox-cli automation` is untouched, the TUI still fires due schedules, the heartbeat
keeper still fires them headless, and `Ctrl+P` still opens the list modal and its
overlay editor, which is a complete authoring surface reached through a `Modal` rather
than a pane. `plugins` is in the default feature set, so no install is in this position.

## ADR-57: The session list is not handed over — the window, not the keys

**Context.** With the automations pane handed over (ADR-56) the session list is the left
column's only native pane, and it is the pane ADR-V1 hinges on: the v2 design says every
user-visible surface is a plugin *including the session list*, so a session list that
could only be kernel-drawn would make the plugin surface second-class by demonstration.
`docs/SPIKE-SESSION-LIST.md` measured whether it *could* be a plugin and answered yes on
three conditions, all of which now hold.

`tests/session_list_pane_handover_gap.rs` recorded nine rows. Three of them were about
the pane's **keys** — the wall the gate was written around — and ADR-51 answered all
three without granting anything.

**Decision.** The pane stays. `src/ui/project_list.rs` is not deleted, and the gate is
re-verdicted rather than left to expire.

*Three rows close on a conjunction*, in ADR-54's shape: the route is declarable, maps to
`InputFocus::SessionList`, is still resolved by `focus_key_context`, **and** the power
the row named is still absent. The second half is load-bearing — a probe reading only
the route would report `closed` after someone granted a view write, and the record that
the grant was *unnecessary* would be gone.

| Row | What it named | Granted? |
|---|---|---|
| `scoped-keys-silenced-by-the-handover` | a plugin pane whose scoped keyboard resolves | no — the *kernel's* pane of that name resolves it |
| `no-active-session-write` | a binding that moves the active session | **no**, and the row now asserts that |
| `no-session-record-write` | a seam operation addressing a session | **no**, still five task/automation operations |

*What decides the verdict is the **window**.* `render_session_section` hands its nodes to
a ratatui `List` with a `ListState`, and four behaviours are read back off that widget's
sticky `offset()`:

| Behaviour | Native | A seated plugin pane |
|---|---|---|
| Which rows are on screen | ratatui's sticky offset, over **items** | `visible_window(len, cursor, height)`, over **children** |
| `▲ N` / `▼ N` indicators | `render_scroll_indicators_variable`, from the offset and per-item heights, painted **on the border** | nothing — no chrome node |
| Click hitboxes | computed *after* the stateful render; a two-line item (group header + row) is **one** hitbox | one per child, so a header is separately clickable |
| The pending-spawn placeholder | inserted into the **items** vector at a computed index | nothing published says a row is a spawn in flight |

And the counts differ: the native item list folds a repo-group header into the row below
it, so eight sessions in one group is eight items and nine lines, while the plugin's list
is nine children whose declared cursor index counts the header — which
`the_two_panes_window_a_long_list_by_different_rules` already asserts. So "both keep the
cursor visible" is true and insufficient: at any height where the list overflows the two
panes show **different sessions**, in the pane whose selection decides what the central
pane, the info column, the file viewer and the code review are all displaying. That is a
behavioural change, not a rendering divergence, and it is why a degraded session list
would be a broken product rather than a cosmetic regression.

*Two rows are promoted out of the oracle.* `tests/bundled_session_list.rs` documented
three enumerated divergences in `///` blocks, of which only the centred empty state had
a gate row — for no recorded reason. The windowing rule and the non-ASCII trim (the
kernel uses `str::trim`, the plugin Luau's ASCII-only `%s`, so a no-break space around an
activity title survives) are now rows too. The port keeps its `assert_ne!`s: those fail
when a divergence *closes*; the rows fail when the tree stops matching the verdict. A
divergence documented only in a test's doc comment is a verdict written in prose, which
is the expiry the gate exists to prevent.

*The wrap is not a blocker, and that is asserted.* The left column's circular list looked
like a question this handover would owe an answer to. ADR-56 settled it: both ends are
kernel focuses whoever draws either pane, and the condition is already "a pane provides
that list". `the_left_columns_wrap_is_not_a_blocker` pins both facts, because the wrap
*was* a row in the automations pane's gate for as long as that pane held its own keys, and
a reader will otherwise re-derive it as one here.

**Rejected.** *Teach `visible_window` the widget's sticky offset* — that helper is what
every plugin list and three native panes scroll by, so a change for one pane changes all
of them; ADR-39 recorded the same hazard from the other side, when the file viewer's
handover was found to have to *relocate* it. *Move the native pane off its list widget as
groundwork* — the honest closure, and refused **here** rather than outright: four
behaviours come off `ListState` and each has a consumer that is not the paint (border
chrome, the click registry, `App::pending_spawn`), so doing it in the same change that
re-verdicts a table is how a regression in primary navigation ships. It is item one of the
ordering. *Relocate the module now* — ADR-54 refused the same thing for the file viewer
and the reason holds: `resolve_rows` is one of the functions a windowing seam moves, so
its destination is decided by a rule that is not written. *Publish a resolved width or a
pre-windowed row set* — the fifth and sixth refusals of publishing geometry (ADR-55
carries the argument). *Hand it over and enumerate the window as a divergence* — that is
what the port did, correctly, for a *reproduction*; a reproduction may differ and a
replacement may not.

**Consequences.** The gate is eleven rows: two structural (the window, the module), three
vocabulary (the border chrome, the centred empty state, the pending-spawn row) plus the
trim, no wiring, and four recorded closed. `the_window_is_settled_before_what_depends_on_it`
asserts the ordering — the window first, because the indicators, the hitboxes and the
placeholder are all functions of it and because `resolve_rows` feeds both panes; then the
module; then the drawing rows. `the_verdict_is_derived_from_the_blockers` asserts
positively that the three closed rows are **not** structural blockers, so a change that
granted one of the powers they named fails there with the reason attached.

Four panes remain native. Two of the four (this one and the file viewer) are now blocked
on the *same class* of thing — a module that is simultaneously the pane's renderer and the
kernel's model — which is a shared host decision rather than two pane problems, and is
worth taking as one piece.

## ADR-58: The file viewer is deleted, and a plugin is the pane — a seat with two occupants

**Context.** The file viewer's handover was refused in ADR-54 with four rows outstanding
(`tests/file_viewer_pane_input_gap.rs`), and the important thing about that table is what
had already *stopped* being a requirement. It began as an **input** gate: seven
`KeyContext::FileViewer` actions, two of which reach outside the process — expanding a
directory **reads the filesystem**, opening a file **launches `$EDITOR`** — and the brief
asked for the minimum widening of `Capability::Files` that would let a plugin do them.
ADR-51 answered a different question, and five rows closed with no grant: a pane that
declares it *is* thurbox's file viewer is focused as `InputFocus::FileViewer`, and the
kernel resolves and performs all seven actions itself.

So the four that remained were decisions, not powers, and
`the_verdict_is_derived_from_the_blockers` asserted exactly that — every outstanding row
was `Gap::Vocabulary`, nothing structural was left.

**Decision.** The pane is handed over. `src/ui/file_viewer.rs` (1601 lines) is deleted;
the column is `src/plugin/bundled/file-viewer/init.luau`, drawn from a new `file-viewer`
seat, bound to `ToggleFileViewer`, gated by `[features] file_viewer`, declaring the
`FileViewer` keyboard. **No capability was widened**: `files` still publishes a basename
per row and nothing else — no path, no contents, no directory listing, no query — and the
gate's three structural rows are preserved in the table below rather than deleted with
their tests, because "the grant was unnecessary" and "the grant happened" must not become
indistinguishable.

| The row said the pane needed | What it needed instead |
|---|---|
| a filesystem read, to fill a directory on `l`/`Enter` | nothing: the kernel keeps the key, so `FileViewerState::activate` keeps doing the read |
| a process launch, to open a file in the editor | nothing: `App::file_viewer_expand` keeps calling `open_file_in_editor` |
| a view write, for all seven keys | nothing: they resolve against `App::file_viewer` because the pane declared the keyboard |

**Four decisions, in the order the gate stated them.**

**1. The seat is named** — `PaneSlot::FileViewer` → `RegionId::FileViewer`, ADR-53's
argument applied a second time: the right column's occupants are drawn in a fixed order,
so a `right`-slot pane lands to the *right* of this pane's position, and a position within
a column is part of the pane.

**2. The seat has a second kernel occupant, and it preempts.** This is the first seat
where ADR-46's rule — a visible plugin pane takes its seat — is the *wrong* rule. The code
review's changed-files list is force-shown into this same column, with its own focus
(`InputFocus::ReviewFiles`) and its own keys, and ADR-45 records that list as wanting
`RegionId::FileViewer` specifically. Under ADR-46 a claim would win and opening a review
would draw a working-tree file tree where the changed files belong, while `Ctrl+L` landed
on a list nobody could see.

`App::seat_preempted` is the rule: while `active_review().is_some()` the seat belongs to
the review's list, `render_plugin_panes` skips it, and `layout_for` carves the column for
the claim **or** the review. Three properties make it preemption rather than sharing —
the two never coexist (the list *replaces* the tree in that column by design), the plugin
is told nothing and keeps its **stored** visibility (so closing the review restores
exactly what the user had, with no keystroke), and the precedence is the kernel's. A
manifest cannot declare it, deliberately: a plugin cannot see thurbox's surfaces, and a
declared precedence would let one independently-written manifest outrank another with
nothing able to arbitrate. Rejected alternatives: a *second region* for the review's list
(empty in every configuration but one, and the layout would have to know which to fill);
and *waiting for the code review's handover* — that handover is refused for **structural**
reasons this change cannot close, so the file viewer would have waited indefinitely for
nothing.

**3. Seat chrome widens from a row to a band.** ADR-53 made the tasks pane's hint row
kernel chrome; the search bar is the same mechanism at three rows, a border and a block
cursor. `App::pane_hints` becomes `App::pane_chrome`, returning a closed set of shapes
(`Hints` inside the frame's bottom row, `SearchBar` as a bordered band **below** the
frame), and `paint_plugin_pane` subtracts a band *before* drawing the frame — the same
`Min(0) | Length(3)` split the native pane made, so the pane's box, content area and row
hitboxes are the ones that pane's content had. Still **data, not a painter**, for ADR-53's
reason. Each shape keeps its own condition: hints follow focus, the bar follows its
sub-mode (visible while a search runs *or* a query is committed, focused or not). It stays
the kernel's because the query, the caret and the counter are kernel state — the kernel
owns the `/` key, so it owns the query, and the `no-query-write` row is why `FilesSnapshot`
carries none.

**4. The module was the model and the window, and splits three ways.**
`FileNode`/`Activation`/`FileViewerState`/`enumerate_paths` move to
`src/app/file_viewer.rs`; `visible_window` — the rule every plugin list and three native
panes scroll by — moves to `src/ui/mod.rs`, the layer's shared vocabulary; the bar's
painter moves to `src/ui/search_bar.rs`. The model goes to `app` and **not** to `session`,
despite `session::review` being the obvious parallel: `session::review` is pure data about
a diff and the git that produces it lives in `git`, whereas `FileViewerState` calls
`read_dir` in `activate`, `reveal_path` and its search expansion — putting it in `session`
would put filesystem I/O in the layer the architecture rules keep free of effects.
`FileRow` is **deleted**: its five fields are `FileNodeSnapshot`'s five fields and the
publication is now its only consumer, so `rows()` yields the published type.

**Two behaviours changed, both named rather than discovered.**

*The scrollbar's **drag** is lost.* The native pane recorded `ScrollTarget::FileViewer`
from geometry its own renderer resolved; `paint_plugin_pane` records no drag target,
because `render_tree_rows` reports row hitboxes and not the track's rect, so the variant
is deleted with it. Wheel scrolling over the column is unaffected (`App::pane_at` resolves
it from the layout). Restoring it means giving the plugin-pane painter a way to report the
track, which is a change for *every* seated list pane and could not be made inside a
handover claiming only that the pane's painter changed.

*The tree is rebuilt on the tick, not in the paint.* The native renderer refreshed the
tree for the active session as it drew; that moves to `tick_core`, immediately before the
publication it feeds, gated on the pane being on screen — which is the native behaviour, a
closed column read no directory.

**Consequences.** `tests/file_viewer_pane_input_gap.rs` is retired. The teardown gate's
`file-viewer-plugin` row is `ready`, and `EXAMPLE_BLOCKED_PANE` moves to the session list.
The oracle keeps its ten recordings — byte-identical after the deletion — and loses the
`file_tree` edge ADR-42 predicted; its two frame tests keep their claims (the kernel
windows the list, the kernel reserves the track) and assert them on painted cells, since
the second builder they compared against is gone.

The code review's `no-second-seat-for-the-changed-files-list` row is **re-verdicted, not
silenced**. It stood on "no slot names `RegionId::FileViewer`"; one does now, and the row
stays blocked for a stronger reason — the seat exists, and this list is its *preemptor*,
so a plugin-drawn review would have to claim a seat its own other half is the reason
nobody may hold. A gate row's probe is a proxy for its reason, and a proxy that stops
matching its reason reports the opposite of the truth.

Three panes remain native: the session list (ADR-57), global search (structurally
unportable) and the code review. A build with no plugin host has no file viewer and no
`InputFocus::FileViewer`; `plugins` is a default feature, so no install is in that
position, and `ToggleFileViewer` says so in that build's own words rather than doing
nothing.

## ADR-59: The code review's keyboard becomes actions — and four capability rows close without a grant

**Context.** `tests/code_review_pane_handover_gap.rs` refused this pane on eleven rows,
and its module doc named the reason ADR-51's route could not reach it: the review's keys
were **not actions at all**. `KeyContext` had six members and none was a review, and
`handle_code_review_key` / `handle_review_files_key` were captures keyed on `self.focus`,
run ahead of the keybinding lookup. There was nothing for a pane's declaration to name.
The refusal recorded the ordering that follows — the keys become scoped actions **first**
— and this is that step, taken in its own change rather than inside a handover for the
reason ADR-52 gives about the frame: a commit that rewrites a keyboard *and* moves who
paints a pane makes a lost key unattributable.

**Decision.** Two key contexts (`CodeReview`, `ReviewFiles`) and 39 scoped rebindable
actions, both contexts in `KeyContext::pane_keyboards()` and mapped by
`App::focus_for_keyboard`. `review_escape_chord` and `handle_review_files_key` are
deleted; `handle_code_review_key` shrinks to `handle_code_review_submode_key`, which
captures only the three sub-modes that own every key while open — the target picker, the
compose box, and the find query while it is being typed.

**Two contexts rather than one**, because the panes disagree about `j`, `k`, `g`, `G` and
`Enter`: the diff walks rows and the list walks files. A single context would branch on
focus inside its dispatcher, which is the capture wearing an action's name, and the F1
editor would show one row for two behaviours.

**The finding.** Four of the refused rows named a **power no capability performs** —
writing a review record, retargeting the diff, reaching the clipboard and the agent,
moving the cursor — and all four closed with **no grant**. The kernel keeps the key and
performs the effect against its own `CodeReviewState`, exactly as it keeps the file
viewer's directory read and editor launch (ADR-58). Each row is re-verdicted met only
when the kernel performs the key *and* the capability it named is still absent, so a
grant appearing flips it back to blocked — which is what stops "the grant was
unnecessary" reading as "the grant happened". `no-cursor-write` is the sharpest of the
four: the table had called it "the cheapest place a view-state write could start", and it
never started.

**Two decided behavioural differences.** The panes stop swallowing unlisted global chords
— `Ctrl+F` and `Ctrl+R` now fork and restart from a review, as they do from every other
pane — because a per-pane allowlist of which globals work is the inconsistency a context
lookup exists to remove. And half-paging moves from `Ctrl+D`/`Ctrl+U` to `d`/`u`: the
capture *shadowed* `DeleteSession` and `OpenRestoreSessions`, which a declared default may
not do (`macos_default_set_has_no_conflicts` reports it), and `d`/`u` are `less`'s
half-window keys, rebindable to `Ctrl+D` by anyone who wants the shadowing back.

**Rejected: adding a `review-write` capability.** It is what the row asked for and it
would have been the host's fourth capability with no consumer. It is also strictly worse
than the declaration: a pane granted the write would still lose every surface its focus
opens — the compose box, the target picker — so two new grants would buy less than one
declaration.

**Rejected: withholding the two contexts from `pane_keyboards()` until a handover.** One
line smaller, and it makes the four re-verdicts unprovable: the rows would rest on "the
kernel would perform this if a pane could declare the context", a promise rather than a
fact, where the gate's whole design is a verdict re-derived from the source. The risk it
opens is closed by construction — `App::session_ring` offers the two review stops only
while `active_review().is_some()`, so a pane declaring either is focusable exactly while
a review is open.

**Consequence.** The handover is still refused, on five rows: the second seat, the
resolved width three layouts divide against, the click's column, the anchored compose
overlay, and the multi-line field inside it. `src/ui/code_review.rs` is unchanged and is
still what thurbox draws. The row that this change taught, and the one a future attempt
should read first: **giving the kernel a key closes the write, not the surface the key
opens.** `c` writes a comment the kernel can perform; the box it opens anchors to a row
of a tree the kernel did not lay out.

## ADR-60: The session list's ordering model leaves the pane that draws it

**Context.** ADR-57 refused this pane's handover on two structural rows. One of them was
never about drawing:

> `the-module-is-the-kernels-model` — `src/ui/project_list.rs` owns the comparator
> `Ctrl+J`/`Ctrl+K` navigate by, the reorder, the sort, the snapshot the *plugin* reads,
> and global search's session matcher.

That is a v1 layering defect that a handover merely happened to trip over.
`compute_session_order` is documented as the single comparator "shared by the rendering
widget and keyboard navigation **so the two never drift**"; `move_in_order` and
`sort_alphabetically_within_groups` are the primitives behind `Shift+J`/`Shift+K`/
`Shift+S`, which renumber `sessions.display_order` densely and persist it. None of it is
drawing, and all of it lived in `ui` — so `App`, the coordinator, called *up* into the
rendering layer for its own model at eight sites, in a crate whose architecture rules let
`ui` see `app` precisely so that rendering cannot become a dependency of the model.

**Decision.** The model moves to `src/session/session_list.rs`, and it moves **before**
the handover rather than inside it.

`migration/handover` already required a handover to relocate the model its deleted module
also held, "in the same change". That rule assumes the handover happens. This one is
refused on the *other* structural row — the window — which this change does not close, so
under the rule as written the model would sit in the rendering layer for as long as that
refusal stands. The rule now carries the exception, on the precedent already beside it:
a pane's **keyboard** is required to become actions in a change before its handover
(ADR-59), for the reason that applies verbatim here — a commit that relocates a model and
moves who draws a pane makes any behavioural difference read equally as either.

To `session` and not to `app`, which is the opposite call from the file viewer's
(ADR-58), and the *same rule* selecting differently. That rule is: a model performing
side effects must not go to a layer kept free of them, however well its types fit.
`FileViewerState` calls `read_dir`, so it went to `app`. Nothing here reads, writes,
spawns or blocks — `compute_session_order` sorts, `move_in_order` swaps index ranges,
`resolve_rows` copies fields out of `SessionInfo` — so it goes to the layer that already
owns `SessionInfo`.

**The cut is geometry.** Everything that is a pure function of the session set moved;
everything needing a resolved width, a ratatui type or a theme stayed. So `SessionRow`
crossed while `resolve_items`, `fit_status_text` and `row_used_columns` did not: the row's
one geometry-bearing field is `None` on every row `resolve_rows` returns and is filled by
the pane, which is the shape the split already had.

Two things deliberately did **not** move, and naming them is the point of the boundary:

- **`pending_spawn_slot`**, though it is as pure as everything that did.
  `migration/phase-4` orders the relocation of anything the widget's window feeds *after*
  the windowing decision, "since what a windowing seam looks like decides where those
  functions live" — and the placeholder's index is one of the four behaviours the window
  row enumerates. Purity is not the criterion; being downstream of an undecided seam is.
- **The width fit**, because `session` may not hold geometry.

Rejected: re-exporting the moved items from `ui::project_list` so no caller changes (the
re-export *is* the defect — the kernel would still spell its own navigation
`crate::ui::…` and the module would still be undeletable, which `migration/handover` now
refuses in as many words); and taking the opportunity to converge the fit onto
`ellipsize` as the automations pane did (ADR-55), which this pane still owes — that
changes what the pane draws and belongs in a change whose whole content is the
convergence, because "no function body was edited" is what makes this one checkable at a
glance.

**Consequence.** `the-module-is-the-kernels-model` closes, and the row keeps asserting
*both* halves — the model is in the pure-data layer **and** the coordinator no longer
names it through `ui` — since a re-export would satisfy the first alone.
`the-window-is-the-list-widgets` is now the pane's **sole** structural blocker, which the
gate asserts as an equality rather than a membership. No pane is handed over: the renderer
exists, `src/app/view.rs` still calls `project_list::render_left_panel`, the bundled
plugin stays hidden with no `input`, and the teardown gate's row stays blocked.

**The window, measured rather than restated.** ADR-57 recorded that the two rules
"differ". What a reader could not re-derive from that is the size of the difference, so it
is recorded here. 40 sessions across four repo groups, pane inner height 30, cursor walked
down one row at a time: `native` is `ListState::offset()` after the stateful render,
`shared` is `ui::visible_window` over the flat children a plugin pane declares.

| cursor | native item offset | shared child window |
|---|---|---|
| 0 | 0 | 0..30 |
| 3 | 0 | 1..31 |
| 5 | 0 | 3..33 |
| 10 | 0 | 9..39 |
| 20 | 0 | 14..44 |
| 28 | 1 | 14..44 |
| 39 | 12 | 14..44 |

The native pane does not scroll at all for the first 28 of 40 rows — the cursor walks down
the pane and the list holds still. The shared rule scrolls after the *third* keypress and
is pinned to the list's tail for the whole second half. Adopting it in the native pane
would be a visible regression in the pane every user navigates with, and the spec already
refuses the converse (redefining the shared rule, which every plugin list and three native
surfaces scroll by, to match one pane's widget).

Two further halves of that row are untouched by whichever scroll rule is chosen, and are
why it is not merely a policy question (**both closed by ADR-61**, which made a list's
window a quantity in rows so that a two-line child is expressible *and* scrollable):

- **Item granularity.** A repo-group header travels with the row below it, so a two-line
  item is **one** hitbox and the window can never split a header from its row. A plugin's
  list emits the header as its own child.
- **Click index space.** `App::render_plugin_panes` maps a seated pane's hitbox index to a
  kernel row as `row(index - 1)`, which holds for all four panes handed over because each
  emits one child per row. This pane's children include headers, so the mapping is wrong
  by the number of preceding headers and the error grows through the list. A handover
  today would ship a session list whose clicks select the wrong session.

## ADR-61: A plugin list's window is a quantity in rows, so a row may be more than one line

**Context.** ADR-60 left `the-window-is-the-list-widgets` as the session list's sole
structural blocker and, at its close, named two halves of it that no choice of scrolling
policy answers: a repo-group header travels with its session as **one** item natively and
is a separate child in a tree, so the plugin's index is not the kernel's, and
`App::render_plugin_panes`' `row(index - 1)` mapping is wrong by the number of preceding
headers — an error that grows down the list.

The tree could already *express* the grouping. A `Column` inside a `List` is a valid
child, already gets one rect from `render_stacked`, one hitbox from the row sink and one
index for the cursor. What it could not do is **scroll**: `ui::visible_window` counts
children and assumes each is one line, so a list of two-line items in a ten-line pane was
handed ten items, five of which were painted and five clipped — including, low enough in
the list, the cursor's own. The grouping was expressible and unusable.

**Decision.** The layer's windowing rule is generalised from "N children in H rows" to "N
items of declared heights in H rows" (`ui::visible_item_window`), and `visible_window`
becomes a wrapper over it with unit heights. A plugin list resolves its window through the
general form, measuring each child with the same `height_of` walk that will draw it. A
declared scroll track measures the same quantity: whether the list overflows, how long its
content is, and where the thumb sits become row counts rather than child counts.

**The reduction is the safety property, and it is proved rather than argued.** Every list
in the tree today has one-line children, so the whole risk is a disagreement between the
two forms. `the_general_rule_reduces_to_the_uniform_one` walks every
`(total ≤ 24, selected < total, height ≤ 24)` triple against the pre-change rule kept in
the test as a reference, and requires the identical pair. Two clauses in the general rule
exist only for cases unit heights cannot produce — an item taller than the whole pane, and
a tall item above the cursor eating the margin the window opened with — and each is
documented as unreachable for them, which is what the exhaustion then confirms.

**Rejected: teaching the shared rule the widget's sticky offset.** The obvious way to make
the two panes agree is to give the shared rule ratatui's policy, since that is what the
pane being handed over does. Refused twice over: that helper is what every plugin list and
three native panes scroll by, so a change for this pane changes all of them (ADR-39's
hazard from the other side); and the widget's rule is *stateful* — it takes the previous
offset as an input — where the view-tree renderer is deliberately a pure function of
`(tree, frame table, palette)` with no state and no path back to a VM. Making it sticky
means a per-pane, per-node offset table threaded through the renderer the way `FrameTable`
is. That is a real design with real precedent (`App::motion`) and it is a *scrolling
policy* change wearing a *plumbing* change's clothes. Nothing here forecloses it: it would
be a second implementation of the same signature, chosen by a declaration on the node.

**Rejected: a new `item` node kind.** `ui.list({ ui.item({header, row}), … })` reads well
and is what ratatui's own `ListItem` is. It would put a second spelling of an existing
container into a catalog whose stated discipline is that it holds "the set thurbox's own
panes need, not a general drawing API", and every walk over the tree — `children`,
`depth`, `node_count`, `is_inlineable`, `height_of`, conversion, the recorder — would grow
an arm saying "same as a column". The cost accepted in exchange is that grouping is a
convention rather than a type: a plugin that forgets to wrap gets exactly today's
behaviour, which is the failure mode worth having.

**Rejected: publishing the rows already grouped**, so the plugin's array and the tree's
children are 1:1 by construction. That is the rule ADR-29 set and every port since has
applied — the kernel publishes a *rendering* only when two panes must agree about it — and
a group header is one pane's presentation of a fact (`row.group`) the snapshot already
carries. It would also help no other pane with a multi-line row.

**Consequence.** `the-window-is-the-list-widgets` stays **blocked**, and its `stands` is
rewritten to record which half moved: the two panes no longer disagree about *what a row
is*, only about *which rows sit beside the cursor*. Its probe is tightened to name
`visible_item_window` — the fourth time in this family of gates that a needle has had to
be updated rather than a verdict flipped, and this one is instructive: the old needle went
on matching by accident, because a *test* in the same file still calls the uniform form.

No tree changes. `session_list_tree` still emits a header and a row as two children and
the bundled plugin still flattens them, because adopting the item shape moves the recorded
goldens — the handover's change to make and to justify. Cost per frame is one `height_of`
call per child, and only for a list that declares a cursor or a track; a list that
declares neither measures nothing and takes the path it always took.

## ADR-62: A centred line, because the last placement rule could not be built from the parts

**Context.** `tests/session_list_pane_handover_gap.rs` held `no-centred-line` against the
session list's handover: the empty state (`No sessions yet`, `Press Ctrl+N to create one`)
is drawn centred, and every node in the catalog draws from the left. Small, and a handover
would ship it — the session list is empty on a fresh install, so the first frame a new
user sees is the one the row is about.

It is also the last placement rule the catalog was missing. Left is the default;
flush-right has had a spelling since ADR-31 (a `Fill` before a run, whose residue the
kernel resolves); centre had none.

**Decision.** `ViewNode::Center(Vec<ViewNode>)` — inline runs packed on one row, that row
placed centrally in the width the node is given, by the kernel. It admits exactly what a
line admits, clips the same way, is one row tall, and is not itself admissible inside a
line, because its width comes from its area rather than from its content.

**Rejected: a fill on either side.** A plugin can already write `line(fill, run, fill)`
and `inline_spans` splits the residue between the placeholders. That is centring to within
one column, and one column is the wrong kind of nearly-right: the remainder goes to the
**first** placeholder, where ratatui's `Alignment::Center` — the call the native pane
makes — leaves it on the **right**. So a pane built this way is off by one whenever the
residue is odd, against the pane it is reproducing, at half of all widths.
`the_odd_column_falls_where_the_kernels_own_centring_puts_it` pins both sides of that.
Changing `Fill`'s remainder rule instead is refused outright: it is load-bearing for a diff
row's tint and a group header's trailing rule, and it would move frames in panes with no
stake in this.

**Rejected: an `align` field on `TextStyle`.** The gate's probe accepted it, so it was on
the table. It is not true of a run: two runs on one line could declare different
alignments and the host would have to arbitrate or let the first win. Every other field of
`TextStyle` says how *that run* is drawn. `ellipsize` is the near miss worth naming — a
per-run field that is really a rule about the line — and it earns that because several
yielding runs share one budget, which is a genuinely per-run fact. Centring has no such
reading.

**Rejected: an alignment field on `Line`.** The model's answer, and it would give
right-alignment a clearer second spelling for free. `Line` is a tuple variant at 48 sites
across five modules and the golden recorder, 21 of them patterns. A change whose whole
content is one placement rule would arrive as a mechanical rewrite touching three native
panes and the recorder — and the recorder prints node shape, so recordings would move for
reasons unrelated to centring, in the artifacts that are the only evidence four
handed-over panes have. If a second alignment consumer appears, `Center` collapses into
the field in a change whose content is that collapse.

**Why a third node is not a second spelling of `Line`.** The objection this change has to
answer, having refused `Item` one commit earlier (ADR-61) on exactly that ground. The
catalog already holds two nodes that take the same children and differ only in what
happens when they run out of room — `Line` clips, `Paragraph` wraps. A third taking the
same children and differing in where the row sits is that grain, not a novelty. `Item`
was refused because `Column` already *was* it; nothing here already centres.

**Consequence.** `no-centred-line` is re-verdicted **met**, on the rule the two seat rows
already use: a row closes when the route exists, not when the reproduction takes it.
Neither pane adopted it — the native one still returns early and draws a `Paragraph`, the
bundled plugin still emits two left-aligned rows — and
`the_empty_pane_is_the_one_place_the_plugin_differs` goes on asserting they differ, so
"the vocabulary exists" cannot come to read as "the panes agree". The row's probe also
asks that the placement stayed the kernel's, so a later change reporting a column or an
offset back into a VM reopens it.

**One note on where the constructor lives.** `ui.center` joins the loop in
`plugin::capabilities::build_ui_table` that already builds `row`, `line`, `paragraph` and
`column`. That file's name notwithstanding, this grants **no capability**: `Capability` is
unchanged, `build_module_table`'s bindings are unchanged, and nothing a plugin may read,
write, run or reach moves. The `ui` table is the node vocabulary and is frozen after
construction.

## ADR-63: The session list's window converges onto the kernel's rule, in the direction of the pane

**Context.** `tests/session_list_pane_handover_gap.rs` refused the session list's handover
on one structural row, `the-window-is-the-list-widgets`, and after ADR-60 relocated the
model it was the sole decider. Two problems hid in it.

**Identity** was the first, and ADR-61 closed it: the native pane's list item is *an
optional repo-group header plus a session row* — one item, one hitbox, one index — while
the plugin flattened the pair into two children, so a session's index in the tree was not
its index in the kernel's rows and the drift grew with every group. Windowing in rows made
the fold expressible.

**Policy** was the second. The kernel's shared rule (`ui::visible_item_window`) opens a
margin above the cursor and clamps at the list's tail; ratatui's `List` holds its offset
until the cursor leaves the viewport. Both keep the cursor visible and they disagree about
which rows sit *beside* it — measured at 40 sessions in a 30-row pane, the widget does not
scroll until the cursor reaches row 28 while the shared rule scrolls after three
keypresses. For the pane whose selection decides what the central pane, the info column,
the file viewer and the code review are all showing, that is a behavioural change.

**Decision.** The **pane** converges onto the kernel's rule, in a change before its
handover.

`ui::project_list::render_session_section` draws its block and paints its list through
`ui::plugin_pane::render_tree_rows` — the renderer its reproduction already goes through.
The window is `visible_item_window` for both, the `▲ N` / `▼ N` indicators and the click
hitboxes are read off that one paint, and the pending-spawn placeholder is an index into
the same folded items. `ListState` leaves the pane and `App::session_list_state` is
deleted with it. Both trees fold a header into the row it heads, so one index names the
same row in both.

**The direction is the whole argument.** `migration/phase-4` forbids closing this row by
redefining the kernel's helper to match one pane's widget: that helper is what every
plugin list and three seated panes scroll by, and a change for one pane would change all
of them (the hazard ADR-39 recorded from the other side). It says nothing about the pane —
and the pane is the thing this handover deletes. The precedent is the frame: ADR-53's rule
that a native pane whose frame differs from the host's is converged onto the **host's**
frame, in its own change, before the handover. A window is the same kind of thing as a
frame — a property of how the host draws a pane — so a handover must not be able to change
one under cover of moving the drawing code.

**What changed for a user, stated rather than discovered.** An overflowing session list now
opens `min(height/4, 3)` rows above the cursor instead of holding a sticky offset. Moving
into a long list jumps the window once rather than scrolling row by row from wherever it
was left. The cursor is visible either way. After this, thurbox has **one** windowing rule
for every list it draws; the session list was the last pane with its own.

**Rejected alternatives.**

| Alternative | Why not |
|---|---|
| Teach `visible_item_window` the widget's sticky offset | It is *stateful* — an offset carried between frames — where the view-tree renderer is a pure function of `(tree, frames, palette)`, and it would have to be reachable from a plugin pane's paint, which holds no mutable kernel state. And it changes four panes to fix one. |
| Keep the widget and make the plugin match it | A plugin is never told its height, so it cannot window anything. The window is the kernel's by construction (ADR-30); the only question was which kernel rule. |
| Converge inside the handover | It makes the handover's claim unverifiable: every moved cell would have two candidate causes, and the recorded expectation would move for two reasons at once. |
| Fold the header only in the plugin | The two would still count different numbers of rows, so the windows could not agree however correct each was. |

**The recording moves, and it is recorded from the native tree.** The eleven
`tests/snapshots/bundled_session_list__*.snap` files are regenerated with the folded
shape, from `session_list_tree` — the edge that gives them provenance, establishable only
while that builder exists (ADR-48's fourth handover condition). The enumerated divergence
in `tests/bundled_session_list.rs` is replaced by its opposite: at a height where the list
overflows, both panes draw the same rows and clip the same counts.

**`ui::draw_clipped_indicators`** goes to `src/ui/mod.rs`, beside the windowing rule, for
that helper's reason — the counts are a function of the window, and a seated plugin pane
whose frame the host draws is its second consumer the moment the chrome row is closed. It
takes a `&mut Buffer` because the view-tree painter has only the buffer, and two painters
of the same glyphs must not diverge on where they land.

**What this does not close.** Three vocabulary rows remain and the gate goes on refusing
the handover: the pane's border chrome (the one-dot-per-session strip and these very
indicators, on a frame a plugin does not draw), the pending-spawn placeholder row, and the
Unicode-aware trim of an agent's activity text. `pending_spawn_slot` deliberately stays in
the pane — it is downstream of the seam this change settled, and moving it is the
placeholder row's work.
