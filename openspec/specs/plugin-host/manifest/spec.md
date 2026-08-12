# plugin-host/manifest Specification

## Purpose
Defines `plugin.toml` — the declarative document that states what a plugin is
and what it provides, readable as pure data so the host knows every pane,
command, and keybinding a plugin offers before running any of its code.
## Requirements
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

### Requirement: A pane declares the slot it occupies

A `[[panes]]` entry SHALL declare which slot its pane occupies, drawn from a
closed set the host defines. A pane declaring an unrecognized slot MUST be
rejected at manifest validation, before any VM is created.

The set SHALL cover every seat one of the kernel's own panes occupies: the
right-hand column, the left column, the band beneath the left column, the narrow
column left of centre, and the central pane. Each slot other than the right-hand
column SHALL name exactly one region of the workspace tree, and that mapping MUST
be readable as data — the host resolves a slot to a region in one place, so no two
consumers can disagree about where a slot is.

The right-hand column SHALL remain the default, so a manifest that says nothing
about placement keeps the placement it had.

No slot SHALL name a region that is not a pane seat — the header, the footer, the
full-width search strip and the transient status band are kernel chrome and are not
addressable by a manifest.

#### Scenario: A pane declares a known slot

- **WHEN** a manifest declares a pane with a slot the host defines
- **THEN** the manifest validates and the pane carries that slot

#### Scenario: A pane declares an unknown slot

- **WHEN** a manifest declares a pane with an unrecognized slot
- **THEN** validation fails naming the offending slot

#### Scenario: A pane omits its slot

- **WHEN** a manifest declares a pane with no slot
- **THEN** the pane takes the host's default slot, the right-hand column

#### Scenario: A pane asks for a native pane's seat

- **WHEN** a manifest declares a pane in the left column, the band beneath it, the
  column left of centre, or the central pane
- **THEN** the manifest validates and the slot resolves to the region the kernel's
  own pane for that seat occupies

#### Scenario: No slot reaches kernel chrome

- **WHEN** every slot the host defines is resolved to a region
- **THEN** none of them names the header, the footer, the search strip, or the
  status band

### Requirement: A pane may declare its default visibility

A `[[panes]]` entry MAY declare whether it is visible by default. Omitting it
SHALL mean visible, so a plugin that says nothing behaves the way an author
expects.

#### Scenario: Default visibility is omitted

- **WHEN** a manifest declares a pane without a visibility default
- **THEN** the pane is treated as visible by default

#### Scenario: A pane opts out of being shown

- **WHEN** a manifest declares a pane as not visible by default
- **THEN** the parsed pane records that

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

### Requirement: A command declaration carries its documentation and policy

A `[[commands]]` entry MAY declare a description, whether an agent may call it,
and its caller policy. Omitting them MUST yield conservative-by-documentation
defaults: agent-callable with an allowing policy, since a command that no
surface can reach is useless and every generated command is reversible.

#### Scenario: A fully specified command

- **WHEN** a manifest declares a command with a title, description, agent
  callability, and a policy
- **THEN** the manifest validates and each value is readable from it

#### Scenario: A minimal command

- **WHEN** a manifest declares a command with only an id
- **THEN** it validates, is agent-callable, and its policy allows

#### Scenario: An unrecognized policy

- **WHEN** a manifest declares a caller policy the host does not implement
- **THEN** the manifest is rejected with an error naming the policies that exist

#### Scenario: A misspelled key in a command

- **WHEN** a command declaration carries a key the host does not define
- **THEN** the manifest is rejected with an error naming the key

### Requirement: A command declares typed arguments

A command MAY declare arguments, each with an identifier name and one of the
types `string`, `integer`, or `boolean`. A repeated argument name, a malformed
name, or an unrecognized type MUST be a manifest error.

#### Scenario: Typed arguments validate

- **WHEN** a command declares a required string argument and an optional integer
  argument
- **THEN** the manifest validates and both are readable with their types

#### Scenario: A repeated argument name

- **WHEN** a command declares the same argument name twice
- **THEN** the manifest is rejected naming the argument

#### Scenario: A malformed argument name

- **WHEN** an argument name breaks the identifier rules
- **THEN** the manifest is rejected naming the argument and the rule

#### Scenario: An unrecognized argument type

- **WHEN** an argument declares a type the host does not define
- **THEN** the manifest is rejected naming the type

### Requirement: An argument may not be named after a global CLI flag

