## Purpose

Defines how a pane a plugin declared becomes a pane a user sees — where it
sits, when it is drawn, how its content is refreshed without plugin code ever
running during a frame, and what it shows when the plugin behind it is slow or
broken.

## ADDED Requirements

### Requirement: A declared pane becomes a rendered pane

A pane declared by a running plugin's manifest SHALL be available as a pane in
the UI, titled from the manifest and placed in the slot it declared. A pane
declared by a plugin that is not running MUST NOT be shown.

#### Scenario: A running plugin declares a pane

- **WHEN** a plugin with a declared pane reaches `running`
- **THEN** its pane is available and titled from the manifest

#### Scenario: A failed plugin declares a pane

- **WHEN** a plugin that declared a pane fails to start
- **THEN** no pane is shown for it

### Requirement: The kernel never calls a plugin during a frame

Painting a frame SHALL NOT invoke plugin code. The kernel MUST render from a
cached view tree, and a plugin's `render` MUST run off the thread that draws.

#### Scenario: A plugin's render is slow

- **WHEN** a plugin's `render` takes far longer than a frame
- **THEN** frames continue to be painted at the normal rate from the cached
  tree

#### Scenario: A plugin's render hangs

- **WHEN** a plugin's `render` never returns
- **THEN** the UI remains responsive and the pane keeps showing its last tree

### Requirement: A pane shows its last good content while refreshing

When a re-render is requested, the pane SHALL keep displaying the previous tree
until a new one arrives. A pane that has never rendered successfully MUST show
a loading state rather than being blank or absent.

#### Scenario: A refresh is in flight

- **WHEN** a re-render has been requested and has not completed
- **THEN** the pane still shows the tree from the previous successful render

#### Scenario: A pane has never rendered

- **WHEN** a pane's plugin has not yet returned a tree
- **THEN** the pane shows a loading state

### Requirement: A render failure is contained to its pane

A `render` that errors, exceeds its execution bounds, or returns an invalid
tree SHALL leave the previous tree on screen with an error indicator, and MUST
NOT blank the frame, panic, or affect another pane.

#### Scenario: Render raises an error

- **WHEN** a plugin's `render` raises
- **THEN** its pane shows the previous tree plus an error indicator
- **AND** every other pane renders normally

#### Scenario: Render returns an invalid tree

- **WHEN** a plugin's `render` returns a value that is not a valid view tree
- **THEN** the pane shows an error indicator and the frame is unaffected

#### Scenario: The first render fails

- **WHEN** a plugin's very first `render` fails
- **THEN** the pane shows an error state rather than a loading state forever

### Requirement: A plugin pane does not defeat the demand-driven loop

A plugin pane SHALL mark the UI dirty only when its content actually changes. A
pane whose plugin returns an unchanged tree, or returns nothing new, MUST NOT
cause a repaint, so the idle paint rate is unchanged from a build with no
plugin panes.

#### Scenario: An unchanged tree

- **WHEN** a plugin re-renders and returns a tree equal to the current one
- **THEN** no repaint is triggered

#### Scenario: A changed tree

- **WHEN** a plugin returns a tree different from the current one
- **THEN** the UI is marked dirty and the pane repaints

### Requirement: Plugin panes obey the existing layout rules

A plugin pane SHALL be subject to the same width thresholds and column sharing
as the native side panels, and MUST NOT displace or overlap them.

#### Scenario: The terminal is too narrow

- **WHEN** the terminal is narrower than the threshold for side panels
- **THEN** the plugin pane is not shown, exactly as a native side panel is not

#### Scenario: A native panel is open

- **WHEN** a plugin pane and a native side panel are both visible
- **THEN** neither overlaps the other
