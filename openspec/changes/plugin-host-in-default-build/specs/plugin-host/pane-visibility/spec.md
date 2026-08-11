# plugin-host/pane-visibility Specification

## ADDED Requirements

### Requirement: No bundled pane is on screen before a user asks for it

While a bundled plugin's pane reproduces a native pane rather than replacing it,
or exists as a worked example rather than as a pane a user asked for, its manifest
SHALL seed it hidden. The rule binds the whole bundled set, not the panes that
happen to have remembered it: a build that ships the host MUST look like the build
before it until the user shows something.

The seed defaults to visible, which is right for a plugin an author installed on
purpose and wrong for one that arrives inside the binary. So the bundled set MUST
be checked rather than reviewed — a bundled manifest that declares a visible pane
MUST fail unless that pane is the one drawn in a native pane's place.

#### Scenario: A reproduction pane ships hidden

- **WHEN** a bundled plugin reproduces a native pane that the application still
  draws
- **THEN** its manifest seeds the pane hidden, so a fresh launch shows one of that
  pane rather than two

#### Scenario: An example pane ships hidden

- **WHEN** a bundled plugin exists as a worked example of the plugin contract
- **THEN** its manifest seeds its pane hidden, so a fresh launch shows no pane the
  user did not ask for

#### Scenario: A bundled manifest omits the seed

- **WHEN** a bundled manifest declares a pane without saying whether it is visible
- **THEN** the check fails, because the seed's default is visible and would put
  that pane on every install
