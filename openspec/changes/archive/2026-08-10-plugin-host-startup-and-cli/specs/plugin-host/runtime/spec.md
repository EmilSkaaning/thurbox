## MODIFIED Requirements

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
