# plugin-host/kernel-state Specification

## ADDED Requirements

### Requirement: The automations pane's rows are a published snapshot section

The published snapshot SHALL carry the automations thurbox has, as one section in
the order the automations pane lists them. The section MUST always be present —
"there are none" is knowledge the kernel has — so a pane iterates it without a nil
check, and it MUST carry every automation, enabled and disabled alike, rather than
only those due to fire.

Each row MUST carry what a pane draws it from: the automation's name, whether it is
enabled, its action's stable wire name, the resolved label of its schedule, the
whole seconds until it is next due, whether a running search filtered the row out,
and the byte offsets in the name that the search matched.

The section MUST NOT carry the marker glyph, the colour role, or the emphasis of a
row: which glyph an enabled automation draws as, and in what colour, is the pane's
decision.

This section is distinct from the already-published list of *upcoming* automations,
which is filtered to those that are enabled and scheduled and exists for a
different pane. Both MUST be readable through the same declared capability.

#### Scenario: Every automation crosses, not only the due ones

- **WHEN** a plugin holding the automations capability reads the pane section while
  one automation is enabled and scheduled and another is disabled
- **THEN** it receives both rows, each reporting its own enabled state

#### Scenario: A row carries no rendering

- **WHEN** a plugin reads a row
- **THEN** the row carries its name, enabled flag, action name, schedule label,
  countdown and search verdict, and carries no glyph, colour token or style

#### Scenario: Both automation readers answer from one capability

- **WHEN** a plugin declares the automations capability alone
- **THEN** both the upcoming-automations reader and the pane-section reader are
  present in its environment

### Requirement: A composed display string is published as its parts

When a pane's row shows a string the kernel composes from several facts, the
snapshot SHALL publish those facts and the plugin SHALL compose the string —
except for the parts a sandboxed plugin cannot compute, which the kernel MUST
resolve.

For an automation's summary that means the schedule's **resolved human label** (a
cron expression's meaning is thurbox's own vocabulary) and the **countdown in whole
seconds** (a plugin has no clock) are published, while the separator, the ordering,
and the precedence that shows `disabled` for a disabled automation, a countdown for
a scheduled one, and a placeholder for an enabled one with no next run, are the
plugin's.

The snapshot MUST NOT publish the finished summary string, because a pane assembled
from strings the kernel formatted is not evidence that a pane can own its
presentation.

#### Scenario: The parts cross and the string does not

- **WHEN** a plugin reads a row for an automation scheduled by a cron expression
- **THEN** it receives the schedule's human label, the action's name and a duration
  in seconds, and no field holds the assembled summary

#### Scenario: A disabled automation still reports its schedule

- **WHEN** the automation is disabled
- **THEN** the row still carries its schedule label and action, and the plugin is
  what decides that a disabled row shows `disabled` in place of a countdown

### Requirement: A list section's scroll anchor and drawn cursor are separate facts

A published list section SHALL carry the row a pane scrolls to and whether that row
is **drawn** as the cursor as two separate facts, because a pane may window to its
cursor while not showing one — thurbox's automations pane scrolls to the cursor's
row whether or not it holds focus, and highlights it only when it does or when a
global search is previewing a row there.

The kernel MUST resolve both, since a plugin can observe neither focus nor a search
preview. The section MUST NOT publish the same fact twice — a per-row "this row is
the cursor" flag alongside an index would be two representations that can disagree.

The section MUST also carry whether the pane holds focus when that changes what is
drawn, so an empty pane names the key that adds an automation only when the pane
can receive it.

#### Scenario: An unfocused pane still names its anchor

- **WHEN** the pane does not hold focus and the cursor is on a row below the fold
- **THEN** the section names that row as the anchor and reports that the cursor is
  not drawn, so a plugin's list scrolls there without highlighting it

#### Scenario: A search preview draws the cursor without focus

- **WHEN** a global search is previewing an automation while focus is in the search
  strip
- **THEN** the section reports the cursor as drawn

#### Scenario: The focus fact reaches the empty state

- **WHEN** there are no automations and the pane holds focus
- **THEN** the section reports focus, and the plugin's empty-state line is the one
  that names the key

### Requirement: The published automations section is bounded and respects its feature

The number of automation rows published SHALL be bounded, so a large automation
list cannot produce a view tree beyond the node budget — which would make every
render of an automations pane *fail* rather than merely scroll.

When more automations exist than the bound allows, the section MUST publish the
first rows up to the bound and MUST NOT publish an anchor that falls outside them.

The section MUST be empty when the automations feature is disabled, mirroring the
task and file sections: thurbox draws no automations pane and fires no schedules in
that configuration, so a pane advertising them would surface a disabled feature.

#### Scenario: A very long list is truncated rather than failing a render

- **WHEN** there are more automations than the bound
- **THEN** the section carries the bound's worth of rows and a pane built from it
  renders

#### Scenario: An anchor beyond the bound is not published

- **WHEN** the cursor is on a row past the bound
- **THEN** the section publishes no anchor

#### Scenario: The feature is off

- **WHEN** the automations feature is disabled
- **THEN** the section is empty
