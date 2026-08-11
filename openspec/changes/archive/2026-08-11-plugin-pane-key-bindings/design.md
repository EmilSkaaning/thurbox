# Design

See `proposal.md` — Why. This records the shape chosen, and what was rejected.

## 1. The context is the binding's address, not a new enum member

`session::KeyContext` is a closed `Copy` enum, and `session::Action::context()`
maps every kernel action onto it. The obvious reading of "give a pane its own key
context" is `KeyContext::Pane(String)`.

**Rejected**, for two reasons that are facts about the existing code rather than
taste:

- `KeyContext` is `Copy` and matched by value in `contexts_overlap`,
  `Action::context`, `App::focus_key_context` and the deterministic tie-break
  inside `lookup_in`. A `String` member makes it non-`Copy`, which churns every
  one of those for a variant no `Action` can ever carry — kernel actions are a
  closed set and none of them belongs to a plugin pane.
- The scope would then be stated **twice**: once by the variant and once by the
  binding's `(plugin, pane)` address, and two statements of one fact drift.

So the entry itself carries the scope. `PaneBindingId { plugin, pane, id }` is the
address, and "active while that pane is focused" is a property of the lookup that
takes the focused pane, not of a value stored beside the chord. `pane:<id>` from
`FEATURES-Keybindings.md` §2 survives as the *displayed* scope (the editor's
section title, the persisted key's prefix), which is where a user meets it.

`contexts_overlap` is untouched, as `plugin-command-registry`'s design predicted.
What is added is one more overlap rule, in one function: a pane binding overlaps a
**global** action and the bindings of its **own** pane, and nothing else.

## 2. Two tables in one `KeyBindings`, not two keymaps

`KeyBindings` gains `panes: BTreeMap<PaneBindingId, PaneBinding>` beside its
`HashMap<Action, Vec<KeyChord>>`.

Rejected: **a separate `PluginKeymap` owned by the host.** It reads cleaner and is
wrong in three places — the F1 editor would have to merge two sources to index one
selection, `keybindings.json` would need a second writer, and conflict detection
between a plugin chord and a kernel chord would live outside the type that knows
about conflicts. One table also makes the *ungated* half fall out for free (below).

`BTreeMap` rather than `HashMap`: the editor's sections and the persisted file are
ordered output, and an ordered map removes the sort that would otherwise be
written twice.

## 3. The keymap half is ungated; only the delivery half is behind `plugins`

`session::keybindings` is pure data, and `keybindings.json` is a user file. A
build without the plugin host must **parse and re-serialize** a file containing
`plugin:` entries without discarding them, or a user who installs a stable
release, edits nothing, and comes back to a plugin build has silently lost their
plugin bindings — because the F1 editor rewrites the whole file on any edit.

So the types, the resolution, the collision rule and the persistence are compiled
always; what is gated is everything that *produces* declarations
(`plugin/keymap.rs`), delivers a binding (`plugin/runtime.rs`, the worker) or
routes a key to a pane (`app`'s `InputFocus::PluginPane` arm, already gated).

A stable build therefore holds overrides for bindings nothing declares. That is
the same shape as `from_json`'s existing treatment of a missing action — fall back,
do not fail — extended in one direction: an *unknown-plugin* entry is **kept**
rather than dropped, since dropping it is what loses the user's file.

## 4. What a binding delivers: the id, beside the raw key

`onKey(paneId, key)` becomes `onKey(paneId, key, binding)`, with `binding` nil
when the chord resolved to none.

Rejected: **a separate `onBinding(paneId, id)` handler.** Two handlers means two
answers about consumption for one keypress, and a plugin that wants both raw keys
(a text field) and bindings (its actions) would have to reconcile them. One
handler with one answer keeps the "unconsumed keys fall through" contract exactly
as specified.

Rejected: **replacing the key with the binding id.** A pane that collects text
needs the keypress, and a pane whose plugin declares no binding at all would then
receive nothing. Both fields, always.

Rejected: **routing a pane binding through the command registry** (ADR-V21's
`{ command, key, context }`). A registered command is dispatched to the plugin's
**service** half, in a different VM with no pane state, over a channel with no
"was it consumed" answer — so `j` in a list would move a cursor that lives in the
*other* VM. The command surface remains the right answer for a *name-addressed*
invocation (`thurbox-cli command run`), and a later change may let a pane binding
name a command as well; it is not the answer for a pane's keyboard.

## 5. Registration is idempotent and does not write the file

`KeyBindings::register_pane_bindings(decls)` **replaces** the registered set: a
reload can add, remove or re-chord bindings, and a pane that vanished must stop
resolving. Two rules make it safe to run on every reload:

- a stored override for an id survives, because overrides are held in their own
  map and consulted when an entry is (re-)built;
- registration never persists anything. Only a user edit writes
  `keybindings.json`, so re-registering identical declarations at startup cannot
  rewrite the user's file — and a plugin cannot make the kernel write to it.

Declarations reach `App` on the channel that already carries panes
(`PluginUiEvent::Panes`), because they change at exactly the same moments panes do
(start, reload, stop) and a second event would be a second ordering to reason
about.

## 6. Where the collision drop is reported

A dropped default is not a status toast: it happens while plugins start, before
anyone is watching, and it is a property of a *configuration*, not an event.

It is logged, and re-derived on demand by `thurbox-cli plugin doctor`, which
already re-derives the spawn contributions from the manifests without starting a
VM. `doctor` loads the user's keybindings file, registers the discovered
declarations against it, and prints each binding's chord or the reason it has
none. That keeps the report answerable when the TUI is not running, which is where
a user debugging a dead key actually is.

## 7. Module ownership, against the architecture allowlist

| New/changed type | Module | Allowed by `tests/architecture_rules.rs` |
|---|---|---|
| `PaneBindingId`, `PaneBinding`, `PaneBindingDecl`, `BindingTarget` | `session::keybindings` | `session` references nothing; these are pure data |
| widened `KeybindingDecl` + its validation | `session::plugin_manifest` | intra-`session` use of `keybindings::KeyChord`, as `plugin_command` already uses `plugin_manifest` |
| manifests → declarations | `plugin::keymap` (new) | `plugin` → `session` |
| publishing declarations with the panes | `plugin::lifecycle` | unchanged |
| `binding` on the key request | `app` + `plugin::runtime` | unchanged |
| the editor's extra sections | `app::view` | `ui` is **not** involved: `render_help_overlay` and `build_rebindable_rows` already live in `app::view`, so no `ui → plugin` edge is created |

No allowlist entry changes, and `ui` gains no reference to `crate::plugin`.

## 8. Why the chord grammar is validated in the manifest

`KeybindingDecl::chord` is documented today as "left unvalidated here: the
manifest layer is pure data and does not own the chord grammar". That was true
when the grammar's only consumer was the keymap; it is not a layering fact —
`session::keybindings` is the *same* layer, and `session::plugin_command` already
depends on `session::plugin_manifest` across files.

Validating there turns `chord = "ctrl+shift"` into a discovery error naming the
plugin, the binding and the chord, which is where an author can act on it. The
alternative — a warning at registration — reports a typo to a log a plugin author
never reads, and leaves a binding that looks declared and does nothing.

The comment is corrected in the same change rather than left contradicting the
code.

## 9. What this deliberately leaves for the port

- **A binding does nothing on its own.** It tells a plugin its key fired; what a
  plugin may *change* is the capability question (`plugin-pane-mutations`).
- **No binding may name a chord the kernel needs.** `Esc` remains kernel-owned
  and is never offered to a plugin, so a binding declared on it can never fire.
  Left as-is rather than made a manifest error: `Esc` is the one chord the pane
  handler intercepts by identity, and enumerating "chords the kernel keeps" in the
  manifest layer would duplicate a routing rule that lives in `app`.
- **No mouse.** A click is the third change.
