## Purpose

Defines `plugin.toml` — the declarative document that states what a plugin is
and what it provides, readable as pure data so the host knows every pane,
command, and keybinding a plugin offers before running any of its code.

## ADDED Requirements

### Requirement: Manifests are readable without a runtime

The host SHALL parse a plugin manifest into structured data without creating a
VM, loading plugin source, or executing plugin code. Parsing a manifest MUST
have no side effects beyond reading the manifest file itself.

#### Scenario: Manifest parsed with no runtime present

- **WHEN** the host parses a well-formed manifest
- **THEN** it returns the plugin's identity, provided panes, commands,
  keybindings, and requested capabilities
- **AND** no VM is created and no plugin source file is read

#### Scenario: Provided surfaces known before activation

- **WHEN** a plugin is discovered but has not been initialized
- **THEN** the host can enumerate every pane, command, and keybinding that
  plugin declares
- **AND** each entry carries the id of the plugin that declares it

### Requirement: Required manifest identity fields

A manifest SHALL declare a plugin `name` and an `api_version`. The `name` MUST
be a non-empty string of lowercase alphanumerics and hyphens, MUST start with a
letter, and MUST NOT exceed 64 characters. A manifest missing either field, or
carrying a `name` that violates those rules, MUST be rejected as invalid.

#### Scenario: Manifest missing a required field

- **WHEN** a manifest omits `name` or `api_version`
- **THEN** parsing fails with an error naming the missing field and the
  manifest path

#### Scenario: Manifest carries a malformed name

- **WHEN** a manifest declares a name containing uppercase letters,
  whitespace, a path separator, or a leading digit
- **THEN** parsing fails with an error naming the offending value

#### Scenario: Manifest declares only the required fields

- **WHEN** a manifest declares `name` and `api_version` and nothing else
- **THEN** parsing succeeds
- **AND** the plugin is treated as providing no panes, no commands, no
  keybindings, and requesting no capabilities

### Requirement: API version compatibility is checked before load

The host SHALL compare a manifest's declared `api_version` against the plugin
API version it implements, and MUST refuse to load a plugin whose declared
version it cannot satisfy. The check MUST happen before the plugin's VM is
created.

#### Scenario: Plugin declares a newer API version than the host

- **WHEN** a manifest declares an `api_version` the host does not implement
- **THEN** the plugin is rejected with an error stating both the declared and
  the supported version
- **AND** no VM is created for it

#### Scenario: Plugin declares a compatible API version

- **WHEN** a manifest declares an `api_version` the host implements
- **THEN** the compatibility check passes and the plugin proceeds to load

### Requirement: Declared surfaces carry stable identifiers

Every pane, command, and keybinding entry in a manifest SHALL carry an `id`
that is unique within that manifest. Ids MUST follow the same character rules
as a plugin name. A manifest declaring two entries of the same kind with the
same id MUST be rejected.

#### Scenario: Duplicate ids inside one manifest

- **WHEN** a manifest declares two commands with the same id
- **THEN** parsing fails with an error naming the duplicated id

#### Scenario: Same id used for different kinds

- **WHEN** a manifest declares a pane and a command that share an id
- **THEN** parsing succeeds, because ids are unique per kind

### Requirement: Unknown manifest fields are rejected

The host SHALL reject a manifest containing keys it does not recognize, rather
than ignoring them. A typo in a manifest MUST surface as an error naming the
unrecognized key, not as a silently missing feature.

#### Scenario: Manifest contains a misspelled key

- **WHEN** a manifest contains a key the schema does not define
- **THEN** parsing fails with an error naming the unrecognized key and the
  manifest path

### Requirement: Invalid manifests fail with actionable errors

A manifest that is unreadable, is not valid TOML, or violates any schema rule
SHALL produce an error that names the manifest path and the specific problem.
The host MUST NOT fall back to a default or partially-populated manifest.

#### Scenario: Manifest is not valid TOML

- **WHEN** a manifest file contains a syntax error
- **THEN** parsing fails with an error naming the path and the syntax problem

#### Scenario: Manifest file cannot be read

- **WHEN** a manifest file exists but cannot be read
- **THEN** parsing fails with an error naming the path and the I/O cause
