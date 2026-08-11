# panes (delta)

## ADDED Requirements

### Requirement: A pane is rendered when a source it reads moves

A pane's re-render SHALL be triggered by a change in something the pane can read,
not by the passage of time. The host MUST know, per pane, which sources its
plugin's granted capabilities reach, and MUST render only the panes whose sources
moved.

A source is one of the published snapshot's sections — the sessions, the host
metrics, the automations, the tasks, the open file tree, the open review — or the
plugin's own durable state. Each state-reading capability SHALL name exactly one
source, and the mapping MUST be exhaustive over the capability vocabulary, so a
capability added later cannot reach a pane without declaring what it reads.

A pane MUST also be rendered when its plugin was offered input (its own state may
have moved in answering), when it has just become visible after being skipped, and
when its plugin has been reloaded.

A pane that the kernel publishes as hidden MUST NOT be rendered whatever moved, and
a change in a source no visible pane reads MUST cost no render at all.

#### Scenario: A source a pane reads moves

- **WHEN** the state behind a source changes and a visible pane's plugin holds the
  capability that reads it
- **THEN** that pane is re-rendered

#### Scenario: A source no visible pane reads moves

- **WHEN** the state behind a source changes and no visible pane's plugin holds the
  capability that reads it
- **THEN** no pane is rendered and no plugin VM is entered

#### Scenario: Nothing moves

- **WHEN** no source a visible pane reads changes
- **THEN** no pane is rendered, however long the worker runs

#### Scenario: A pane becomes visible

- **WHEN** a pane the worker was skipping is shown
- **THEN** it is rendered rather than waiting for a source to move

#### Scenario: A plugin is offered input

- **WHEN** a key or a click is offered to a pane's plugin
- **THEN** that pane is re-rendered, because answering may have changed the
  plugin's own state

#### Scenario: A capability is added to the vocabulary

- **WHEN** a new capability is added
- **THEN** the source mapping fails to compile until it names the source that
  capability reads, or records that it reads none

### Requirement: A pane's render rate is bounded, and the bound is coalescing only

The host SHALL bound how often a pane is rendered, at no more than ten render
passes per second, and MUST reach that bound by **coalescing** rather than by
delaying: a change that arrives after the bound's interval has elapsed MUST be
rendered immediately, and changes arriving inside it MUST be merged into one pass
at its end.

A bound is required because a source can move far faster than a user can perceive —
agent activity text can change on consecutive ticks — and an unbounded trigger would
put a plugin VM call on every one of them. It MUST NOT be reached by rendering on a
timer, because that is the fixed cadence this requirement replaces.

The bound MUST be no looser than the kernel's own forced-redraw floor, so a plugin
pane cannot be more than one forced frame behind the interface around it.

The residual latency the bound introduces SHALL be recorded rather than described as
zero.

#### Scenario: A change after a quiet period

- **WHEN** a source moves and no pane has been rendered within the bound's interval
- **THEN** the render happens immediately, with no wait

#### Scenario: A source moving faster than the bound

- **WHEN** a source changes on many consecutive ticks
- **THEN** the changes coalesce and the pane is rendered at most ten times a second

#### Scenario: A change inside the interval

- **WHEN** a source moves shortly after a render pass
- **THEN** the pane is rendered once the interval has elapsed, and the delay is
  recorded as the trigger's worst case

### Requirement: A pane whose source the kernel cannot observe is the only pane on a timer

A pane that may read its plugin's **own durable state** SHALL keep a periodic
re-render, because that state has no observable change event: a plugin's headless
half can write it with nothing on the UI thread knowing. Every other pane MUST have
no timer at all.

The periodic re-render MAY share the cadence of the source-file poll the host
already runs for reload, since both exist for the same reason — a change the kernel
cannot be told about. It MUST NOT be raised when no running plugin declares the
capability that reads that state, so a set of plugins that read only published state
costs zero idle renders.

#### Scenario: A pane reads its plugin's own state

- **WHEN** a visible pane's plugin holds the capability that reads its own durable
  state
- **THEN** that pane is re-rendered periodically, whether or not any published
  source moved

#### Scenario: No plugin reads its own state

- **WHEN** no running plugin holds that capability
- **THEN** no periodic re-render is raised and an idle host enters no plugin VM
