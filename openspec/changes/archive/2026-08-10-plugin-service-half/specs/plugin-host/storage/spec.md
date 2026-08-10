## Purpose

Defines the durable key/value store a plugin owns — enough state to do useful
background work, namespaced so one plugin can neither read nor corrupt
another's.

## ADDED Requirements

### Requirement: A plugin reads and writes its own keys

A plugin with the storage capabilities SHALL be able to write a value under a
key and read it back, and the value MUST survive a restart of the host.

#### Scenario: Round trip

- **WHEN** a plugin writes a value and later reads that key
- **THEN** it gets the value back

#### Scenario: Persistence across restarts

- **WHEN** a plugin writes a value and the host restarts
- **THEN** reading the key still returns the value

#### Scenario: An unset key

- **WHEN** a plugin reads a key it never wrote
- **THEN** it gets nothing, not an error

### Requirement: Namespacing is applied by the host

The host SHALL derive a plugin's namespace from its identity and MUST NOT
accept a namespace supplied by the plugin. A plugin MUST NOT be able to read or
write another plugin's keys by any key it can construct.

#### Scenario: Two plugins use the same key name

- **WHEN** two plugins each write the same key name
- **THEN** each reads back its own value

#### Scenario: A plugin tries to escape its namespace

- **WHEN** a plugin uses a key containing separators or another plugin's name
- **THEN** it still addresses only its own namespace

### Requirement: Storage is bounded

The host SHALL bound key length, value size, and the number of keys one plugin
may hold, refusing a write that would exceed them rather than growing without
limit.

#### Scenario: An oversized value

- **WHEN** a plugin writes a value larger than the limit
- **THEN** the write fails with an error naming the limit

#### Scenario: Too many keys

- **WHEN** a plugin exceeds its key-count limit
- **THEN** the write fails rather than the store growing

### Requirement: Storage requires a capability

Reading and writing SHALL each require their own declared capability, and a
plugin without one MUST NOT have that binding.

#### Scenario: A plugin without write

- **WHEN** a plugin declared only read
- **THEN** the write binding is absent from its environment