An argument name matching one of `thurbox-cli`'s global output-format flags
SHALL be a manifest error. Such an argument could never receive its value,
because the CLI matches its own global flags before a command's arguments.

#### Scenario: An argument named after an output flag

- **WHEN** a command declares an argument named after a global output-format
  flag
- **THEN** the manifest is rejected with an error naming that flag

#### Scenario: An ordinary argument name

- **WHEN** a command declares an argument whose name matches no global flag
- **THEN** the manifest validates

### Requirement: An identity default is restricted to string arguments

An argument MAY declare that its value defaults from the calling session or the
calling task. Declaring it on an argument that is not a `string` MUST be a
manifest error, because the identity it fills is an id.

#### Scenario: An identity default on a string

- **WHEN** a string argument declares that it defaults from the calling session
- **THEN** the manifest validates

#### Scenario: An identity default on an integer

- **WHEN** an integer argument declares an identity default
- **THEN** the manifest is rejected naming the argument

#### Scenario: An unrecognized identity source

- **WHEN** an argument declares an identity source the host does not define
- **THEN** the manifest is rejected naming the sources that exist

### Requirement: A keybinding declaration names its pane, its chord and its capability

A keybinding declaration SHALL carry a stable `id`, the `pane` it is scoped to, an
optional human-readable `title`, and an optional default `chord`. The manifest
MUST be rejected when:

- the `pane` names no pane the same manifest declares — the binding would be
  scoped to nothing;
- the `chord` cannot be parsed by the keymap's chord grammar — the same grammar
  the user keybindings file uses, so a chord means one thing everywhere;
- the manifest declares a keybinding without requesting the capability to receive
  input — the binding could never be delivered.

Each rejection MUST name the offending binding, mirroring the rejection of a pane
declared without the capability to render: a declaration the host would never act
on fails where the error names its own fix, rather than becoming a key that
silently does nothing.

A declaration with **no** chord MUST be valid: it is how a plugin ships an action
without claiming a key, leaving the user to bind it.

#### Scenario: A binding names an unknown pane

- **WHEN** a manifest declares a keybinding whose pane it does not declare
- **THEN** validation fails naming the binding and the pane

#### Scenario: A binding declares an unparsable chord

- **WHEN** a manifest declares a keybinding whose chord the grammar does not
  accept
- **THEN** validation fails naming the binding and the chord

#### Scenario: A binding without the input capability

- **WHEN** a manifest declares a keybinding and does not request the input
  capability
- **THEN** validation fails naming the binding and the missing capability

#### Scenario: A binding with no chord

- **WHEN** a manifest declares a keybinding with a pane and no chord
- **THEN** the manifest validates and the binding is registered unbound

#### Scenario: A well-formed binding

- **WHEN** a manifest declares a pane, the render and input capabilities, and a
  keybinding naming that pane with a parsable chord
- **THEN** the manifest validates and the declaration carries all four fields

### Requirement: A pane may declare the kernel action that toggles it

A `[[panes]]` entry MAY name the kernel action that shows and hides it, spelled as
the user keybindings file spells that action — one spelling for an action wherever a
user meets it. Omitting it MUST leave the pane reachable only through the generic
plugin-pane toggle, exactly as before.

The name SHALL be validated against a **closed set**: the actions whose purpose is
to show or hide a pane. A name that is not an action at all, an action that is not
one of those, and the generic plugin-pane toggle itself MUST each be a manifest
error naming the offending value and the actions that are accepted — the generic
toggle because it already reaches every declared pane, so binding it would toggle a
pane twice.

Two panes in one manifest MUST NOT name the same action: one key flipping two of a
plugin's own panes together is a declaration the host refuses rather than honours.

#### Scenario: A pane names a pane-toggle action

- **WHEN** a manifest declares a pane naming an action whose job is to show or hide
  a pane
- **THEN** the manifest validates and the pane carries that action

#### Scenario: A pane names something that is not an action

- **WHEN** a manifest declares a pane naming an action the host does not define
- **THEN** validation fails naming the offending value

#### Scenario: A pane names an action that is not a pane toggle

- **WHEN** a manifest declares a pane naming a real kernel action whose job is not
  showing or hiding a pane
- **THEN** validation fails naming the action and listing the actions that are
  accepted

#### Scenario: A pane names the generic plugin-pane toggle

- **WHEN** a manifest declares a pane naming the action that already toggles every
  declared pane
- **THEN** validation fails, because the pane would be toggled twice

