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

## Forward-looking design

[`v2/`](v2/README.md) holds the design set for **Thurbox v2** — the refactor
that turns the TUI into a minimal Rust kernel hosting reloadable TypeScript
plugins. Nothing there is implemented; the documents above remain
authoritative for the shipping product until v2 lands.

## Keeping Docs Current

**Rule**: If a code change invalidates or extends a documented decision,
update the relevant doc in the same PR.

- Operational changes (new commands, module moves) go in `CLAUDE.md`
- Decisional changes (why we chose X over Y) go in `docs/`
- Don't duplicate content between the two
