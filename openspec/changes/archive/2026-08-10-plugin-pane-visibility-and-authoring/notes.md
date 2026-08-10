# Notes

## Deviation from ADR-V21

ADR-V21 specifies generated `<plugin>.<pane>.{toggle,show,hide}` commands.
Those need the command registry (Phase 5), so this change ships **one**
rebindable `TogglePluginPane` action instead, which delivers the property the
ADR is protecting — panes are toggleable, rebindably, without each plugin
inventing its own convention. With several plugin panes it toggles the first.
Recorded as an interim step, not a substitute.

## Toolchain note

`luau-analyze` in the pinned release does not resolve `@alias` requires, so
`scripts/dev/lint-luau.sh` type-checks a copy with the module specifier
rewritten to a relative path. A unit test
(`plugin::runtime::tests::the_lint_script_rewrites_the_specifier_this_host_actually_uses`)
asserts the script's rewrite matches `HOST_MODULE`, so the two cannot drift and
leave the bundled plugins silently unchecked.
