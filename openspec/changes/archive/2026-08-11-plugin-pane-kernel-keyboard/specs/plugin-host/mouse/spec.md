# plugin-host/mouse Specification

## MODIFIED Requirements

### Requirement: Only a plugin that declared input receives a click

A click SHALL be delivered only to a plugin that declared the input capability,
and only for a pane that is on screen. A pane whose plugin did not declare it MUST
NOT be focused by a click and MUST NOT be told about one — the same rule focus
navigation already applies.

A click is input; it is gated by the capability that gates input rather than by a
capability of its own.

A pane that declared one of the kernel's pane keyboards is the exception, and it is
not a widening: a click on one of its rows SHALL do what a click on that row of the
kernel's own pane did — select it, through the kernel's own handling — and a click
anywhere else in it SHALL focus it as that pane. Nothing is delivered to the plugin,
which is why the pane needs no input capability for its rows to be clickable. A pane
that declared no keyboard and no input capability MUST still record no click target
at all, so that a click on it falls through to whatever the interface does with the
region it occupies.

#### Scenario: A pane without the input capability is clicked

- **WHEN** a visible plugin pane whose plugin lacks the input capability and which
  declared no kernel keyboard is clicked
- **THEN** nothing is delivered and focus does not move to it

#### Scenario: A pane with the input capability is clicked

- **WHEN** a visible plugin pane whose plugin declared input is clicked
- **THEN** the click is delivered to that plugin

#### Scenario: A row of a pane that declared a kernel keyboard is clicked

- **WHEN** a row of a visible plugin pane that declared a kernel pane keyboard is
  clicked
- **THEN** the kernel selects that row of its own state, and nothing is delivered to
  the plugin

#### Scenario: Such a pane is clicked away from its rows

- **WHEN** a visible plugin pane that declared a kernel pane keyboard is clicked
  outside its rows
- **THEN** it holds focus as thurbox's own pane of that name
