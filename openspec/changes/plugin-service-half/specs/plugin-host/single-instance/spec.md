## Purpose

Defines the machine-wide lock that keeps exactly one host running a given
plugin's service, so a TUI and a background tick never both drive the same
poll loop.

## ADDED Requirements

### Requirement: One holder per plugin service

At most one host SHALL run a plugin's service at a time. A second host
attempting to start the same service MUST be refused rather than starting a
duplicate.

#### Scenario: A second host contends

- **WHEN** one host holds a plugin's service lock and another tries to start it
- **THEN** the second is refused and does not start the service

#### Scenario: Different plugins do not contend

- **WHEN** two hosts start services for two different plugins
- **THEN** both succeed

### Requirement: The lock names its holder

The lock SHALL record which host holds it, so a refusal can say who is running
the service rather than only that something is.

#### Scenario: Reporting the holder

- **WHEN** a host is refused the lock
- **THEN** the refusal identifies the current holder

### Requirement: A dead holder does not wedge a plugin

The lock SHALL expire, so a host that was killed without releasing it cannot
prevent the service from ever running again.

#### Scenario: An expired lock is reclaimable

- **WHEN** a lock's holder has not renewed it past its expiry
- **THEN** another host may take it

#### Scenario: A live holder is not displaced

- **WHEN** a lock's holder keeps renewing it
- **THEN** another host is still refused

### Requirement: Releasing frees the service immediately

A host that stops a service SHALL release its lock, so the next host can take
it without waiting for expiry.

#### Scenario: Clean handover

- **WHEN** a host releases the lock and another tries to take it
- **THEN** the second succeeds
