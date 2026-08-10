## Why

The plugin host exists but nothing starts it. `plugin-host-foundation` built
discovery, the runtime, the lifecycle, and the capability model, and left them
reachable only from tests: no binary entry point references `plugin::`, so on a
real machine a plugin dropped into `~/.config/thurbox/plugins/` is never found
and never runs.

That also left two things unanswerable. The foundation's specs require a
plugin's state and failure cause to be *inspectable* — but with no surface,
"why isn't my plugin running?" has no answer a user can reach. And the runtime
spec's startup-cost bound could not be measured, because both builds had
identical startup paths.

This change starts the host in both binaries and adds the read-only surface
that reports what it found.

## What Changes

- **The TUI starts the plugin host during boot** and stops it during shutdown,
  alongside the existing session restore. Startup remains non-blocking: a
  plugin that hangs in `init` must not delay the first frame.
- **`thurbox-cli` starts the host too**, so a headless invocation sees the same
  plugin set as the TUI — the two must never disagree about what is installed.
- **New `thurbox-cli plugin` subcommand**, read-only in this change:
  - `plugin list` — every discovered plugin with its state, source, and
    granted capabilities.
  - `plugin status [<name>]` — one plugin in detail, or all; includes the
    failure cause and failing transition for anything that did not start.
  - `plugin doctor` — everything discovery rejected and why: invalid
    manifests, overridden plugins, same-source name conflicts, unreadable
    directories.
  - Output follows the existing convention: human-readable by default, JSON
    when piped, forced with `--json` / `--pretty` / `--text`.
- **Plugin failures are logged** at startup with the plugin name, the
  transition, and the cause, so a failure is discoverable without running a
  command.
- **The startup cost bound becomes measurable** and is asserted: with the
  feature compiled in and no plugins installed, boot does discovery and stops.
- **Still no UI surface.** Nothing a plugin returns is rendered, no pane slot
  exists, and the TUI's panels are unchanged. `plugin install`, `enable` and
  `disable` are not part of this change — the surface here only reports.

## Capabilities

### New Capabilities

- `plugin-host/cli`: the `thurbox-cli plugin` verbs — what each reports, the
  output contract, and what they do when the feature is compiled out.

### Modified Capabilities

- `plugin-host/lifecycle`: adds requirements for *when* the host starts and
  stops in each binary, and for startup being non-blocking. The existing state
  machine and per-plugin failure isolation requirements are unchanged.
- `plugin-host/runtime`: the existing "runtime cost is zero when no plugins are
  present" requirement is tightened from a structural claim into one measured
  against a booting binary, now that the host is actually on the startup path.

## Non-goals

- **No rendering.** No pane, no view tree, no plugin-contributed UI. A
  manifest's `[[panes]]` is still only data the host can enumerate.
- **No mutation verbs.** `install`, `uninstall`, `enable`, `disable`, and
  `reload` are all later changes. Everything here reads.
- **No plugin config.** Per-plugin settings and a way to deny a capability an
  installed plugin requested are not introduced.
- **No new capabilities in the vocabulary.** The host API surface a plugin sees
  is exactly what the foundation shipped.
- **No bundled plugins.** The bundled source stays empty; this change makes the
  user directory work end to end.

## Impact

**Code.** `src/main.rs` (TUI boot and shutdown), `src/bin/thurbox-cli.rs` plus
a new `src/cli/plugins.rs` for the subcommand, and `src/plugin/` gains the
reporting shape the CLI renders. The `cli` module's architecture allowlist entry
must be extended to reach `plugin`; per the existing convention for
headless→backend dependencies, that is a path-only reference so each call site
stays visible.

**Feature gating.** Every new surface is behind `#[cfg(feature = "plugins")]`.
The `plugin` subcommand is absent from a stable build's `--help` rather than
present and erroring, matching how the feature is absent rather than disabled.

**Startup path.** This is the first change where a plugin can affect boot. The
ordering relative to session restore, and what happens when a plugin is slow,
are the design's central questions.

**Docs.** `CLAUDE.md`'s thurbox-cli subcommand list and `docs/CONFIG.md`'s
config-location table gain the plugin directory and the new verbs.
