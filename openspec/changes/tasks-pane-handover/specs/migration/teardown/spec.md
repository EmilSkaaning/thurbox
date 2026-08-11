# migration/teardown Specification

## MODIFIED Requirements

### Requirement: A native pane's replacement is ready only on handover

A native pane's replacement verdict SHALL be derived from **handover**, not from
the existence of a second renderer. The probe MUST require all four of the
following, and a pane failing any one MUST be recorded unready:

1. the replacement plugin exists;
2. the application no longer draws the native pane;
3. the runtime that draws the replacement reaches the build a user installs; and
4. the pane's equality oracle holds a **recorded** expectation, not only a
   comparison against the native builder the deletion removes.

The first two together are not sufficient. A plugin rendering a pane alongside the
native one has replaced nothing, and a replacement that only runs behind a
compile-time feature the released binary does not enable is not a pane a user
has — deleting the native renderer in either state removes what users see, which
is the one outcome the inventory exists to prevent.

The third condition SHALL be derived from the tree rather than recorded, on the
same terms as the other two, and it MUST be read from the declaration that
actually decides it — the default feature set the crate builds with — rather than
from a document, a comment, or the feature set the test process happens to have
been compiled with. It is a property of the build, so it MUST hold the same
verdict whether or not the gated feature is enabled while the check runs.

Because that condition is a property of the build rather than of any one pane, it
MUST apply uniformly to every pane row, so that the release decision which
unblocks them is visible as a single shared blocker instead of appearing as seven
independent pane problems.

The condition MUST remain checked once it is satisfied rather than being retired
as settled. It records that the runtime reaches installed builds, and that is a
release decision a later change could reverse — by suppressing default features
in the release build, or by removing the runtime from the default feature list —
in which case every pane already handed over would silently become an empty
column. So while the runtime is part of the default build, the inventory SHALL
assert that it is, and each pane row's verdict SHALL then turn on the remaining
pane-level conditions alone.

The inventory's own **worked example** of a blocked row MUST name a pane the
interface still draws. Every handover falsifies the example that names the pane it
hands over, and the repair that makes the suite pass is to invert that example's
assertions — turning an argument about why a reproduction is not a replacement into a
transcript of what the tree happens to say. So the example MUST be moved in the change
that hands its pane over, and a rule MUST fail when it has not been.

#### Scenario: A plugin exists but the native pane is still drawn

- **WHEN** a bundled plugin for a pane exists and the application still calls that
  pane's native renderer
- **THEN** the pane's replacement is recorded unready and the native renderer is
  still protected from deletion

#### Scenario: The native renderer is dropped while the replacement is feature-gated

- **WHEN** a bundled plugin for a pane exists, the application no longer calls that
  pane's native renderer, and the plugin runtime is absent from the default build
- **THEN** the pane's replacement is recorded unready, so the deletion of that
  renderer fails the inventory rather than being permitted

#### Scenario: The pane is handed over

- **WHEN** a bundled plugin for a pane exists, the application no longer calls that
  pane's native renderer, the plugin runtime is part of the default build, and the
  pane's oracle holds a recorded expectation
- **THEN** the pane's replacement is recorded ready and its renderer may be
  deleted

#### Scenario: Neither exists

- **WHEN** no bundled plugin for a pane exists
- **THEN** the replacement is unready regardless of what the application draws

#### Scenario: The build condition is shared by every pane

- **WHEN** the plugin runtime is absent from the default build
- **THEN** no pane row is ready, whatever each pane's plugin renders and whatever
  the application draws

#### Scenario: The build condition is satisfied and still asserted

- **WHEN** the plugin runtime is part of the default build
- **THEN** the inventory asserts that it is, and each pane row is unready only for
  its own pane-level reason — so removing the runtime from the default feature
  list fails the inventory instead of quietly emptying every handed-over pane

#### Scenario: A handover is proposed while the pane's oracle is differential

- **WHEN** a bundled plugin for a pane exists, the application no longer calls that
  pane's native renderer, the runtime is in the default build, and the pane's
  oracle holds no recorded expectation
- **THEN** the pane's replacement is recorded unready and the failure names the
  missing recording, because the deletion would leave an oracle that cannot fail

#### Scenario: A reproduced pane's recording is asserted to exist

- **WHEN** a bundled plugin reproduces a native pane
- **THEN** the inventory asserts that the pane's oracle records the native tree,
  so a pane reproduced without a recording fails before any handover is attempted

#### Scenario: A handed-over row is not required to still be drawn

- **WHEN** the rule that every pane row names a native renderer the application
  draws is checked after a handover
- **THEN** the handed-over row is exempt because its verdict is ready, and every
  blocked row is still required to name a renderer that is drawn

#### Scenario: A worked example names a pane that is still native

- **WHEN** the inventory illustrates why a reproduced-but-not-replaced pane is
  blocked
- **THEN** the pane it names is one the application still draws, so the example
  fails loudly if that pane is handed over without being replaced in the example

#### Scenario: The worked example is handed over

- **WHEN** the pane the inventory illustrates a blocked row with becomes handed over
- **THEN** a rule fails naming the example, so it is moved rather than inverted
