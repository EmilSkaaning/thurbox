# Phase 4 — bundled panes

## ADDED Requirements

### Requirement: A surface occupying more than one seat is not handed over as one pane

When a native surface occupies **two or more** of the interface's seats at once, the
migration record SHALL treat it as needing every one of them before it may be handed
to a plugin, and SHALL name which seats and what each contributes.

A pane replacement is per pane, and a plugin declares its panes in its own manifest,
so a surface spanning two columns is not "a pane with an extra region": it is two
panes that must be granted, focused and navigated together. Handing over only the
seat a plugin can take would remove the other from the interface while the
replacement still looked complete.

The code-review view is the first such surface: its diff owns the central pane, and
its changed-files list owns the file-viewer column with a focus and a keyboard of its
own, force-shown for as long as a review is open.

#### Scenario: A two-seat surface is refused as one pane

- **WHEN** a handover of a surface occupying two seats is attempted
- **THEN** it is refused, and the record names both seats and what handing over only
  one would remove

#### Scenario: The second seat is a pane in its own right

- **WHEN** the record describes the second seat
- **THEN** it states that the seat has its own focus and its own keys, so it is a
  second pane to be granted rather than a region of the first

### Requirement: A pane whose keys are a capture rather than actions loses them silently

When a native pane's keyboard is implemented as a **capture keyed on the focused
surface** rather than as bindable actions, the migration record SHALL state that a
handover loses those keys with nothing to name in their place.

The three earlier refusals could each point at the scoped actions a plugin binding
would have to replace, because those panes' keys are `Action`s in a `KeyContext`. A
capture has no such surface: the keys are not in the keybinding vocabulary, so they
are not rebindable, the interactive editor has never listed them, and a configuration
file cannot restore them after a handover.

The consequence MUST be recorded as an ordering constraint rather than only as a
blocker: such a pane's keys have to *become* actions before a handover can be
described, and that is a change to the keybinding vocabulary rather than to the
plugin surface.

#### Scenario: A capture-keyed keyboard is named as such

- **WHEN** the record examines the pane's keys
- **THEN** it states that the handler runs ahead of the keybinding lookup on the
  focused surface, that the keybinding contexts name no such surface, and that a
  focused plugin pane therefore resolves none of the pane's keys

#### Scenario: The ordering is stated

- **WHEN** the work the handover would need is ordered
- **THEN** turning the pane's keys into scoped actions precedes any plugin-facing
  change, because until then there is nothing for a plugin binding to claim

### Requirement: A mouse channel carrying a row cannot express a click that means a column

The migration record SHALL state where a pane's mouse surface exceeds the row channel
a plugin pane receives, entry by entry, and SHALL distinguish a *missing target kind*
from a *missing coordinate*.

A plugin pane's click reports the row of the pane's outermost list. A pane whose
mouse surface includes buttons, a draggable scrollbar, wheel scrolling or picker
entries is missing target **kinds**, each of which a wider event could carry. A pane
where the **column** a click landed in changes the meaning of the click is missing a
coordinate the row channel cannot hold at all — and that is the stronger statement,
because no additional target kind closes it.

The code-review view has both: eleven footer buttons, a scrollbar, a wheel and a
target picker, plus a paired side-by-side row where the half clicked decides which
side of the diff a comment attaches to.

#### Scenario: The two kinds of gap are separated

- **WHEN** the record enumerates the pane's mouse surface
- **THEN** the targets a wider event would carry are listed apart from the coordinate
  the row channel cannot carry, with the reason the second is not a subset of the
  first

### Requirement: A refused row states whether it is narrower than a row already refused

When a handover is refused for a reason another pane was already refused for, the
record SHALL state whether **this** pane's version of that reason is narrower, and if
it is, that it is the cheaper place to start.

A gate that only repeated "the cursor is kernel state" would hide a real difference:
the session list's cursor *is* the application's active session, so making it
writable is the widest grant in the host, while the code review's cursor is a row
inside a view the user already opened. Two rows spelled alike with very different
prices should not read alike.

#### Scenario: The narrower row is identified

- **WHEN** two panes are refused for a cursor the kernel owns
- **THEN** the record states which pane's cursor is the narrower grant, and names it
  as the order the work would be done in

### Requirement: A refusal adds no capability that has no consumer

A change that records a refusal SHALL NOT add a capability, a binding or a manifest
field in the same change.

A capability added alongside a verdict has no pane using it — the defect the earlier
gates identified in `input`, `tasks-write` and `automations-write`, which existed
before any bundled plugin declared them. The refusal's job is to state what is
needed and in what order; granting it belongs to the change that consumes it.

#### Scenario: The refusal changes no source

- **WHEN** the change recording the refusal is inspected
- **THEN** it adds tests and documentation and no capability, binding or manifest
  field