#### Scenario: Two panes name one action

- **WHEN** a manifest declares two panes naming the same action
- **THEN** validation fails naming that action

#### Scenario: A pane names no action

- **WHEN** a manifest declares a pane without naming an action
- **THEN** the manifest validates and the pane carries none

### Requirement: A pane may declare the feature flag that gates it

A `[[panes]]` entry MAY name the whole-feature switch that gates it, spelled as the
settings file spells that switch. Omitting it MUST mean the pane is gated by no
feature.

The name SHALL be validated against a **closed set** of the switches that exist. An
unrecognized switch MUST be a manifest error naming it, never a silently ignored
field — a pane gated on a flag the host does not have would be a pane that either
never appears or is never gated, and the manifest cannot say which was meant.

#### Scenario: A pane names an existing switch

- **WHEN** a manifest declares a pane naming a feature switch the host defines
- **THEN** the manifest validates and the pane carries that switch

#### Scenario: A pane names an unknown switch

- **WHEN** a manifest declares a pane naming a switch the host does not define
- **THEN** validation fails naming the offending value

#### Scenario: A pane names no switch

- **WHEN** a manifest declares a pane without naming a switch
- **THEN** the manifest validates and the pane is gated by no feature

### Requirement: A pane may declare the kernel keyboard it is the pane for

A `[[panes]]` entry MAY name the kernel key context whose keyboard it answers,
spelled as the kernel spells that context (`key_context = "Tasks"`). Omitting it
MUST leave the pane exactly as panes were before: focusable only if its plugin
declared input, and answering only its own bindings.

The name SHALL be validated against a **closed set**: the contexts that scope a
*pane's* keyboard. A name that is no context at all, and a real context that scopes
no pane, MUST each be a manifest error naming the offending value and listing the
contexts that are accepted. The global context is refused because it belongs to no
pane, and the terminal context is refused because its keys are forwarded to a
process rather than dispatched as actions — a pane claiming it would receive
nothing and no error would say why.

A context whose kernel surface exists only **conditionally** — a pane that is on
screen for as long as some kernel state holds and absent otherwise — SHALL be in that
set on the same terms as any other. The condition is the kernel's and is enforced
where focus is resolved, not in the manifest: a pane declaring such a keyboard is
focusable exactly while the surface exists, and receives nothing while it does not.
A manifest MUST NOT be able to state the condition, for the reason a manifest cannot
state a seat's precedence — a plugin cannot see thurbox's surfaces, and a declared
condition would let one manifest decide when another's pane is reachable.

Two panes in one manifest MUST NOT name the same keyboard: one keyboard belongs to
one pane, and a keypress that reached two of a plugin's own panes would have no
defined meaning. Two panes of one manifest MAY name **different** keyboards, which is
what a surface drawn as two panes in two columns needs.

A `[[keybindings]]` entry naming a pane that declared a keyboard MUST be a manifest
error. Such a pane answers thurbox's own actions; a binding of its own would be a
second answer to one keypress, and the host refuses the declaration rather than
silently preferring one.

#### Scenario: A pane names a pane keyboard

- **WHEN** a manifest declares a pane naming a context that scopes a pane's keyboard
- **THEN** the manifest validates and the pane carries that context

#### Scenario: A pane names something that is not a context

- **WHEN** a manifest declares a pane naming a key context the host does not define
- **THEN** validation fails naming the offending value

#### Scenario: A pane names a context that scopes no pane

- **WHEN** a manifest declares a pane naming the global or the terminal context
- **THEN** validation fails naming the context and listing the contexts that are
  accepted

#### Scenario: A pane names a conditionally present keyboard

- **WHEN** a manifest declares a pane naming a context whose kernel surface is present
  only while some kernel state holds
- **THEN** the manifest validates, and the pane is a focus stop exactly while that state
  holds

#### Scenario: Two panes name one keyboard

- **WHEN** a manifest declares two panes naming the same context
- **THEN** validation fails naming that context

#### Scenario: Two panes name different keyboards

- **WHEN** a manifest declares two panes naming two different pane keyboards
- **THEN** the manifest validates and each pane carries its own context

#### Scenario: A pane with a keyboard also declares a binding

- **WHEN** a manifest declares a keybinding whose pane declared a kernel keyboard
- **THEN** validation fails naming the binding and the pane

#### Scenario: A pane names no keyboard

- **WHEN** a manifest declares a pane without naming a context
- **THEN** the manifest validates and the pane carries none

