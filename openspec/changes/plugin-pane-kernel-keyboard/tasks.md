# Tasks

## 1. The declaration, and the closed set it is validated against

- [x] `src/session/keybindings.rs`: derive `Serialize`/`Deserialize` on `KeyContext`
      (the same PascalCase wire spelling `Action` uses, so one name per concept
      wherever a user meets it), and add `KeyContext::pane_keyboards()` — the four
      contexts that scope a pane's keyboard — with the reasons `Global` and
      `Terminal` are not in it.
- [x] `src/session/plugin_manifest.rs`: `PaneDecl::key_context: Option<KeyContext>`,
      documented as "the pane this is, not a power it holds".
- [x] `src/session/plugin_manifest.rs`: validation — a context that scopes no pane is
      `ManifestErrorKind::PaneKeyContext` (listing the accepted set), two panes
      naming one context is `DuplicateKeyContext`, and a `[[keybindings]]` entry
      whose pane declared a context is `KeybindingOnKernelKeyboard`.
- [x] Tests in `src/session/plugin_manifest.rs`: one per scenario in the manifest
      delta (accepted, unknown name, `Global`/`Terminal` refused, duplicate,
      binding-on-such-a-pane, absent).
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'test(plugin_manifest)'`.

## 2. The pane carries it, and is focusable because of it

- [x] `src/plugin/pane.rs`: `PluginPane::key_context`, and `is_focusable_with` becomes
      "on screen **and** can receive keys" — `accepts_input || key_context.is_some()`.
- [x] `src/plugin/lifecycle.rs`: fill it from the decl in `panes()`, beside
      `toggle_action`/`feature`, with the same "manifest data the kernel acts on"
      note.
- [x] Tests: `src/plugin/pane.rs` (focusable without `input`), `src/plugin/lifecycle.rs`
      (the field survives publication).
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E 'binary(thurbox)'`.

## 3. Focus: such a pane is thurbox's own pane of that name

- [x] `src/app/mod.rs`: `App::keyboard_pane(KeyContext)` (the visible pane declaring
      it) and `App::focus_for_keyboard(KeyContext) -> InputFocus` (the one table from
      a context to the focus it is delivered to), with `#[cfg(not(feature =
      "plugins"))]` stubs.
- [x] `src/app/mod.rs`: `focusable_plugin_pane` skips a pane that declared a keyboard
      — it is focused as the kernel's pane, so landing on it as `PluginPane` would
      silence the keyboard it declared.
- [x] `src/app/key_handlers.rs`: the ring's right-column stops appear when **either**
      occupant is on screen (`App::pane_keyboard_taken`), and the left column's
      `j`/`k` hand-off follows the same predicate.
- [x] `src/app/mod.rs`: `App::pane_focus_level(KeyContext) -> FocusLevel`, and
      `render_tasks_panel` / the other native panes use it so one rule serves both
      occupants.
- [x] Verify: `cargo check --all && cargo check --all --no-default-features`.

## 4. The frame shows focus, and a click means what it meant

- [x] `src/app/view.rs`: `paint_plugin_pane` takes a `FocusLevel`;
      `render_plugin_panes` resolves it (a declared keyboard → the kernel's level for
      that context; an `input` pane → focused when it holds focus; otherwise
      inactive).
- [x] `src/app/view.rs`: for a pane that declared a keyboard, record the kernel's row
      action per row and `FocusPane(<inherited focus>)` for the rect, instead of
      `PluginPaneRow`.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`.

## 5. The behaviour, driven end to end

- [x] `src/app/acceptance.rs`: a pane declaring `key_context = "Tasks"` — the ring
      lands on it as `TaskList` with no `input` capability; `j`/`k` move the kernel's
      task cursor; `Space` cycles the selected task's status; the published section
      reports it focused; a row click selects that row; `Esc` leaves.
- [x] `src/app/acceptance.rs`: `a_focused_plugin_pane_draws_a_focused_border`, on the
      painted cells rather than on the level — a test that read the level back would
      pass for a painter that ignored it.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all -E
      'test(acceptance)'`.

## 6. The gates keep their verdicts, and say why

Revised while implementing: three recorded *sentences* became false, which a gate
exists to prevent, so scoping them was not optional.

- [x] `tests/automations_pane_handover_gap.rs`: `focused-pane-draws-an-unfocused-border`
      is **closed** — the gate failed on it, which is what it is for. Re-verdicted
      with a probe reading the painter's signature *and* the caller that resolves the
      level.
- [x] `tests/automations_pane_handover_gap.rs`: `central-seat-follows-the-native-focus`
      keeps its verdict but is re-scoped to the plugin-keys route, and
      `the_central_pane_follows_the_native_panes_focus_not_the_panes_identity` now
      asserts both halves — the wall is "focused as `PluginPane`", not "drawn by a
      plugin".
- [x] `tests/session_list_pane_handover_gap.rs`:
      `scoped-keys-silenced-by-the-handover` re-scoped the same way.
- [x] `tests/tasks_pane_input_gap.rs` and `tests/file_viewer_pane_input_gap.rs`: no
      row is re-verdicted — every one is about a power a *plugin's own* keys would
      need — but the module note records that the wall ADR-50 named is closed by a
      different route, so the next reader does not conclude the panes are still
      unreachable.
- [x] `tests/code_review_pane_handover_gap.rs`: a note recording that the route does
      **not** reach this pane, because its keys are not actions — which is why
      ADR-45's ordering stands.
- [x] `tests/bundled_manifests.rs`: no bundled manifest declares `key_context`, for
      ADR-47's reason (a reproduction inheriting the keyboard would paint two panes as
      focused).
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` and
      `--test architecture_rules`.

## 7. Documentation

- [x] `docs/ARCHITECTURE.md`: ADR-51, with the two routes and why the queue of grants
      was refused.
- [x] `docs/PHASE4-PANE-READINESS.md` §26: the focus wall, closed; what each of the
      four blocked panes still needs afterwards.
- [x] `src/plugin/bundled/thurbox.d.luau`: the `focused` fact's two readings.
- [x] `CLAUDE.md`: the plugin-pane paragraph gains the declaration.
- [x] Verify: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`,
      `./scripts/dev/lint-luau.sh`, `rumdl check .`.

## 8. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --no-default-features -- -D warnings`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --no-default-features`
- [x] `openspec validate plugin-pane-kernel-keyboard --strict`
