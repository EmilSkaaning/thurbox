# Tasks — the kernel/pane field map

## 1. Enumerate the fields

- [x] 1.1 Extract every field of `App` from `src/app/mod.rs` (the struct at line
      951), separating the four `#[cfg(feature = "plugins")]` fields from the rest,
      and record the totals the map's tally must reproduce.

**Verify:** the extracted count matches the field lines in the struct, and
`grep -c '#\[cfg(feature = "plugins")\]'` over the struct's line range accounts
for the gated ones

## 2. Write the map

- [x] 2.1 New `docs/KERNEL-BOUNDARY.md`: purpose, the map-not-a-refactor framing,
      and a tally table whose three classes are disjoint and sum to the field
      count.
- [x] 2.2 The kernel section, grouped by responsibility (sessions/backends,
      frame/loop, theme/keymap/config, pane visibility, input/mouse/selection,
      metrics/diagnostics, plugin host), each group naming the symbol that
      justifies it.
- [x] 2.3 The pane section: one row per field with its owning plugin and the code
      that shows the field is read by that pane alone.
- [x] 2.4 The service section: the background-task handles and the in-flight flow
      state, with the reason this is a third class rather than kernel.
- [x] 2.5 The fields that do not split cleanly: `modal` (by variant),
      `pending_editor_run` (intent vs execution), `active_index` vs
      `session_list_state`, `cached_session_order` vs `App::render_order_indices`,
      `pending_spawn`.
- [x] 2.6 A closing section naming what the map does **not** answer, including its
      own staleness (it is a snapshot, and why it is not a Rust test).

**Verify:** `rumdl check .`

## 3. Index it

- [x] 3.1 `docs/README.md`: list the new doc with its one-line purpose.
- [x] 3.2 `CLAUDE.md`: add it to the design-documentation list.

**Verify:** `rumdl check .`

## 4. Confirm nothing moved

- [x] 4.1 `git diff --stat` shows no change under `src/` or `tests/`.

**Verify:** `cargo fmt --all -- --check`; `cargo clippy --all-targets --features
plugins -- -D warnings`; `cargo clippy --all-targets -- -D warnings`;
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`;
`cargo nextest run --all` and `--all --features plugins` — all unchanged, since no
Rust is touched.
