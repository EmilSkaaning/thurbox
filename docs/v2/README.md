# Thurbox v2 — Design Documentation

Thurbox v2 turns the TUI inside out: instead of a Rust binary that owns every
pane, thurbox becomes a **plugin host** with a minimal Rust kernel and a Luau
plugin layer that can be reloaded without recompiling.

This directory holds the design set for that refactor. It is **forward-looking
documentation** — nothing here is implemented yet. Until v2 ships, `docs/` (one
level up) describes the shipping product and remains authoritative.

## The shape of v2 in one page

```text
┌──────────────────────────────────────────────────────────────┐
│ Rust kernel (thurbox)                                        │
│                                                              │
│  sessions · backends (tmux/ssh/wsl) · git · SQLite · PTY     │
│  event loop · layout solver · theme · keymap · renderer      │
│  plugin host: VM lifecycle, host bindings, capabilities      │
└───────────────┬──────────────────────────────────────────────┘
                │  one sandboxed Luau VM + thread per plugin
   ┌────────────┴────────────┬─────────────┬──────────────┐
   ▼                         ▼             ▼              ▼
┌────────────┐  ┌───────────────┐  ┌────────────┐  ┌────────────┐
│ sessions   │  │ tasks         │  │ files      │  │ review     │
│ (bundled)  │  │ (bundled)     │  │ (bundled)  │  │ (bundled)  │
└────────────┘  └───────────────┘  └────────────┘  └────────────┘
   Luau, hot-reloadable, capability-scoped, agent-callable
```

Everything below the line is a plugin — including the session list. The kernel
never renders a feature; it renders **view trees** that plugins return, and owns
only the things a plugin cannot: session supervision, the terminal grid,
persistence, and the frame.

## The five decisions that carry the rest

If you read nothing else:

| Decision | Consequence if it is wrong |
|---|---|
| [V1](ARCHITECTURE.md#adr-v1) — everything but six things is a plugin, **including the session list** | The API stays second-class, because nothing important dogfoods it |
| [V2](ARCHITECTURE.md#adr-v2) — Luau in-process, one VM and thread per plugin, no native code | Either a plugin fault takes the TUI down, or the capability model goes back to being advisory |
| [V5](ARCHITECTURE.md#adr-v5) + [V14](ARCHITECTURE.md#adr-v14) — declarative view tree, frozen primitives, widgets in userland | Either plugins cannot draw what they need, or the kernel becomes the bottleneck for every widget |
| [V16](ARCHITECTURE.md#adr-v16) — plugins have a headless service half | Automations stop firing when the TUI closes, silently revoking a v1 guarantee |
| [V20](ARCHITECTURE.md#adr-v20) — trunk-based delivery behind a compile-time gate | v1 velocity collapses under merge debt, and abandoning v2 orphans months of work |

Two more are worth knowing because they resolve limitations rather than create
them: [V18](ARCHITECTURE.md#adr-v18) (motion is declared, not pushed) and
[V19](ARCHITECTURE.md#adr-v19) (real-time content is a vt100 surface, not a
faster tree).

## Reading paths

**Deciding whether this is a good idea** — start here, ~30 minutes:

1. [VISION.md](VISION.md) — why v2 exists, what it costs, how it arrives
2. [ARCHITECTURE.md](ARCHITECTURE.md) — the index, then the five ADRs above
3. [LIMITATIONS.md](LIMITATIONS.md) — what the design cannot do, and §8's
   tripwires for reopening it

**Planning or doing the work**:

1. [RELEASE-STRATEGY.md](RELEASE-STRATEGY.md) — delivery model, the three
   stages, the nightly channel, the website
2. [MIGRATION.md](MIGRATION.md) — phases, decision gates, teardown inventory,
   rollback
3. [SECURITY.md](SECURITY.md) — threat model, what capabilities do and do not
   buy, and what must land before third-party plugins are installable
4. [DECISION-plugin-runtime.md](DECISION-plugin-runtime.md) — why in-process
   Luau, what it costs, and the conditions it rests on
5. [CONSTITUTION-DELTA.md](CONSTITUTION-DELTA.md) — which v1 rules and ADRs v2
   amends or retires

**Writing a plugin** — these are the contracts, and what a plugin may rely on:

1. [FEATURES-Plugin-API.md](FEATURES-Plugin-API.md) — manifest, lifecycle,
   capabilities, host API
2. [FEATURES-View-Tree.md](FEATURES-View-Tree.md) — the view half: node
   catalog, styling, input
3. [FEATURES-Layout.md](FEATURES-Layout.md) — the workspace tree, anchored
   overlays, measurement
4. [FEATURES-Animation.md](FEATURES-Animation.md) — motion: timing, identity,
   leases, accessibility
5. [FEATURES-Keybindings.md](FEATURES-Keybindings.md) — contexts, conflicts,
   terminal passthrough, F1, and pane show/hide
6. [FEATURES-Backend-API.md](FEATURES-Backend-API.md) — the service half: CLI
   verbs, services, schedules, storage, bus
7. [FEATURES-Agent-API.md](FEATURES-Agent-API.md) — how agents discover and
   drive plugin commands

## How v2 reaches users

Not as a big bang. Everything lands on `main` behind a Cargo feature that is
absent from stable builds, and the gate moves in three steps
([ADR-V20](ARCHITECTURE.md#adr-v20)):

| Stage | What a user sees | Version |
|---|---|---|
| **A** | Nothing. Stable does not contain the plugin host; nightly does | v1.x, unchanged |
| **B** | Plugins work, opt-in, alongside every native pane | an ordinary v1 **minor** |
| **C** | Plugins *are* the panes; `extensions/` gone | **2.0.0** |

There is no long-lived `v2` branch. v1 keeps releasing on its existing cadence
throughout, from the same trigger, with the same process.

## Status

| Area | State |
|---|---|
| Design | In review — this directory |
| Phase 0 — foundations | Not started |
| Phase 1 — plugin host | Not started |
| Phases 2–3 — service half, motion | Not started |
| Phase 4 — bundled plugins | Not started |
| Phase 5 — command registry | Not started |
| Phase 6 — teardown, 2.0.0 | Not started |

Two questions can still change the architecture, and each has a deadline
earlier than the work that depends on it:

| Question | Due | If the answer is no |
|---|---|---|
| Does Luau hold up for a real pane? ([DECISION-plugin-runtime.md](DECISION-plugin-runtime.md)) | End of Phase 1 | Reversal to an out-of-process runtime, at the cost of the enforced capability model and a bundled runtime returning |
| Does the session list meet its frame budget as a plugin? ([MIGRATION §3](MIGRATION.md#3-the-session-list-decision-gate)) | End of Phase 1 | A kernel `sessionList` surface — a recorded retreat from [ADR-V1](ARCHITECTURE.md#adr-v1), taken before six panes assume otherwise |

Both are settled by building, not by arguing: Phase 1's validation gate writes
one hard pane — the code review or the session list — in Luau, and has an agent
write it too. A language that makes those awkward makes every third-party pane
awkward.

## Conventions

Same rule as `docs/`: if an implementation change invalidates or extends a
decision here, update the document in the same PR. v2 ADRs are numbered
`ADR-V*` so they never collide with v1's `ADR-*` / `ADR-P*` series.
Cross-references between these documents are checked by
`scripts/dev/check-doc-links.sh`, wired into `just lint` and the pre-commit
hooks — markdown links are invisible to rumdl and to the compiler, so without it
they rot silently.
