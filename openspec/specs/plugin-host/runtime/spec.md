# plugin-host/runtime Specification

## Purpose
Defines the sandboxed script runtime that executes plugin code — one isolated
VM per plugin, bounded in time and memory, so that a slow, buggy, or hostile
plugin degrades only itself and never the orchestrator hosting it.
## Requirements
### Requirement: One isolated VM per plugin

The host SHALL create a separate VM for each loaded plugin, and plugins MUST
NOT be able to observe or mutate another plugin's VM state. Global values,
loaded modules, and registered functions created by one plugin MUST NOT be
visible to another.

#### Scenario: A plugin cannot read another plugin's globals

- **WHEN** plugin A assigns a global value and plugin B reads that global name
- **THEN** plugin B observes no value

#### Scenario: A plugin cannot mutate another plugin's environment

- **WHEN** plugin A replaces a standard library function in its environment
- **THEN** plugin B continues to observe the original function

### Requirement: Plugin execution does not block the render loop

Plugin code SHALL execute off the thread that draws frames. The host MUST NOT
call into a plugin VM from the render path, and a plugin that blocks
indefinitely MUST NOT prevent the UI from drawing frames or accepting input.

#### Scenario: A plugin blocks forever

- **WHEN** a plugin's entry point enters an infinite loop
- **THEN** the host continues to process input and draw frames
- **AND** the blocked plugin is reported as failed once its execution bound
  trips

### Requirement: Execution is bounded by an instruction budget

The host SHALL enforce a per-call instruction budget on plugin execution. A
plugin call that exceeds its budget MUST be interrupted, MUST cause that call
to fail with a budget-exceeded error, and MUST NOT leave the host waiting.

#### Scenario: A plugin exceeds its instruction budget

- **WHEN** a plugin call runs longer than its configured instruction budget
- **THEN** the call is interrupted and reported as a budget-exceeded failure
- **AND** the host records the failure against that plugin

#### Scenario: A plugin stays within its budget

- **WHEN** a plugin call completes within its instruction budget
- **THEN** the call returns normally and no failure is recorded

### Requirement: Memory is bounded per plugin

The host SHALL enforce a per-plugin memory ceiling. An allocation that would
exceed the ceiling MUST fail inside that plugin's VM rather than growing the
host process, and MUST be reported as a plugin-level failure.

#### Scenario: A plugin allocates past its ceiling

- **WHEN** a plugin allocates memory beyond its configured ceiling
- **THEN** the allocation fails within that VM
- **AND** the failure is reported against that plugin, and other plugins keep
  running

### Requirement: Plugin faults are contained

An error, panic, or abort raised by plugin code SHALL be caught at the host
boundary and converted into a plugin-level failure. A failing plugin MUST NOT
terminate the process, corrupt host state, or prevent other plugins from
running.

#### Scenario: A plugin raises an error

- **WHEN** plugin code raises an uncaught error
- **THEN** the host records a failure carrying the plugin id, the failing
  entry point, and the error message
- **AND** the process continues running and other plugins are unaffected

#### Scenario: A plugin fails during one call but not another

- **WHEN** one call into a plugin fails and a later call succeeds
- **THEN** the earlier failure does not prevent the later call from running

### Requirement: The plugin environment is restricted by default

A plugin's VM SHALL be created with no ambient access to the filesystem,
network, process spawning, environment variables, or the host clock beyond what
the host explicitly provides. Standard library entry points that would grant
such access MUST be absent from the plugin's environment.

#### Scenario: A plugin attempts ambient filesystem access

- **WHEN** plugin code calls a standard library function that would open a file
- **THEN** that function is absent from its environment and the call fails

#### Scenario: A plugin attempts to spawn a process

- **WHEN** plugin code calls a standard library function that would execute a
  command
- **THEN** that function is absent from its environment and the call fails

### Requirement: Plugin code cannot load arbitrary host code

The host SHALL restrict module resolution to the plugin's own directory and the
host-provided module namespace. A plugin MUST NOT be able to load code from
outside its own directory, including via relative paths that traverse upward.

#### Scenario: A plugin requires a path outside its directory

- **WHEN** plugin code requires a module path that resolves outside the
  plugin's own directory
- **THEN** the require fails with an error and no file outside the directory is
  read

#### Scenario: A plugin requires one of its own modules

- **WHEN** plugin code requires a module inside its own directory
- **THEN** the module loads in the same VM

### Requirement: Runtime cost is zero when no plugins are present

With the plugin feature compiled in and no plugins discovered, the host SHALL
create no VMs and spawn no plugin threads. Startup time in that configuration
MUST stay within 100% of the same build's startup time measured with the
plugin feature compiled out, using the existing `THURBOX_PERF_LOG=1`
`first_frame_ms` measurement. Because the host now runs during boot, this
budget is measured against a booting binary rather than being satisfied by the
host never being invoked.

#### Scenario: No plugins are installed

- **WHEN** the host starts with the plugin feature enabled and no plugins
  discovered
- **THEN** no VM is created and no plugin thread is spawned

#### Scenario: Startup budget with no plugins

- **WHEN** `first_frame_ms` is compared between a plugin-enabled build with no
  plugins and a build with the feature compiled out
- **THEN** the plugin-enabled measurement is within 100% of the other

#### Scenario: A missing plugin directory costs nothing

- **WHEN** the host starts and the user plugin directory does not exist
- **THEN** discovery completes without creating the directory, and no VM or
  thread is created

### Requirement: A plugin may walk a string by character

A plugin's VM SHALL be created with the standard library that decodes and encodes
UTF-8, so plugin code can iterate a string's **characters** rather than its bytes.

It is admissible under the restricted-environment rule because it grants no
ambient access of any kind: it reaches no file, no process, no environment
variable and no clock. It is pure computation over a string the host already
handed the plugin.

It is necessary rather than convenient. A pane that styles the *inside* of a line
— highlighting code, splitting a matched run, measuring an indent — must agree
with the host about where one character ends and the next begins, and the host
counts characters. Without it a plugin scanning a line containing any multi-byte
character drifts after the first one and every run to its right is wrong, which is
a silently incorrect pane rather than a refused one.

#### Scenario: A plugin iterates a multi-byte string

- **WHEN** plugin code walks a string containing multi-byte characters
- **THEN** it visits one character per iteration and can rebuild each as a string

#### Scenario: The addition grants no ambient access

- **WHEN** the plugin environment is enumerated
- **THEN** it still contains no filesystem, process, environment or clock library,
  and the addition is a pure-computation library like the arithmetic one beside it

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

