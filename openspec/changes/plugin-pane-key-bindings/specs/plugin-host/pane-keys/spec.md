# plugin-host/pane-keys Specification

## ADDED Requirements

### Requirement: A pane binding is a keymap entry scoped to its pane

The keymap SHALL hold, alongside its closed set of kernel actions, an entry for
every keybinding a discovered plugin declares. An entry MUST be addressed by the
triple `(plugin, pane, binding id)` — the same way every other plugin surface is
addressed — and MUST be active **only** while that pane is focused.

Scoping by the entry's own address rather than by a new member of the kernel's
context enumeration is required: the set of panes depends on which plugins are
installed, so a closed enumeration cannot name them, and the F1 editor's row
order must not depend on the installed set.

Two consequences MUST hold, because they are what a per-pane scope buys:

- two panes MAY bind the same chord to different bindings, and each resolves only
  in its own pane;
- a pane binding and a kernel action scoped to a *different* pane MAY share a
  chord, because the two panes are never focused at the same time.

#### Scenario: Two panes bind the same chord

- **WHEN** two plugin panes each declare a binding on the same chord and the
  first is focused
- **THEN** the chord resolves to the first pane's binding, and never to the
  second's

#### Scenario: A pane binding does not fire for another pane

- **WHEN** a pane declares a binding and a *different* pane is focused
- **THEN** the chord does not resolve to that binding

#### Scenario: A pane binding does not collide with another pane's action

- **WHEN** a plugin pane binds a chord a kernel action scoped to another pane
  already holds
- **THEN** both keep the chord, and each resolves only while its own pane is
  focused

### Requirement: A chord resolves to an action or to a pane binding, never both

Resolution SHALL produce at most one target for a keypress. When a chord is held
by both a kernel action that is active in the current context and a binding of
the focused pane, the **kernel action** MUST win, and the outcome MUST NOT depend
on map iteration order.

The kernel action wins because those chords are the user's escape route out of a
pane, and a pane that could shadow them could trap the user inside itself.

#### Scenario: Only a pane binding holds the chord

- **WHEN** a chord bound to the focused pane's binding is pressed and no active
  kernel action holds it
- **THEN** it resolves to that pane binding

#### Scenario: An active kernel action also holds the chord

- **WHEN** a hand-edited keymap binds one chord to both a global kernel action
  and a binding of the focused pane
- **THEN** the chord resolves to the kernel action, deterministically

#### Scenario: Nothing holds the chord

- **WHEN** a key that no action and no binding of the focused pane holds is
  pressed
- **THEN** it resolves to no target, and the pane still receives the raw key

### Requirement: A manifest default is dropped on collision, never stolen

When a plugin's declared chord is already bound to a kernel action whose scope
overlaps the pane's, or to another binding of the same pane, the host SHALL leave
the plugin's binding **unbound** and MUST NOT unbind the existing holder. The
drop MUST be reported.

This is deliberately asymmetric with a *user's* rebind, which steals: installing
a plugin must not silently move a key the user already uses, while a user asking
for a key is an instruction the keymap obeys.

#### Scenario: A declared chord is already a global action's

- **WHEN** a plugin declares a binding on a chord a global kernel action holds
- **THEN** the action keeps the chord, the plugin's binding has no chord, and the
  drop is reported

#### Scenario: Two bindings of one pane declare the same chord

- **WHEN** one pane declares two bindings on the same chord
- **THEN** the first keeps it, the second is left unbound, and the drop is
  reported

#### Scenario: An uncontested chord is bound

- **WHEN** a plugin declares a binding on a chord nothing overlapping holds
- **THEN** the binding carries that chord

#### Scenario: A user rebinds onto a kernel action's chord

- **WHEN** the user binds a pane binding to a chord a global action holds
- **THEN** the pane binding takes it, the action loses it, and the reassignment
  is reported to the user

### Requirement: A user's binding survives what the plugin says

A binding the user has rebound SHALL keep the user's chord, and MUST NOT be
overwritten when the plugin's declarations are registered again — at startup, on
a hot reload, or after the plugin changed its own default.

#### Scenario: A stored override wins over the manifest

- **WHEN** the keymap holds a stored override for a binding and the plugin's
  declaration names a different chord
- **THEN** the binding resolves on the user's chord

#### Scenario: A reload does not undo a rebind

- **WHEN** a plugin is reloaded after the user rebound one of its bindings
- **THEN** the binding still carries the user's chord

#### Scenario: Reset restores the manifest default

- **WHEN** the user resets a rebound pane binding
- **THEN** it carries the chord its manifest declared, or no chord when the
  manifest declared none

### Requirement: Pane bindings are editable in the keybinding editor

The interactive keybinding editor SHALL list every registered pane binding, in
its own section per pane, after the kernel's sections. Each row MUST show the
binding's chord and its human-readable name, and MUST support the editor's
existing operations — capture a new chord, reset this row, and reset everything.

Row indices MUST stay in step between the editor's model and its renderer, as
they already do for kernel actions, so a selection never edits a different row
from the one highlighted.

#### Scenario: A pane's bindings are listed

- **WHEN** a plugin declaring keybindings is running and the editor is opened
- **THEN** a section for its pane lists one row per binding with its chord

#### Scenario: No plugin declares a binding

- **WHEN** no registered binding exists
- **THEN** the editor shows exactly the kernel's sections, unchanged

#### Scenario: Capturing rebinds the selected pane binding

- **WHEN** a pane binding's row is selected and a chord is captured
- **THEN** that binding carries the captured chord

#### Scenario: Reset-all restores plugin defaults too

- **WHEN** every binding is reset from the editor
- **THEN** each pane binding carries its manifest default again

### Requirement: A rebound pane binding persists like any other

A pane binding's chords SHALL be written to and read from the same user
keybindings file the kernel's actions use, under a key that cannot collide with
an action name. The file MUST round-trip in a build with no plugin host, so a
user who switches builds does not lose the file's other contents.

An entry naming a binding no installed plugin declares MUST be retained as an
override rather than reported as an error, because a plugin being uninstalled or
temporarily failing must not silently discard the user's choice.

#### Scenario: A rebind survives a restart

- **WHEN** a pane binding is rebound and the keymap is written and read back
- **THEN** the binding carries the rebound chord

#### Scenario: An entry for an absent plugin is kept

- **WHEN** the file names a pane binding no discovered plugin declares
- **THEN** the entry is retained, and applies if that plugin appears later

#### Scenario: The file round-trips without the plugin host

- **WHEN** a file containing pane-binding entries is parsed by a build with no
  plugin host
- **THEN** parsing succeeds and the kernel's own bindings are unaffected

### Requirement: A registered binding requires the input capability

The host SHALL register a plugin's keybindings only when that plugin was granted
the capability to receive input. A plugin without it MUST contribute no keymap
entry, so a chord can never resolve to a plugin the host would refuse to deliver
to.

#### Scenario: A plugin without the input capability

- **WHEN** a plugin that was not granted input declares a keybinding
- **THEN** no keymap entry is registered for it

#### Scenario: A plugin with the input capability

- **WHEN** a plugin granted input declares a keybinding
- **THEN** the entry is registered and resolves while its pane is focused
