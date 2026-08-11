# plugin-host/view-tree Specification

## ADDED Requirements

### Requirement: A run may yield its width and be ellipsized by the kernel

A text run SHALL be able to declare that it **yields its width to the other runs on
its line**. The declaration MUST be optional and default to absent, so every line
already written is laid out exactly as before.

When a line holds one or more such runs, the kernel SHALL give every other run its
intrinsic width, hand the remainder to the yielding runs, and truncate them with an
ellipsis when they do not fit. A line with no yielding run MUST clip at the pane's
edge exactly as before.

**Consecutive yielding runs share one budget.** A string split into matched and
unmatched runs is one piece of text to a reader, so the ellipsis MUST fall where the
concatenation would have been cut, and the runs after the cut MUST draw nothing —
never one ellipsis per run.

The truncation SHALL use the **same** fitting the kernel's own panes use, so a
plugin's copy of a pane and that pane cannot disagree about where a title was cut.
The consequence MUST be accepted rather than papered over: that fitting counts
characters, so a run of double-width glyphs can still exceed its budget in cells, as
it does in the kernel's own panes.

A yielding run MUST NOT be given width at the expense of a fill. A fill is the
line's *residue* and a yielding run is bounded by what the fixed runs leave, so the
yielding runs are resolved first and a fill takes whatever remains after them —
which, on a full line, is nothing.

The declaration SHALL be a field of a run's style rather than a new node kind: it
describes how a run is drawn when its line runs out of room, and a node kind would
have to be threaded through every walk over the tree.

#### Scenario: A line that fits is untouched

- **WHEN** a line whose runs fit declares a yielding run
- **THEN** every run draws in full and no ellipsis appears

#### Scenario: A line that overflows

- **WHEN** a line overflows and one of its runs yields its width
- **THEN** the other runs keep their full width and the yielding run is truncated
  with an ellipsis

#### Scenario: A trailing marker survives the overflow

- **WHEN** an overflowing line ends with a fixed run after the yielding one
- **THEN** that run is still drawn, because the yielding run gave up the columns it
  needed

#### Scenario: Several yielding runs

- **WHEN** an overflowing line holds consecutive yielding runs
- **THEN** they are cut as one piece of text, with a single ellipsis at the cut and
  nothing drawn after it

#### Scenario: A line with no yielding run

- **WHEN** an overflowing line declares none
- **THEN** it clips at the pane's edge exactly as before
