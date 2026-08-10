## ADDED Requirements

### Requirement: Commands are registered from manifests without running a plugin

The host SHALL build the command registry from discovered manifests alone. A
plugin's commands MUST be listable and describable without creating its VM, so a
plugin that has never started, or whose code faults, still contributes its
commands to the registry.

#### Scenario: A plugin that has not started

- **WHEN** the registry is built over a discovered plugin declaring a command,
  and no plugin VM has been created
- **THEN** the command is in the registry with its title, description, and
  argument schema

#### Scenario: A plugin whose code is broken

- **WHEN** a plugin's entry source fails to compile but its manifest is valid
- **THEN** its commands are still in the registry

#### Scenario: A plugin declaring no commands and no panes

- **WHEN** a manifest declares neither commands nor panes
- **THEN** it contributes nothing to the registry

### Requirement: A command id is namespaced by its plugin

Every command in the registry SHALL be identified as `<plugin>.<declared-id>`.
Two plugins declaring the same local id MUST produce two distinct registry
entries.

#### Scenario: Two plugins declare the same local id

- **WHEN** two discovered plugins each declare a command with the same local id
- **THEN** both appear in the registry under their own plugin-qualified ids

#### Scenario: A declared id cannot spell a generated one

- **WHEN** a manifest declares a pane and a command whose local ids are equal
- **THEN** the pane's generated command ids and the declared command's id are
  all distinct, because a declared id may not contain a dot

### Requirement: The kernel generates a visibility command per declared pane

For every pane a manifest declares, the host SHALL register
`<plugin>.<pane>.toggle`, `<plugin>.<pane>.show`, and `<plugin>.<pane>.hide`
with no plugin code involved. These commands MUST be handled by the kernel, not
dispatched to the plugin.

#### Scenario: A pane gets three commands

- **WHEN** a manifest declares one pane
- **THEN** the registry holds that pane's toggle, show, and hide commands

#### Scenario: Two panes get six commands

- **WHEN** a manifest declares two panes
- **THEN** each pane has its own three commands, addressable independently

#### Scenario: Generated commands take no arguments

- **WHEN** a generated visibility command is described
- **THEN** its argument schema declares no properties

### Requirement: Arguments are typed and validated before dispatch

A command SHALL declare its arguments with a name and one of the types `string`,
`integer`, or `boolean`. The host MUST reject an invocation carrying an
undeclared argument, a value of the wrong type, or a missing required argument,
and MUST NOT dispatch it.

#### Scenario: A well-typed invocation binds

- **WHEN** an invocation supplies every required argument with the declared type
- **THEN** the arguments bind and the command is dispatched

#### Scenario: A missing required argument

- **WHEN** an invocation omits a required argument
- **THEN** it fails with an argument error naming that argument, and the command
  is not dispatched

#### Scenario: An undeclared argument

- **WHEN** an invocation supplies an argument the command does not declare
- **THEN** it fails with an argument error naming that argument

#### Scenario: A value of the wrong type

- **WHEN** an invocation supplies a non-numeric value for an integer argument
- **THEN** it fails with an argument error naming the argument and the expected
  type

#### Scenario: An optional argument may be omitted

- **WHEN** an invocation omits an argument that is not required
- **THEN** it binds without that argument and dispatches

### Requirement: The argument list emits JSON Schema

A command's declared arguments SHALL be expressible as a JSON Schema object
describing its properties and its required set, so a command list can be handed
to an agent as a tool definition with no translation.

#### Scenario: Schema for a typed command

- **WHEN** a command declaring a required string and an optional integer is
  described
- **THEN** the emitted schema is an object whose properties carry those two
  types and whose required set names only the string

#### Scenario: Schema for a command with no arguments

- **WHEN** a command declaring no arguments is described
- **THEN** the emitted schema is an object with no properties and no required
  entries

### Requirement: An argument may default from the caller's identity

A `string` argument MAY declare that its default comes from the calling
session or the calling task. When the caller's identity supplies it and the
invocation does not, the host SHALL fill it; an explicitly supplied value MUST
always win.

#### Scenario: The default is filled

- **WHEN** a command declares an argument defaulting from the calling session,
  the caller is inside a session, and the invocation omits the argument
- **THEN** the argument binds to the calling session's id

#### Scenario: An explicit value overrides the default

- **WHEN** the same invocation supplies the argument explicitly
- **THEN** the supplied value is used

#### Scenario: No identity is available

- **WHEN** the argument is required, defaults from the calling session, and the
  caller is not inside a session
- **THEN** the invocation fails with a missing-argument error rather than
  binding an empty value

### Requirement: A caller policy decides whether an agent may invoke a command

A command SHALL declare whether an agent may call it and under which policy.
An invocation from inside a thurbox session MUST be refused when the command is
not agent-callable or its policy denies agent callers; the same command invoked
outside a session MUST run.

#### Scenario: An agent invokes an allowed command

- **WHEN** a command is agent-callable with an allowing policy and is invoked
  from inside a session
- **THEN** it runs

#### Scenario: An agent invokes a denied command

- **WHEN** a command's policy denies agent callers and it is invoked from inside
  a session
- **THEN** it is refused with a denied error and is not dispatched

#### Scenario: A user invokes a denied command

- **WHEN** the same denied command is invoked from outside any session
- **THEN** it runs

#### Scenario: A command that is not agent-callable

- **WHEN** a command declares that agents may not call it and it is invoked from
  inside a session
- **THEN** it is refused with a denied error

### Requirement: A plugin command runs against the service half

A command a plugin implements SHALL be dispatched to that plugin's service half,
so it works with no TUI running. A command on a plugin with no service half MUST
fail with an unavailable error naming the missing half.

#### Scenario: A command with no TUI

- **WHEN** a plugin command is invoked and no TUI is running
- **THEN** it still runs, and its return value is reported

#### Scenario: A plugin with no service half

- **WHEN** a plugin declares a command but ships no service entry
- **THEN** the invocation fails with an unavailable error naming the missing
  service half

#### Scenario: A command the plugin never implemented

- **WHEN** a manifest declares a command whose id has no matching handler in the
  service half's exports
- **THEN** the invocation fails with an error naming the command

### Requirement: A command's return value is converted with bounds

A command's return value SHALL be converted to a JSON value. The conversion
MUST be bounded in depth and in total size, and MUST refuse a value it cannot
represent rather than substituting a placeholder.

#### Scenario: A table becomes an object

- **WHEN** a command returns a table of scalar keys and values
- **THEN** the result is a JSON object carrying them

#### Scenario: A sequence becomes an array

- **WHEN** a command returns a table whose keys are a dense sequence from one
- **THEN** the result is a JSON array in that order

#### Scenario: Nothing returned

- **WHEN** a command returns no value
- **THEN** the result is JSON null and the invocation succeeds

#### Scenario: An excessively deep value

- **WHEN** a command returns a table nested past the conversion depth limit
- **THEN** the invocation fails with an error saying so

#### Scenario: A value that cannot be represented

- **WHEN** a command returns a function
- **THEN** the invocation fails with an error naming the type
