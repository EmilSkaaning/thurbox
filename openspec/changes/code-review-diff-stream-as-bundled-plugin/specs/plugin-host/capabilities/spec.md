# plugin-host/capabilities Specification

## ADDED Requirements

### Requirement: Reading the open review's diff is its own declared capability

The closed capability vocabulary SHALL include a **review** capability, and the
binding that reads the published review section MUST be inserted into a plugin's
module table only when that capability is granted.

It MUST be separate from the capabilities that read sessions, metrics,
automations, tasks and files, for the reason the first four were separated: a
plugin that wants a diff must not have to demand host telemetry or the user's task
list to get one, and the capability list is what an install prompt shows.

Denial MUST remain by **absence**. A plugin without the capability MUST find no
binding to call, rather than a binding that refuses.

It MUST NOT be a version-control capability and MUST NOT be named as one: it
grants no diff of a plugin's choosing, no revision range, no file read and no
command. A capability that could produce a diff would be strictly more power for
strictly less result, since the pane's rows are the review the user opened.

#### Scenario: A plugin without the capability finds no binding

- **WHEN** a plugin that has not declared the review capability inspects its module
  table
- **THEN** the reader is absent

#### Scenario: A plugin with the capability reads the section

- **WHEN** a plugin declaring the review capability calls the reader
- **THEN** it receives the published review section

#### Scenario: The capability is not a git capability

- **WHEN** the vocabulary is enumerated
- **THEN** it defines no capability that runs a version-control command or reads a
  repository, and the review capability's single binding reads only what is
  published
