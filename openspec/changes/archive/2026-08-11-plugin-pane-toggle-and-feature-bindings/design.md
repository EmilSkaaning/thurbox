# Design

See `proposal.md` — Why. This records the shape chosen and what was rejected.

## 1. The action is spelled the way the keymap spells it

`toggle_action: Option<Action>`, deserialized by serde straight into
`session::keybindings::Action` — so a manifest writes `"ToggleInfoPanel"`, the same
token `keybindings.json` uses and the same one the F1 editor persists. One spelling
for one action, everywhere a user might meet it.

Rejected: **a separate kebab-case vocabulary** (`toggle-action = "toggle-info-panel"`,
matching `slot` and `capabilities`). It would give every action two names, and the
one place a user compares them — "which key toggles this pane?" — is exactly where
the mismatch would show.

Rejected: **a free-form string resolved at dispatch time.** A typo would then be a
key that silently does nothing, which is the failure mode every other manifest field
is validated to prevent.

## 2. Two closed sets, and the second is narrower than "an action"

Serde closes the first set: an unknown name is a parse error naming it, exactly as
an unknown `capability` or `slot` is.

`validate()` closes the second: the action must be in `Action::pane_toggles()`, the
six whose *job* is to show or hide a pane. `ManifestErrorKind::PaneToggleAction`
names the offending action and lists the six, so the error carries its own fix.

**Why curate rather than accept any action.** The field exists for a *handover*:
taking over the key a native pane already answers. Binding `QuitApp` would make a
pane toggle when the user quits — nonsense the host would have to honour. Two
exclusions are worth stating:

- **`TogglePluginPane`.** It already toggles every declared pane (ADR-28), so a pane
  binding it would flip twice and end where it started. Refused rather than
  special-cased at dispatch.
- **`GlobalSearch`.** The strip is a mode, not a pane (§10), and a plugin cannot be
  seated in its band anyway (ADR-46).

`OpenAutomations`, `ToggleHelp`, `OpenSettings` and `TogglePerfHud` are not in the
set either: they open a modal or an overlay, not a pane in a seat. The automations
pane consequently has **no** action to bind — it is always-present, feature-gated —
and that is why the field is optional rather than required.

Two panes in one manifest binding the same action is a manifest error
(`DuplicateToggleAction`): one key flipping two of a plugin's own panes together is
a declaration nobody means. Across *plugins* it is legal and both toggle, because
the host cannot arbitrate between manifests it did not write together.

## 3. The flag is settings.toml's own key

`feature: Option<FeatureFlag>`, where `FeatureFlag` is a new enum in
`session::settings` whose wire names are the `[features]` keys (`snake_case`), with
`FeatureFlags::enabled(flag)` as the single lookup.

Rejected: **a `bool` field on `PaneDecl` per flag**, or a string looked up by hand at
each read. The enum plus one accessor is what makes "an unknown flag is a manifest
error" free and keeps the mapping in one `match` the compiler checks.

**Every flag is accepted, not a curated subset.** Unlike the action set there is no
nonsense case: gating a pane on any switch the user has is a coherent statement, and
a reproduction gates on the flag its native counterpart rides. The set is closed by
the enum, which is what the requirement asks for.

An exhaustiveness test guards the enum against the struct: a settings file that sets
**every** `FeatureFlag::all()` name to `false` must parse to a `FeatureFlags` with
every field false. A field with no enum member is then a failing test, because its
key would never be written.

## 4. Both occupants toggle

The hook is at the top of `App::dispatch_action`, before the kernel's own dispatch
chain: every pane whose `toggle_action` matches flips, then the action runs as it
always did.

Rejected: **the plugin steals the action** (the kernel effect suppressed while a
pane declares it). It reads like a handover, and it is what the *end state* looks
like — but while both panes exist it would leave the native pane unreachable by any
key, and a third-party plugin declaring `ToggleInfoPanel` would remove the user's
info panel with no way back. Toggling both keeps ADR-46's reversibility rule: the
kernel never loses track of its own pane's state.

Rejected: **hooking each of the six actions' handlers.** Six edits that must each
remember to do the same thing, versus one funnel every action already passes
through.

The hook runs **before** the per-action feature gate, so a pane answers its action
even when the kernel pane's own flag is off — each occupant is gated by the flag it
named, not by the other's. A pane whose own flag is off is skipped.

## 5. The gate is one predicate, read everywhere visibility is

`PluginPane` carries `feature: Option<FeatureFlag>` and answers:

- `is_enabled(&FeatureFlags)` — the declared flag is on (or none was declared);
- `is_shown(&FeatureFlags)` — `visible && is_enabled`;
- `is_focusable_with(&FeatureFlags)` — `is_shown && accepts_input`.

`is_focusable()` is **replaced** rather than kept beside `is_focusable_with`: two
predicates for one question is how a gate gets forgotten at one of its call sites.
Every read of `visible` that decides what the user sees now goes through
`is_shown` — the seat resolver, the right-column count, the painter, focus, motion,
and the hidden-set publication (so a gated-off pane's VM is not entered either).

The flag is evaluated at **read** time, from `App::features`, not resolved when the
pane set is published: `[features]` is live-reloadable (the settings panel and the
mtime poll both re-apply it), and a value baked in at publication would be stale
until the next plugin reload. `FeatureFlags` is `Copy`, so each reader snapshots it
and the borrow checker stays out of the way.

**A gated-off pane keeps its stored visibility.** The gate answers "is this pane
available", the stored choice answers "does the user want it" — collapsing them
would silently erase a choice when a flag went off and back on.

## 6. Where the types live

- `Action::pane_toggles()` — `src/session/keybindings.rs`, beside `Action`.
- `FeatureFlag`, `FeatureFlags::enabled` — `src/session/settings.rs`.
- `PaneDecl::{toggle_action, feature}`, the two error variants —
  `src/session/plugin_manifest.rs`, which already reaches sideways to
  `session::keybindings` for the chord grammar.
- `PluginPane::{toggle_action, feature}` and the three predicates —
  `src/plugin/pane.rs` (`plugin` may reference `session`).
- The action hook — `src/app/key_handlers.rs`; the gate's readers — `src/app/mod.rs`,
  `src/app/view.rs`, `src/app/motion_state.rs`.

Checked against `tests/architecture_rules.rs`: no module gains a reference it did not
already have, and `ui` still never learns that a pane has a feature flag.
