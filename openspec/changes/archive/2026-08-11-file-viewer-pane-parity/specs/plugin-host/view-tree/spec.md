# view-tree (delta)

## ADDED Requirements

### Requirement: A list may declare a scroll track

A list node SHALL be able to declare that it wants a scroll track. The
declaration MUST be optional and MUST default to absent, so a list that does not
declare one is laid out exactly as before — the panes that deliberately overflow
without a scrollbar MUST NOT gain one.

When a list declares a track and has more children than the rows it was given,
the **kernel** MUST reserve the rightmost column of the list's area for the
track, draw the thumb there at the declared cursor's position, and lay the rows
out in the width that remains. The column MUST be reserved by the same rule
thurbox's own panes reserve one with, so a native pane and a plugin reproducing
it cannot place the track in different columns or draw different thumbs.

When the list fits the rows it was given, no column MUST be reserved and no thumb
drawn: a track that appeared for a list with nothing to scroll would take a
column of content away for no information.

A list that declares a track and no cursor MUST be drawn with the thumb at its
first position rather than refused, because whether a cursor is published is a
decision of whatever the pane reads and a plugin's node shape must not depend on
it.

A plugin MUST NOT be told the width or the height the track was resolved
against, and MUST NOT be able to place, size or style the track: it declares
that the list scrolls, and the kernel owns where that is shown.

The rows a click resolves to MUST exclude the reserved column, so a click on the
thumb is never delivered as a click on a row.

#### Scenario: A declared track appears when the list overflows

- **WHEN** a plugin returns a list that declares a track and has more children
  than the pane has rows
- **THEN** the pane's rightmost column carries a thumb positioned at the declared
  cursor, and the rows are drawn in the remaining width

#### Scenario: A declared track is absent when everything fits

- **WHEN** a plugin returns a list that declares a track and every child fits
- **THEN** no column is reserved and the rows are drawn at the pane's full width

#### Scenario: A list that declares no track is unchanged

- **WHEN** a plugin returns an overflowing list that declares no track
- **THEN** the rows occupy the pane's full width and no thumb is drawn

#### Scenario: A track without a cursor is drawn, not refused

- **WHEN** a plugin returns an overflowing list that declares a track and no
  selected row
- **THEN** conversion succeeds and the thumb is drawn at its first position

#### Scenario: A click on the track is not a click on a row

- **WHEN** a user clicks the column the track occupies
- **THEN** no row hitbox contains that column, so the click does not select a row

### Requirement: The scroll-track declaration is part of the granted module surface

The module a plugin requires SHALL let it declare a track through the same list
constructor it already uses, rather than by spelling a node table by hand, and
the declared type surface MUST describe it — otherwise a strict type-check of a
bundled pane would reject the argument that makes it scroll.

A declaration that is not a boolean MUST be refused as a malformed node naming
the kind and the field, like every other bad field.

#### Scenario: A plugin declares a track through the constructor

- **WHEN** a plugin builds a list through the granted constructor and asks for a
  track
- **THEN** the resulting node carries the declaration and renders with one

#### Scenario: A non-boolean declaration is refused

- **WHEN** a plugin declares a track as a string or a number
- **THEN** conversion fails naming the node kind and the field, and the pane
  reports the error rather than drawing a list
