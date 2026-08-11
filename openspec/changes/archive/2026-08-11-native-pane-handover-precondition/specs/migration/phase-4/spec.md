# migration/phase-4 delta

## MODIFIED Requirements

### Requirement: The native pane survives the port

The port SHALL be additive. The native renderer MUST stay compiled in and MUST
remain the pane the interface draws by default, and the plugin's pane MUST NOT be
visible until a user asks for it. Replacing the native pane is a later phase,
and the teardown inventory MUST continue to protect the native renderer until
that handover happens.

That later phase has a precondition this phase MUST NOT be read as satisfying: a
pane may stop being drawn natively only when its replacement is present in the
build a user installs. While the plugin runtime is reachable only behind a
compile-time feature that released binaries do not enable, **no port may become a
handover**, however exactly the plugin reproduces the pane — dropping the native
renderer in that state removes the pane from every install while the only build
able to draw the replacement is one nobody runs.

A port MUST NOT satisfy this by keeping both renderers and selecting between them
on the compile-time feature. That leaves two renderings of one pane which differ
by build rather than one pane, and it hands nothing over: the native renderer is
still what the installed binary draws.

#### Scenario: The default interface is unchanged

- **WHEN** thurbox starts with the bundled plugin present and no stored
  visibility choice
- **THEN** the plugin's pane is off screen and the native pane renders as before

#### Scenario: The native renderer is still protected

- **WHEN** the teardown inventory is checked after the port
- **THEN** the native pane's renderer is still required to exist, because the
  pane has not been handed over

#### Scenario: A port is attempted as a handover while the runtime is gated

- **WHEN** a pane's plugin reproduces it exactly and the plugin runtime is absent
  from the default build
- **THEN** the native renderer stays the pane the interface draws, and the
  attempt is recorded with the release decision it waits on rather than landing
  as a handover

#### Scenario: The proof a handover offers is checked for being able to fail

- **WHEN** a handover claims that unchanged rendering snapshots demonstrate the
  replacement is equivalent
- **THEN** at least one of those snapshots must contain the pane, and a handover
  whose snapshots contain none of it MUST state that instead of citing them
