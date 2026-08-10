## ADDED Requirements

### Requirement: A pane may declare its default visibility

A `[[panes]]` entry MAY declare whether it is visible by default. Omitting it
SHALL mean visible, so a plugin that says nothing behaves the way an author
expects.

#### Scenario: Default visibility is omitted

- **WHEN** a manifest declares a pane without a visibility default
- **THEN** the pane is treated as visible by default

#### Scenario: A pane opts out of being shown

- **WHEN** a manifest declares a pane as not visible by default
- **THEN** the parsed pane records that
