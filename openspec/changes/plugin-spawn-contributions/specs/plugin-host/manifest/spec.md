## ADDED Requirements

### Requirement: The manifest declares a spawn contribution

A manifest MAY declare a spawn contribution naming environment variables.
Omitting it MUST mean the plugin contributes nothing, and an unrecognized key
inside it MUST be a manifest error rather than a silently ignored field.

#### Scenario: A manifest declares a contribution

- **WHEN** a manifest declares environment variables for spawned sessions,
  together with the capability that permits it
- **THEN** the manifest validates and the declaration is readable from it

#### Scenario: A manifest declares no contribution

- **WHEN** a manifest omits the spawn declaration
- **THEN** the manifest validates and the plugin contributes nothing

#### Scenario: A misspelled key inside the declaration

- **WHEN** a manifest's spawn declaration carries a key the host does not
  define
- **THEN** the manifest is rejected with an error naming the key
