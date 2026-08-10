# migration/teardown Specification

## ADDED Requirements

### Requirement: A native pane's replacement is ready only on handover

A native pane's replacement verdict SHALL be derived from **handover**, not from
the existence of a second renderer. The probe MUST require both that the
replacement plugin exists and that the application no longer draws the native
pane; a plugin rendering a pane alongside the native one MUST leave the verdict
unready, because deleting the native renderer at that point would remove what
users see.

#### Scenario: A plugin exists but the native pane is still drawn

- **WHEN** a bundled plugin for a pane exists and the application still calls that
  pane's native renderer
- **THEN** the pane's replacement is recorded unready and the native renderer is
  still protected from deletion

#### Scenario: The pane is handed over

- **WHEN** a bundled plugin for a pane exists and the application no longer calls
  that pane's native renderer
- **THEN** the pane's replacement is recorded ready and its renderer may be
  deleted

#### Scenario: Neither exists

- **WHEN** no bundled plugin for a pane exists
- **THEN** the replacement is unready regardless of what the application draws
