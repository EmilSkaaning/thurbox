# Thurbox v2 — Vision

## The problem v1 has

Thurbox v1 works, and its architecture is coherent: one `App` model, one
`view()`, strict module isolation, deterministic tests. But every user-visible
surface is compiled in, and the cost of that shows up as a **cross-cutting tax
on every new pane**.

Adding one pane to v1 means touching, at minimum:

| Site | v1 size today |
|---|---|
| `App` struct fields | 80 fields |
| `InputFocus` | 11 variants |
| `Action` | 61 variants |
| `KeyContext` | 6 variants |
| `FeatureFlags` | 13 flags |
| `PanelAreas` + `compute_layout` | 10 fixed rects, 9-argument signature |
| `ClickAction` | 17 variants |
| `App::view` + a `ui/` module | 36 modules, 2,273 lines of view dispatch |
| `SettingsField`, `focus_ring`, `enforce_feature_visibility` | 3 more parallel tables |
| Acceptance snapshots | 7 pinned screens |

`src/app/mod.rs` is 14,605 lines. The tasks pane, the file viewer, the code
review, and the automations pane are all *features of the binary* rather than
things that plug into it. Three consequences follow:

1. **Nobody but us can add a pane.** A user who wants a Jira column, a CI
   dashboard, or a log tailer must fork a 95k-line Rust codebase, learn the
   TEA conventions, and ship a build.
2. **Every experiment is permanent.** A pane cannot be tried out, iterated on
   in an afternoon, and thrown away — it must survive review, clippy, the
   architecture rules test, and a release.
3. **The extension system solved the wrong half.** v1's `extensions/` reach
   agents (sessions, automations, prompts, shell scripts) but never the UI.
   Roughly 5,000 lines of manifest/install/self-heal machinery buy zero ability
   to draw a pane.

Meanwhile the parts of thurbox that are genuinely hard — persistent tmux
sessions across local/SSH/WSL transports, worktree orchestration, the
demand-driven render loop, multi-instance SQLite sync — are the parts almost
nobody needs to modify.

## The v2 thesis

**Split the hard kernel from the soft surface.**

The Rust kernel keeps what is genuinely hard and genuinely shared: session
lifecycle, backends, git, storage, the event loop, the layout solver, the
theme, the keymap, and the frame. Everything a user *sees and argues about* —
which panes exist, what they show, how they behave — moves into Luau
plugins that load at runtime, reload without recompiling, and can be written
by anyone in an afternoon.

