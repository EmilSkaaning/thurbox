# Design Documentation

This directory contains the **rationale** behind Thurbox's design
decisions. For operational guidance (build commands, module layout,
event loop), see [`CLAUDE.md`](../CLAUDE.md).

## Documents

| Document | Purpose | Update when... |
|---|---|---|
| [CONSTITUTION.md](CONSTITUTION.md) | Core principles | Adding/removing an enforced invariant |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Architecture decisions | Changing a technology or structural pattern |
| [FEATURES.md](FEATURES.md) | Feature-level design | Altering keybindings, lifecycle, layout, or UX |
| [ORCHESTRATION.md](ORCHESTRATION.md) | The control-plane pattern for running sessions across many repos | Changing the session/message/extension surface the pattern relies on |
| [CONFIG.md](CONFIG.md) | Every config file / env var / DB setting in one place | Adding/changing a config file, env var, or DB setting |
| [SPIKE-SESSION-LIST.md](SPIKE-SESSION-LIST.md) | Whether the session list can be a v2 plugin, and the measurements behind the answer | The view tree, the plugin render trigger, or pane selection ownership changes |
| [PHASE4-PANE-READINESS.md](PHASE4-PANE-READINESS.md) | Audit of what a bundled plugin pane still cannot express, traced to the code | Closing one of its gaps, or finding a new one while porting a pane |
| [PHASE6-TEARDOWN-READINESS.md](PHASE6-TEARDOWN-READINESS.md) | What the v2 teardown may not delete yet, and the worklist that unblocks it | Landing a replacement the teardown waits on (`tests/teardown_gate.rs` will say so) |

## Keeping Docs Current

**Rule**: If a code change invalidates or extends a documented decision,
update the relevant doc in the same PR.

- Operational changes (new commands, module moves) go in `CLAUDE.md`
- Decisional changes (why we chose X over Y) go in `docs/`
- Don't duplicate content between the two
