# Thurbox v2 — Decision: In-Process Luau Plugins

[ADR-V2](ARCHITECTURE.md#adr-v2) puts plugins in a sidecar process. This
document reopens that on performance grounds and answers the question it
raises: **can in-process keep isolation and reload safety?**

Short answer: **yes for everything except native code, and only under three
conditions.** The conditions are the whole decision, and one of them couples
the process model to the language choice in a way that is easy to miss.

Status: **decided — in-process Luau via `mlua`.** The design set has been
updated to match ([ADR-V2](ARCHITECTURE.md#adr-v2),
[ADR-V3](ARCHITECTURE.md#adr-v3), [ADR-V4](ARCHITECTURE.md#adr-v4),
[ADR-V14](ARCHITECTURE.md#adr-v14)). This document is kept as the reasoning
behind it and as the record of what was traded away — §6 and §7 are the parts
to re-read if the decision is ever revisited.

The decision is taken on analysis, not measurement. §7's validation gate is
what turns it into evidence, and §9 states the reversal conditions.

---

## 1. Where out-of-process actually costs

Worth measuring the premise before optimizing it. Most of the obvious IPC costs
were already designed out:

| Path | IPC cost today | Why |
|---|---|---|
| Rendering a frame | **none** | Frames paint from cached view trees; plugins push on state change ([ADR-V11](ARCHITECTURE.md#adr-v11)) |
| Animation | **none** | The kernel owns the clock ([ADR-V18](ARCHITECTURE.md#adr-v18)) |
| Terminal / diff / file tree content | **none** | Kernel surfaces carry identifiers, never content ([ADR-V6](ARCHITECTURE.md#adr-v6)) |
| Key event | one small notification, fire-and-forget | Not awaited |
| Host call | one round trip, off the frame path | Async by contract |

So the frame budget is not where the cost is. What remains is real but
different from what "IPC is slow" suggests:

| Real cost | Magnitude | Notes |
|---|---|---|
| **Memory** | ~15–30 MB RSS per Bun process | With two halves across ~7 bundled plugins this is plausibly 200–400 MB. For a terminal tool this is the cost users would actually notice |
| **Cold start** | ~20–50 ms per process | Hidden by lazy activation until the first time a user opens a pane, when it is visible |
| **Serialization** | 256 KB warn / 2 MB reject per push | A live-filtering pane re-serializes its tree per keystroke |

These are estimates from published runtime characteristics, not measurements.
**Phase 1 should measure them before this decision is made** — if bundled
plugins land at 60 MB total rather than 300 MB, the premise weakens
considerably.

---

## 2. What in-process can and cannot recover

Isolation is not one property. Separating it is what makes the answer
tractable:

| Failure mode | Out-of-process | In-process, done well |
|---|---|---|
| Plugin throws | contained | contained — `Result` in mlua, catch in JS |
| Infinite loop / hang | `SIGKILL` | contained — Luau's VM-level interrupt handler, V8's `TerminateExecution` |
| Memory exhaustion | OS / process kill | contained — `set_memory_limit` per state, V8 heap caps |
| Blocking the render loop | impossible | contained **iff** plugins run on their own threads |
| Corrupting kernel memory | impossible | possible only via FFI |
| **Segfault in native code** | contained | **kills the TUI. Not recoverable.** |

One row is unrecoverable, and it is entirely about native code. That gives the
decision a clean shape:

> **In-process isolation is nearly as good as out-of-process, provided plugins
> cannot execute native code.**

---

## 3. The three conditions

In-process is safe if and only if all three hold. Dropping any one of them
gives away something the current design has.

### C1 — Thread per plugin, never the UI thread

Each plugin gets its own OS thread with its own VM. The render loop never
calls into a plugin synchronously, so a slow or wedged plugin cannot stall a
frame. This preserves [ADR-V11](ARCHITECTURE.md#adr-v11) and
[N2](CONSTITUTION-DELTA.md) unchanged — the mechanism moves from "another
process" to "another thread", and the guarantee is identical.

### C2 — No native code in plugins

No FFI, no native addons, no `dlopen`. This is what buys back crash isolation,
and it is not negotiable: a single native module segfaulting takes the whole
TUI with it, which would violate Constitution rule 1 in a way no supervisor
can catch.

For Luau this is the natural state — the host provides every capability
already. For JavaScript it means **pure-JS npm packages only**, which excludes
a real if small slice of the ecosystem.

### C3 — The VM is the unit of reload

Reload destroys the entire VM and builds a new one. Not module-cache clearing,
not selective re-require — full teardown. And the kernel must hold **no handle
that can outlive the VM**.

Rust makes the second half enforceable rather than aspirational: tie every
registry key, callback and pane binding to the VM's lifetime, and dropping the
VM invalidates them by construction. A stale reference becomes a compile
error rather than a mysterious "my edit didn't apply".

This is why in-process reload here would be **safer than neovim's**, which is
the usual cautionary example: neovim reloads *modules inside a shared
state*, so anything that captured a reference keeps the old version alive.
Destroying the state sidesteps that entire class.

Two residual risks, both answerable:

- **Host-held resources** (open files, sockets, spawned children) are acquired
  through host APIs, so the kernel already owns those handles and reclaims
  them on teardown — arguably cleaner than out-of-process, where they die with
  the process but the kernel never knew about them.
- **Native modules cannot be unloaded** — which C2 forbids anyway.

---

## 4. Reload and hot-install under C1–C3

Everything in
[FEATURES-Plugin-API.md §4](FEATURES-Plugin-API.md#4-lifecycle) survives
unchanged in wording; only the mechanism differs:

| Operation | Out-of-process | In-process under C1–C3 |
|---|---|---|
| Install a new plugin while running | Read manifest, spawn on activation | Read manifest, create VM on activation |
| Reload a changed plugin | `dispose()` → respawn | `dispose()` → drop VM → new VM |
| Reload a **wedged** plugin | `SIGKILL` | Interrupt handler aborts, then drop VM |
| State across reload | not preserved | not preserved — same rule |
| Fault backoff | respawn 1s, 2s, 4s… | rebuild VM, same schedule |

Reload gets **faster**, not just equivalent: dropping and rebuilding a VM is
microseconds to low milliseconds, against ~20–50 ms for a process respawn.
`thurbox plugin dev` watch-mode iteration improves noticeably.

---

## 5. Runtime candidates

**The process model and the language choice are not independent.** C2 is what
forces that: "no native code" rules out anything whose ecosystem assumes FFI,
and rewards runtimes where the host provides every capability anyway.

With TypeScript no longer a hard requirement, the field is wider than the
JavaScript options:

| Runtime | Footprint / start | Sandbox | Typing | Familiarity | VM written in |
|---|---|---|---|---|---|
| **Luau** (`mlua`) | KBs, µs | Purpose-built — Roblox runs untrusted code | Gradual + `luau-analyze` | Lua-5.1-shaped | C++ |
| **Lua 5.4** (`mlua`) | KBs, µs | DIY — strip `io`/`os`/`package`, per-chunk `_ENV` | None (or Teal, which compiles to Lua) | **The standard everyone knows** | C |
| **LuaJIT** (`mlua`) | KBs, µs | DIY, and `ffi` must be stripped | None | Lua 5.1 | C + asm |
| **QuickJS** (`rquickjs`) | ~1 MB, sub-ms | DIY | TypeScript, transpiled at install | JS/TS | C |
| **Rhai** | small, fast start | Sandbox-first; operation and memory limits built in | Dynamic, Rust-ish | **Nobody knows it** | **Pure Rust** |
| **Starlark** (`starlark-rust`) | small | Hermetic by construction — no I/O in the language at all | Optional types | Python-shaped | Pure Rust |
| **V8** (`deno_core`) | tens of MB/isolate | None; native addons possible | TypeScript | JS/TS | C++ |

### Reading the field

**Luau remains the pick**, and the familiarity objection against it is weaker
than it first looks: Luau is a fork of Lua 5.1, so ordinary Lua reads and runs
largely unchanged, and the type annotations are opt-in. A plugin author who
knows Lua is not learning a new language — they are choosing whether to add
types. It is not a strict superset (`setfenv`/`getfenv` are gone, some stdlib
differs), but the gap is small.

**Lua 5.4 is the credible alternative**, and the case for it is one thing:
**it is the standard, so everything knows it** — documentation, snippets,
every neovim user, and language models. If "ask an agent to write me a pane"
matters, that gap is real and it runs the other way from every other criterion.
What it costs: no gradual typing (Teal exists and compiles to Lua, but that is
another toolchain), and a sandbox you assemble and own yourself rather than one
the upstream project maintains because its business depends on it. Hang
containment works via debug hooks rather than a purpose-built interrupt.

**Rhai is the interesting outlier.** It is the only candidate written in pure
Rust, which removes the one residual crash risk §6 names — with a C-based VM,
"in-process is safe if plugins carry no native code" still rests on the VM
itself not segfaulting. Rhai has no `unsafe` FFI surface, and sandboxing,
operation limits and memory limits are first-class rather than assembled. The
disqualifier is adoption: a plugin system exists to attract third-party
authors, and asking them to learn a language almost nobody uses is a steeper
tax than any of the safety properties are worth.

**Rejected on inspection:**

- *LuaJIT* — fastest, and frozen at 5.1 with upstream effectively in
  maintenance. Its built-in `ffi` is precisely what C2 forbids, so it ships
  with the sharp edge enabled and must be stripped. JIT also means writable-
  executable pages, which some hardened environments refuse.
- *Starlark* — sandboxed by construction, which is exactly right, and it is a
  **configuration language**: no I/O, and dialects restrict recursion and
  unbounded loops. Excellent for `layout.toml`-shaped problems, wrong for
  something that has to hold state and react to events.
- *V8 / `deno_core`* — the incoherent middle. It keeps the ecosystem but
  surrenders the dependency-policy argument
  [ADR-V2](ARCHITECTURE.md#adr-v2) rests on, and its per-isolate memory gives
  back the reason for going in-process at all.

### One choice, not two

`mlua` compiles against a single Lua backend — the `lua54`, `luajit` and
`luau` features are mutually exclusive. "Support both Lua and Luau" is not
available; this is a real either/or.

---

## 6. What in-process still gives away

Even with C1–C3 satisfied, these are real and permanent:

1. **Native-code crashes kill the TUI.** C2 reduces this to "a bug in the
   embedded VM itself", which is a much smaller surface than "any plugin" —
   but it is not zero, and it is the property Constitution rule 1 currently
   gets for free.
2. **A plugin can consume the kernel's address space.** Per-VM memory limits
   bound it, but fragmentation and allocator pressure are shared.
3. **The blast radius of a VM bug is the whole app**, not one pane.
4. **Debugging changes shape.** A crashed sidecar leaves a corpse to inspect;
   an in-process abort takes the debugger with it.
5. **npm shrinks** to pure-JS packages (or disappears entirely, with Luau).

---

## 7. Recommendation

An earlier draft of this section recommended QuickJS, conditioned on "keeping
the TypeScript decision intact". **That condition has since been withdrawn —
TypeScript is not a hard requirement — and it changes the answer.**

### With TypeScript optional, Luau is the stronger choice

Not marginally. It collapses three separate open problems into one solved one:

| Open problem | Under Luau |
|---|---|
| [SECURITY.md](SECURITY.md) Finding 1 — `fs`/`net`/`shell` declared but unenforceable | **Closed.** The sandbox is the design centre: there is no `io` or `os.execute` unless the host hands it over. Roblox's entire model is running untrusted code |
| [ADR-V3](ARCHITECTURE.md#adr-v3) — 40–90 MB bundled runtime on every packaging channel, plus CVE ownership, plus an unanswered musl / Windows-ARM64 question | **Deleted.** A `mlua` embed is part of the binary. The ADR mostly disappears and takes a risk-register row and a packaging burden with it |
| Plugin memory and cold start (§1) | **Answered by orders of magnitude.** A Luau state is kilobytes and starts in microseconds, against 15–30 MB and 20–50 ms per Bun process |

It also fits C1–C3 (§3) without special pleading: no FFI by default satisfies
C2 outright, `mlua` gives per-state memory limits and a VM-level interrupt for
C1, and destroying a `Lua` state is the natural unit of reload for C3.

And the objection I raised against plain Lua in
[ADR-V2](ARCHITECTURE.md#adr-v2) does not apply: Luau has gradual typing and
`luau-analyze`, so the view-tree contract stays statically checkable.

### The npm objection is weaker than it looks — and this repo is the evidence

The real cost of leaving TypeScript is losing npm. But thurbox's *own*
extensions are a direct sample of the workload, and they use:

| Dependency | Uses across `extensions/` |
|---|---|
| `jq` | 72 |
| `curl` | 15 |
| `gh` | 14 |
| `glab` | 3 |
| npm packages | **0** |

The workload is **shell, HTTP and JSON** — not SDKs. And all three are already
host APIs in this design (`ctx.exec`, `ctx.fetch`, plus a JSON binding), because
the capability model routes them through the kernel by construction. The
bundled plugin set needs even less: `markdown`, `diff`, `fileTree` and `code`
are kernel surfaces, so tasks, files, review and the session list have no
library needs at all.

### What it genuinely costs

Stated plainly, because these do not go away:

1. **The ceiling drops for third parties.** A plugin wanting a chart renderer,
   a YAML parser, or a vendor SDK has npm in TypeScript and nothing in Luau.
   The evidence above is about what thurbox has needed, not what someone else
   might want to build.
2. **Agents author TypeScript better than Luau.** For a tool whose users
   operate agent CLIs, "ask an agent to write me a pane" is a plausible
   workflow, and training-data volume is not close.
3. **[ADR-V14](ARCHITECTURE.md#adr-v14) loses its distribution mechanism.**
   "Widgets live in TypeScript, versioned on npm, forkable" was half an
   argument about *packaging*. Luau has no npm, so `@thurbox/widgets` becomes
   a bundled module or a git-distributed one — which pulls widget versioning
   back toward the kernel, the exact coupling ADR-V14 exists to prevent. This
   is the sharpest cost and it needs an answer before committing.
4. **Contributor familiarity.** More developers know TypeScript, though
   thurbox's audience skews toward people who have written a neovim config.

### So: measure, then commit — with one validation gate

The §1 measurement still runs in Phase 1 and still decides in-process versus
out-of-process. But if it says in-process, **go to Luau**: QuickJS keeps a
language nobody now requires while giving up the sandbox that is Luau's main
prize, and the wider field in §5 does not produce a better answer — Lua 5.4
trades the sandbox and the typing for familiarity, and Rhai trades adoption for
a memory-safe VM.

**The one argument that could flip it to Lua 5.4** is agent authorship. Every
other criterion favours Luau, but models know standard Lua far better, and
"ask an agent to write me a pane" is a plausible primary workflow for this
tool's users. If that turns out to be how plugins actually get written, the
familiarity gap outweighs gradual typing. Worth testing during the validation
gate below rather than arguing about: have an agent write the same pane in
both.

Before committing either way, **write one hard bundled pane in it** — the code
review or the session list, not a hello-world. Those two exercise the
view-tree contract at full width, and a language that makes them awkward will
make every third-party pane awkward. That gate is cheap and it is the only
thing that turns this from a paper comparison into a decision.

---

## 8. What moves if in-process is adopted

So the cost of the change is visible rather than discovered:

| Document | Change |
|---|---|
| [ADR-V2](ARCHITECTURE.md#adr-v2) | Rewritten: thread-per-plugin in-process, C1–C3 as the stated conditions; the sidecar becomes the rejected alternative with its isolation argument preserved |
| [ADR-V3](ARCHITECTURE.md#adr-v3) | Largely deleted — no bundled runtime, so the 40–90 MB artifact cost and its packaging-channel risk disappear |
| [ADR-V4](ARCHITECTURE.md#adr-v4) | "One process per plugin" → "one VM and one thread per plugin"; capability enforcement is unchanged for host RPC, and improves if Luau |
| [ADR-V15](ARCHITECTURE.md#adr-v15) | Lazy activation stops being load-bearing; it stays as hygiene rather than necessity |
| [SECURITY.md §3](SECURITY.md) | Finding 1 either closes (Luau sandbox) or persists unchanged (QuickJS — a JS VM without a permission model has the same gap Bun does) |
| [SECURITY.md §4](SECURITY.md) | Environment scrubbing becomes *harder*, not easier: an in-process plugin sees the kernel's own `std::env`, so the scrub must happen at VM construction by not exposing an env binding at all |
| [MIGRATION.md](MIGRATION.md) Phase 1 | Supervisor work is replaced by VM lifecycle and thread management; the measurement in §7 lands here |
| [CONSTITUTION-DELTA](CONSTITUTION-DELTA.md) rule 1 | Must be amended: "a plugin fault never reaches the user as a broken frame" becomes conditional on C2 |

That last row is the one to weigh hardest. It is the only place where going
in-process forces a Constitution rule to become weaker rather than merely
different.