Prior art: neovim, wezterm and yazi all embed Lua for exactly this — a small
sandboxed VM, discovered from a directory, reloadable without rebuilding the
host. [pi](https://github.com/earendil-works/pi) does the same shape with
TypeScript around an agent loop. Thurbox v2 takes the same shape and
points it at the thing thurbox is actually good at: **many agent sessions
across many repos and hosts**.

## What changes for users

**Nothing, on first launch.** The bundled plugin set ships inside the binary
and is enabled by default, so a fresh `thurbox` looks and behaves like v1:
session list, terminal, info panel, files, tasks, automations, review.

What becomes possible:

```bash
thurbox plugin list                  # what's loaded, and what each provides
thurbox plugin install <name|url>    # from the registry, a git URL, or a path
thurbox plugin disable tasks         # replaces [features] tasks = false
thurbox plugin dev ./my-plugin       # load from disk, reload on save
```

A plugin is a directory with a manifest and a Luau entry point. The
manifest is what the pane, the command, and the keybinding *are* — the kernel
reads it without starting anything, so the UI is complete before a line of
plugin code runs ([ADR-V15](ARCHITECTURE.md#adr-v15)):

```toml
# plugin.toml
name = "ci-status"

[[panes]]
id = "ci"
slot = "right"
title = "CI"

[[commands]]
id = "ci.refresh"
title = "Refresh CI status"
```

```lua
-- src/view.luau — behavior only
local thurbox = require("@thurbox")
local ui = thurbox.ui

return thurbox.definePlugin({
    init = function(ctx) return { runs = fetchRuns(ctx) } end,
    render = function(ctx, s)
        return ui.list(s.runs, function(r)
            return ui.row(ui.text(r.name), ui.statusDot(r.state))
        end)
    end,
    commands = {
        ["ci.refresh"] = function(ctx) ctx.setState({ runs = fetchRuns(ctx) }) end,
    },
})
```

Because plugins declare **commands with typed schemas**, and because thurbox
already puts an agent inside every session, those commands are callable by the
agents themselves — a session's agent can create tasks, open a review, or
drive a pane through the same surface a keybinding uses. Thurbox stops being a
TUI that agents happen to run inside and becomes an orchestrator agents can
operate. See [FEATURES-Agent-API.md](FEATURES-Agent-API.md).

## What v2 is not

- **Not a rewrite of the session layer.** tmux backends, the control-mode
  protocol, SSH/WSL transports, worktrees, and the SQLite schema carry over
  largely intact. They are the kernel.
- **Not a plugin marketplace.** v2 ships a plugin *format*, a local install
  path, and a small curated registry. Discovery, ratings, and sandboxed
  third-party hosting are later problems.
- **Not a general TUI framework.** The view tree is scoped to what thurbox
  panes need. Plugins that want arbitrary pixels are out of scope by design —
  see [FEATURES-View-Tree.md](FEATURES-View-Tree.md).
- **Not a config-file replacement.** `settings.toml`, `agents.toml`,
  `hosts.toml`, `themes.toml`, and `keybindings.json` survive. Plugins add
  their own namespaced config; they do not restructure the existing files.
- **Not multi-language.** The plugin runtime is Luau only. WASM guests and
  native modules are explicitly rejected in
  [ADR-V2](ARCHITECTURE.md#adr-v2).

## How it arrives

Not as a big bang. v2 lands on `main` behind a compile-time gate and the gate
moves in three steps, so the capability reaches users well before the breakage
does ([ADR-V20](ARCHITECTURE.md#adr-v20)):

| Stage | What a user sees | Version |
|---|---|---|
| A | Nothing. Stable builds do not contain the plugin host | v1.x, unchanged |
| B | Plugins work, opt-in, alongside every native pane | an ordinary v1 **minor** |
| C | Plugins are the panes; `extensions/` gone | **2.0.0** |

Stage B is the point of the exercise arriving early: a plugin author gets a
stable binary to build against, and the protocol gets real third-party use while
its mistakes are still free to fix. Nothing breaks until 2.0.0.

## What v2 costs

Stated plainly, because these are real and permanent:

- **Two languages, two toolchains.** Rust *and* Luau in CI, in the pre-commit
  hooks, and in every contributor's head.
- **No package ecosystem.** Luau has no npm. Every capability a plugin needs
  comes from the host or gets written by hand
  ([ADR-V2](ARCHITECTURE.md#adr-v2)).
- **A public API you cannot casually break.** Once third-party plugins exist,
  the view tree, the host bindings, and the command schemas are versioned
  surfaces.
- **A plugin fault is closer than it was.** Plugins run in the thurbox process
  ([ADR-V2](ARCHITECTURE.md#adr-v2)). Errors, hangs and memory are contained
  per VM; a segfault in native code would not be, which is why plugins may not
  carry any.
- **A larger trust surface.** v1 extensions already run arbitrary shell, but
  they run it *on demand*. Plugins run continuously, which is why capabilities
  are declared and enforced ([ADR-V4](ARCHITECTURE.md#adr-v4)).

The trade is deliberate: thurbox gives up a package ecosystem and absolute
fault isolation in exchange for being extensible by its users, at no cost to
the single-binary install.

## Success criteria

Each is written to be falsifiable, and the measurable ones carry their bar.

**Stage B is done when:**

1. A new pane can be written, loaded, and iterated on without recompiling the
   Rust binary or restarting the TUI.
2. [FEATURES-Plugin-API.md](FEATURES-Plugin-API.md) is sufficient to build a
   working plugin without reading kernel source — tested by someone outside the
   project doing it.
3. At least one plugin exists that thurbox did not write, and one full minor
   release has passed with no non-additive protocol change.

**2.0.0 is done when, additionally:**

1. The session list, terminal, info panel, files, tasks, automations, and code
   review are plugins, and disabling any one of them leaves a working thurbox.
2. With the default bundled set active, `first_frame_ms`
   (`THURBOX_PERF_LOG=1`) is within **115%** of v1's, and the idle paint rate is
   unchanged at v1's ~4 fps floor — plugins must not defeat the demand-driven
   loop. Measured with plugins *active*, since lazy activation makes the
   no-plugins number meaningless
   ([MIGRATION §3](MIGRATION.md#3-the-session-list-decision-gate)).
3. An agent inside a session can list and invoke every bundled plugin command
   through `thurbox-cli`.
4. The v1 `extensions/` tree is gone and its ~5,000 lines of Rust are deleted
   rather than deprecated, with the 554 lines of hooks behavior absorbed into
   the kernel ([MIGRATION §4](MIGRATION.md#4-teardown-inventory--the-v1-extension-system)).
