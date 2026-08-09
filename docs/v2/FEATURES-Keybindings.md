# Thurbox v2 — Keybindings, Help, and Pane Visibility

How a plugin's keys reach the user, how they coexist with 61 kernel actions and
with whatever the agent in a focused terminal expects, and how panes are shown
and hidden.

v1's keybinding system is one of its better-designed parts — contexts, live
rebinding from F1, conflict detection scoped by overlap, and readline
passthrough. **None of it is replaced.** This document specifies how plugins
join it.

---

## 1. The registry

Every binding — kernel or plugin — resolves through one registry, persisted in
one file (`~/.config/thurbox/keybindings.json`), edited in one place (the F1
panel). A plugin declares defaults in its manifest:

```toml
[[commands]]
id = "ci.refresh"
title = "Refresh CI status"

[[keybindings]]
command = "ci.refresh"
key = "r"
context = "pane:ci"
```

Bindings register **from the manifest, without creating the plugin's VM**
([ADR-V15](ARCHITECTURE.md#adr-v15)). A plugin that never activates still
resolves its chords, still appears in F1, and still lists in the palette; it
activates when a chord is actually pressed.

### Precedence

| Rank | Source | Notes |
|---|---|---|
| 1 | The user's `keybindings.json` | Always wins. Editing F1 writes here |
| 2 | Kernel action defaults | The 61 v1 actions |
| 3 | Plugin manifest defaults | Between plugins, install order |

---

## 2. Contexts

v1 has six contexts (`Global`, `SessionList`, `Automations`, `Tasks`,
`FileViewer`, `Terminal`) and one overlap rule:

```rust
a == KeyContext::Global || b == KeyContext::Global || a == b
```

v2 keeps the rule and makes the set **open**: every declared pane gets
`pane:<id>` as a context. `SessionList`, `Tasks` and friends stop being
hardcoded variants and become the pane contexts of the plugins that own them —
which is the whole point of ADR-V1, and it costs no change to the overlap
predicate. Two different plugin panes never overlap, so both may bind `j`.

| Context | Active when |
|---|---|
| `global` | Always |
| `pane:<id>` | That pane has focus |
| `terminal` | A `sessionTerminal`, `pty`, or `surface` node has focus |

---

## 3. Conflicts

Two situations that look alike and must behave differently.

**A user rebinds in F1.** Stealing is correct: the user pressed the chord
deliberately. v1's behavior stands — the chord is reassigned, the previous
owner is unbound, and a toast names the move.

**A plugin's manifest default collides on install.** Stealing is wrong. A newly
installed plugin must not silently break muscle memory, and the user has not
expressed any intent about that chord. So:

> A plugin default that collides with an already-bound chord in an overlapping
> context is **dropped, not applied.** The command stays fully reachable from
> the palette, `thurbox-cli`, and the agent API; only the shortcut is withheld.

`thurbox plugin install` reports dropped bindings, and `thurbox plugin doctor`
lists them with the chord's current owner so the user can rebind deliberately
in F1 if they want it.

This asymmetry is the design: **a chord changes owner only when a human asks
for it.**

---

## 4. Terminal passthrough

v1 defers eleven actions to the PTY when a terminal is focused, because they
sit on readline's namespace — `Ctrl+B` (backward char), `Ctrl+E` (end of line),
`Ctrl+W` (delete word), `Ctrl+R` (reverse search), `Ctrl+X` (emacs prefix), and
so on. Each keeps an F-key alternate that works everywhere.

For plugins the problem mostly disappears by construction: a `pane:<id>`
binding fires only while that pane has focus, and a focused terminal is a
different pane. **Pane-scoped plugin bindings can never reach a terminal.**

The exception is a plugin binding declared `context = "global"`:

> A global plugin binding on a bare `Ctrl+<letter>` chord is **automatically
> passthrough-deferred** while a terminal, `pty`, or `surface` node has focus.
> Plugins cannot opt out.

The readline namespace belongs to the program running in the pane, and no
plugin author is in a position to know which of the user's agent CLIs needs
`Ctrl+W`. The kernel applies the same `is_ctrl_letter_chord` test it uses for
its own actions, so the rule is one predicate shared by both.

**Guidance for plugin authors**: prefer a `pane:<id>` context. If you genuinely
need a global chord, use an F-key or a modified chord (`Ctrl+Shift+…`,
`Alt+…`) rather than a bare `Ctrl+<letter>` — those dispatch everywhere,
including from a focused terminal.

---

## 5. The F1 panel stays kernel

The theme picker, the settings panel, and the repo picker are all ordinary
`overlay`-slot panes in v2. **The F1 keybinding editor is not**, and the reason
is mechanical rather than a matter of taste:

> The editor's core operation is capturing the **next physical keypress** —
> including chords the kernel would otherwise intercept, like `Ctrl+Q`. A
> plugin cannot receive a keypress the kernel routes elsewhere, so a plugin
> implementation of the editor structurally could not rebind the chords most
> worth rebinding.

So F1 remains kernel chrome, alongside the frame, the footer, and the layout
solver. It renders the merged registry, which means plugin actions appear in it
automatically, grouped under the plugin's name, with no plugin code running at
all.

### What plugins contribute to F1

| Element | Source |
|---|---|
| Section heading | Plugin `title` |
| Rebindable rows | Each `[[keybindings]]` entry, with its live chord |
| Non-rebindable rows | `documented_keys` (§6) |
| Availability marker | Whether the plugin is enabled, suspended, or faulted |

---

## 6. In-pane keys and discoverability

A plugin handling raw `key` events (§`plugin/event`) takes keys that never
reach the registry — so they cannot be rebound, and F1 cannot list them. This
is the same category as v1's "fixed" keys (modal selectors, the file-viewer
search sub-mode), which the F1 panel documents but cannot edit.

**Prefer commands.** A pane whose `j`/`k` are declared as commands with
`pane:<id>` bindings gets rebinding, F1 documentation, palette entries, and
agent-callability for free. Raw key events are for what genuinely needs them:
text-entry-like interaction, high-frequency navigation, and sub-modes.

For those, declare them as documentation:

```toml
[[panes]]
id = "ci"
documented_keys = [["/", "filter"], ["esc", "clear filter"]]
```

`documented_keys` is inert — it renders in F1 under the plugin's section, marked
non-rebindable, and does nothing else. It exists so a plugin's raw-key
sub-modes are discoverable rather than folklore.

---

## 7. Pane visibility

v1 has two independent axes that v2 must keep separate, because collapsing them
loses a behavior users rely on:

| Axis | v1 | v2 |
|---|---|---|
| Does this surface exist at all? | `[features] tasks = false` | `thurbox plugin disable tasks` |
| Is it on screen right now? | `F5` / `F9` / `F2` / `F3` | This section |

### Kernel-owned, not plugin-owned

**Pane visibility is kernel state.** The manifest declares the initial value;
the kernel owns it thereafter.

```toml
[[panes]]
id = "ci"
slot = "right"
default_visible = true
min_width = 24
toggle_key = "f6"        # optional default chord, subject to §3
```

Plugin-owned visibility would be circular: a suspended plugin cannot show its
own pane, and `onPaneVisible:<id>` is precisely the event meant to wake it.
`ctx.ui.showPane(id, visible)` therefore becomes a **request** the kernel
honors, not the source of truth.

### Auto-generated commands

Every declared pane gets three commands, registered from the manifest with no
plugin code:

```text
<plugin>.<pane>.toggle
<plugin>.<pane>.show
<plugin>.<pane>.hide
```

They appear in the palette, in `thurbox-cli command run`, in the agent API, and
in F1 as rebindable rows. This is what preserves v1's uniform "one key shows or
hides a panel" model across third-party panes, instead of every plugin
inventing its own toggle.

### Showing wakes a suspended plugin

`show` on a suspended plugin's pane activates it and fires
`onPaneVisible:<id>`. The frame paints the pane's chrome immediately with a
loading state; the tree arrives when the plugin pushes it. Toggling a pane is
therefore never blocked on a cold start.

### Three states, kept distinct

| State | Loaded? | Contributes keys/commands? | On screen? |
|---|---|---|---|
| **Disabled** (`plugin disable`) | No | No | No |
| **Hidden** | Manifest registered; process may be suspended | Yes | No |
| **Visible** | Yes | Yes | Yes, subject to `min_width` |

A disabled plugin contributes nothing at all — its chords are free for other
plugins to claim, which is the v2 equivalent of `[features] tasks = false`
freeing up the tasks pane's keys.

### Responsive hiding is not user intent

A pane dropped by the layout solver for being under its `min_width` is
**not** marked hidden. Widening the terminal restores it. This mirrors v1,
where the info panel and tasks panel appear at width ≥ 120 without the user's
toggle state changing underneath them.

### Persistence — a deliberate change from v1

v1 does **not** persist panel visibility: `show_info_panel`, `show_file_viewer`,
`show_tasks_panel` and `show_session_list` are plain `bool` fields on `App`,
reset on every launch.

v2 **persists** per-pane visibility in the kernel database, keyed by pane id.
With four built-in panels, re-toggling after each launch is a minor annoyance;
with an open-ended set of third-party panes it is not, and a user who hides a
plugin's pane means it. `thurbox plugin doctor` reports panes hidden by user
state, so "my pane never appears" has a discoverable answer.

---

## 8. Worked example

A CI plugin contributing one pane and two shortcuts:

```toml
[[panes]]
id = "ci"
title = "CI"
slot = "right"
default_visible = false
toggle_key = "f6"
documented_keys = [["/", "filter"], ["esc", "clear filter"]]

[[commands]]
id = "ci.refresh"
title = "Refresh CI status"

[[keybindings]]
command = "ci.refresh"
key = "r"
context = "pane:ci"
```

What the user gets, with no plugin code beyond `render`:

- `F6` toggles the pane, rebindable in F1, and it wakes the plugin if suspended.
- `r` refreshes while the pane is focused, rebindable, and never fires while a
  terminal has focus.
- An F1 section titled **CI** listing `F6`, `r`, and the two documented keys.
- `thurbox-cli command run ci.refresh` and `ci.ci.toggle` for scripts and
  agents.
- If `F6` was already taken, the binding is dropped with a warning and every
  command above still works.
