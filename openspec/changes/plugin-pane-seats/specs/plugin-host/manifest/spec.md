# plugin-host/manifest Specification

## MODIFIED Requirements

### Requirement: A pane declares the slot it occupies

A `[[panes]]` entry SHALL declare which slot its pane occupies, drawn from a
closed set the host defines. A pane declaring an unrecognized slot MUST be
rejected at manifest validation, before any VM is created.

The set SHALL cover every seat one of the kernel's own panes occupies: the
right-hand column, the left column, the band beneath the left column, the narrow
column left of centre, and the central pane. Each slot other than the right-hand
column SHALL name exactly one region of the workspace tree, and that mapping MUST
be readable as data — the host resolves a slot to a region in one place, so no two
consumers can disagree about where a slot is.

The right-hand column SHALL remain the default, so a manifest that says nothing
about placement keeps the placement it had.

No slot SHALL name a region that is not a pane seat — the header, the footer, the
full-width search strip and the transient status band are kernel chrome and are not
addressable by a manifest.

#### Scenario: A pane declares a known slot

- **WHEN** a manifest declares a pane with a slot the host defines
- **THEN** the manifest validates and the pane carries that slot

#### Scenario: A pane declares an unknown slot

- **WHEN** a manifest declares a pane with an unrecognized slot
- **THEN** validation fails naming the offending slot

#### Scenario: A pane omits its slot

- **WHEN** a manifest declares a pane with no slot
- **THEN** the pane takes the host's default slot, the right-hand column

#### Scenario: A pane asks for a native pane's seat

- **WHEN** a manifest declares a pane in the left column, the band beneath it, the
  column left of centre, or the central pane
- **THEN** the manifest validates and the slot resolves to the region the kernel's
  own pane for that seat occupies

#### Scenario: No slot reaches kernel chrome

- **WHEN** every slot the host defines is resolved to a region
- **THEN** none of them names the header, the footer, the search strip, or the
  status band
