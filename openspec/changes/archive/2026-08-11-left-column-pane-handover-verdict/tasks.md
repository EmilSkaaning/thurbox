# Tasks

## 1. The session list's handover verdict

- [x] `tests/session_list_pane_handover_gap.rs`: one row per unmet requirement —
      the active-session write, the `display_order` write, the sort, the scoped
      keys silenced by a plugin pane's focus, the left seat, the render trigger,
      the centred empty state, the border's status strip, the pending-spawn
      placeholder, and the module that is the kernel's model. Each with its probe.
- [x] Pin the two findings the spike could not have: that
      `App::focus_key_context` maps the plugin-pane focus to `Global` (so all six
      `KeyContext::SessionList` actions stop resolving), and that
      `src/ui/project_list.rs` is what `App` navigates, reorders, sorts and
      searches by.
- [x] Derive the verdict from the rows, and assert both directions.
- [x] Verify: `GIT_CONFIG_GLOBAL=/dev/null cargo test --test
      session_list_pane_handover_gap` and the same with `--features plugins` —
      identical results, since the gate reads source text. **6 passed** in both.

## 2. The automations pane's handover verdict

- [x] `tests/automations_pane_handover_gap.rs`: the central seat, the left seat,
      the creation key, the authoring key, the circular wrap, the module that is a
      model for the `Ctrl+P` modal too, the unknown focus, the focused pane that
      draws an unfocused border, the render trigger, and the fitted name — ten
      rows across all three gap kinds.
- [x] Pin that `render_central_pane` branches on the *native* pane's focuses, so a
      plugin pane taking focus removes the editor and the run history — the
      finding that decides this handover.
- [x] Pin that the two unported keys are the pane's whole **authoring** surface,
      read from the shipped manifest rather than from this test's memory, so
      "five of seven" cannot be read as a shortfall of two.
- [x] Verify in both feature configurations. **5 passed** in both.

## 3. Non-vacuity

- [x] Perturb `PaneSlot` with a `Left` variant → both gates fail naming
      `no-left-seat` with its full reason. Reverted.
- [x] Perturb `KernelWriter` with default-bodied `create_automation` +
      `move_session_order` → the automations gate fails naming
      `no-creation-operation` **and** `no-authoring-operation`; the session-list
      gate fails naming `no-session-record-write`. Reverted.
- [x] Perturb `render_central_pane`'s `matches!` with `InputFocus::PluginPane` →
      the automations gate fails on `recorded_blockers_match_the_tree` **and** on
      `the_central_pane_follows_the_native_panes_focus_not_the_panes_identity`,
      the latter with the re-verdict instruction. Reverted; `git diff` clean after
      each.

## 4. Documentation

- [x] `docs/ARCHITECTURE.md`: ADR-43 — both verdicts with their evidence, the
      third gap kind and why it is not a courtesy, the two capabilities
      deliberately not added, and the ordering that would unblock each pane.
- [x] `docs/PHASE4-PANE-READINESS.md` §18: the two attempts, the finding each
      turned up that its own port had not looked for, and the five rows the two
      panes share.
- [x] `docs/PHASE6-TEARDOWN-READINESS.md`: step 9 records both refusals and their
      shared rows; the automations row's reason is corrected from keys to seat.
- [x] `rumdl check .`

## 5. Hand-driven

- [x] `scripts/dev/sandbox.sh --show automations` in a 220×45 tmux pane, **default
      build, no `--features plugins`**, with three seeded `exec` automations.
      Observed, in order:
      - both panes on screen at once — native `Automations` at the bottom of the
        left column, plugin `Automations (plugin)` in the right column: the
        `no-left-seat` row, seen rather than reasoned;
      - the native pane at 38 columns renders ` ●  — daily 09:00 · exec · in 18h
        13m` — **the name is gone entirely** — while the plugin shows
        ` ● gamma — daily 09:00 · exec · in 18h…` clipped at the right: the
        `no-fitted-name` divergence showing two panes with *different*
        information;
      - focusing the **native** pane turns the central pane into the automation
        editor (name/trigger/hour/minute/timezone/action/command/next/status) plus
        a `Run history` panel; its ring is `Automations → Edit Automation → Run
        history` and never reaches the plugin pane;
      - focusing the **plugin** pane leaves the central pane **empty** — the
        finding, observed;
      - `j` twice moves the plugin's own cursor; `Space` flips `● → ○` and the DB
        (`alpha` → `enabled: false`); `r` recorded run id 1 `status: success` on
        the *kernel's* pass; `d` removed the row (`['gamma','beta']`); `Esc` left
        the pane;
      - `j` at the last row is declined and focus stays — the wrap's missing
        kernel half; `n` does nothing; `e` and `Enter` do nothing and the central
        pane stays empty.

## 6. Full verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --features plugins -- -D warnings`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo nextest run --all --features plugins`
- [x] `GIT_CONFIG_GLOBAL=/dev/null cargo test --test teardown_gate` — both rows
      still blocked
- [x] `./scripts/dev/lint-luau.sh`, `./scripts/dev/lint-workflows.sh`,
      `rumdl check .`
- [x] `openspec validate left-column-pane-handover-verdict --strict`
