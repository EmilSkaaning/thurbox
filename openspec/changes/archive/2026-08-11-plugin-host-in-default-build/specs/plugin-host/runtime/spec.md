# plugin-host/runtime Specification

## ADDED Requirements

### Requirement: The plugin runtime is part of the default build

The plugin runtime SHALL be a member of the crate's default feature set, so that
a build produced with no feature flags — which is what every installer, every
package channel and every `cargo build` produces — contains the VM that draws a
bundled pane. The crate's declared minimum Rust version MUST cover the runtime's
own floor rather than only the floor of the default set without it, and the lint
configuration's minimum version MUST equal the declared one, since a per-feature
minimum is not expressible and no longer needs to be.

A deterministic check SHALL assert the inclusion, in the direction that can now
fail: the default dependency tree MUST contain the runtime crate. The check MUST
be part of a required CI job, because the build a user installs is the one whose
composition is easiest to change by accident and hardest to notice.

#### Scenario: The default dependency tree carries the runtime

- **WHEN** the default dependency tree is inspected with no feature flags
- **THEN** the runtime crate is present, and the check passes

#### Scenario: The runtime leaves the default feature set

- **WHEN** the runtime is removed from the default feature set, or a build
  suppresses default features
- **THEN** the check fails, naming the runtime crate as absent from the default
  dependency tree

#### Scenario: The declared minimum version covers the runtime

- **WHEN** the crate's declared minimum Rust version is compared with the
  runtime crate's own
- **THEN** the crate's is at least the runtime's, and the lint configuration
  declares the same version

### Requirement: The vendored runtime builds for every released target

The runtime is vendored C++ sources compiled at build time, so every target the
release workflow publishes SHALL have a C++ compiler available to its build
environment. A target whose toolchain provides only a C compiler MUST NOT be in
the release matrix while the runtime is in the default feature set, because the
build would fail at release time rather than in CI.

The requirement is verified by the release build matrix itself, and MUST be
reproducible outside it: for each cross-compiled target, the compiler the release
environment supplies MUST be named, so that a local build can be pointed at an
equivalent toolchain instead of the verdict resting on a green workflow run.

#### Scenario: A natively built target

- **WHEN** a release target is built on a runner of its own platform
- **THEN** that runner's system C++ compiler compiles the vendored sources

#### Scenario: A cross-compiled target

- **WHEN** a release target is cross-compiled
- **THEN** its build environment supplies a C++ compiler and a C++ standard
  library for that target, named in the release configuration rather than assumed

#### Scenario: A target with no C++ compiler

- **WHEN** a release target's build environment provides no C++ compiler for that
  target
- **THEN** the release build for that target fails, and the target is not
  published with a partial artifact
