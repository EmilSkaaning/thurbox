# The kernel boundary — where every `App` field lands

v2 turns thurbox into a plugin host: a Rust kernel that owns sessions, backends,
git, storage, the loop, the layout and the frame, plus Luau plugins that own the
panes. Phase 4 of that migration ports the panes one at a time, and each port has
to answer "what state does this pane own?".

Answered per pane at port time, that question gets seven inconsistent answers: the
first pane sets a precedent nobody agreed to, and the fifth discovers that a field
it needs was claimed by the second. So it is answered here, once, before any pane
moves.

**This is a map, not a refactor.** Nothing here moves code. No field, type or
module changes; `src/` and `tests/` are untouched by the change that produced this
document. It exists so a pane port starts from a decision instead of making one.

Counted against `src/app/mod.rs` (14,958 lines; the `App` struct at line 951) with
the plugin host present:

| | Fields |
|---|---|
| In every build | 81 |
| Behind `#[cfg(feature = "plugins")]` | 4 |
| **Total** | **85** |

## 1. The tally

Three classes, disjoint, summing to 85 — enumerated in §2–§4 rather than asserted,
because a map whose columns do not add up invites a reader to assume the missing
field was considered.

| Class | Fields | What migration means for it |
|---|---|---|
| **Kernel** | 58 | Stays in the kernel model |
| **Pane** | 11 | Moves into the owning plugin's VM-local state |
| **Service** | 16 | Kernel-owned work a pane *asks for*; becomes a host call whose result arrives as an event, not a plugin field |

**The ratio is the finding: only 11 of 85 fields are pane state.** What reads as a
monolith of pane concerns is mostly session supervision, the frame, and
asynchronous plumbing — all of which stay. The cross-cutting cost of adding a pane
in v1 is not that panes own a lot of state; it is that a new pane must be threaded
through every *parallel table* — a `App` field, an `InputFocus` variant, an
`Action`, a `KeyContext`, a `FeatureFlags` flag, a `PanelAreas` rect, a
`ClickAction`, a `SettingsField` row, a `focus_ring` stop, and a snapshot.
`InputFocus` alone already has eleven variants, nine of which are panes.

The three-class split is not cosmetic. Two classes would be enough if every field
either stayed or left, and a third of them do neither: they are handles on work
only the kernel can perform, which a pane triggers and renders. Calling those
"kernel" would be true and useless, because it hides the migration shape Phase 4
actually needs — request, then wait for an event.

## 2. Kernel (58)

State the kernel must own because it outlives panes, arbitrates between them, or
routes to them.

**Sessions and backends (18).** ADR-V1's core responsibility; never a plugin,
because these hold PTYs, SSH transports and the database.

`sessions`, `active_index`, `backends`, `agents`, `hosts`, `db`,
`session_counter`, `deferred_inputs`, `session_terminal_views`, `remote_restore`,
`cached_hook_states`, `hook_states_version`, `pending_remote_hook_events`,
`last_output_gen`, `last_active_session_id`, `usage`, `usage_tx`, `usage_rx`.

- `cached_hook_states` / `hook_states_version` are the ADR-P6 cache: hook rows
  reloaded only when `PRAGMA data_version` moves. Session *status* is derived from
  them in `App::refresh_session_statuses`, which also drives OS notifications and
  the stuck-`working` fallback — one derivation, many consumers, none of them a
  pane's.
- `last_output_gen` is the lock-free output-change signature behind
  `App::detect_output_redraw`. It is how the demand-driven loop learns an agent
  wrote something without touching a `vt100` parser, so it belongs to the frame,
  not to whatever pane displays the output.
- `deferred_inputs` delays an `Enter` after a paste. It is keyed by `SessionId`
  and drained on the tick, so it survives any pane being hidden.

**Frame, loop and clock (8).** `focus`, `should_quit`, `terminal_rows`,
`terminal_cols`, `needs_redraw`, `last_draw_at`, `spinner_frame`,
`status_message`.

`needs_redraw` and `last_draw_at` are the demand-driven paint gate
(`App::request_redraw`); `spinner_frame` is the kernel's animation clock.
ADR-V18 *generalizes* `spinner_frame` rather than moving it: a plugin declares
motion and the kernel keeps the frame counter, because a push-per-frame model
would put a VM call on the render loop.

**Theme, keymap and config (7).** `active_theme`, `keybindings`, `features`,
`motion_settings`, `config_warnings`, `config_reload`, `notification_state`.

- `motion_settings` is kernel for the same reason `features` is: the render path
  reads it every frame, so it cannot come from the write-once settings global. It
  carries `reduce_motion`, which suppresses **every** animation app-wide — a
  property no single pane can be allowed to decide.
- `features` is the one field that *disappears* rather than moving:
  `[features]` becomes enabling and disabling plugins.

**Pane visibility (4).** `show_info_panel`, `show_tasks_panel`,
`show_file_viewer`, `show_session_list`.

The deliberate carve-out. Plugin-owned visibility is circular: a suspended plugin
cannot show its own pane, and the "pane became visible" event is what is meant to
wake it. `PluginPane::visible` already carries the rule in its own doc comment —
the manifest seeds it, the user owns it thereafter.

**Input, mouse and selection plumbing (7).** `text_selection`, `scrollbar_hits`,
`dragging_scrollbar`, `click_targets`, `mouse_hover`, `selected_text_cache`,
`clipboard`.

Kernel by construction. `click_targets` and `scrollbar_hits` are rebuilt every
`App::view` from what was rendered and hit-tested by `App::handle_mouse_click`, so
they are derived from the view tree rather than owned by whoever produced it.
Selection is pane-*scoped* but kernel-*owned*: a drag can begin in one pane and
the extracted text is one clipboard.

**Metrics, updates and diagnostics (10).** `metrics`, `metrics_refresh`,
`disk_usage`, `update_status`, `version_check_task`, `auto_update_rx`,
`perf_log_env`, `show_perf_hud`, `perf_window_base`, `startup_phases`.

`metrics_refresh`, `disk_usage` and `version_check_task` are background handles
like §4's, but they are not service state: no pane requests them. They refresh on
the kernel's own cadence and feed the header, the info panel and the perf
snapshot.

**Plugin host (4, gated).** `plugin_panes`, `plugin_events`, `plugin_keys`,
`motion` — present only with `--features plugins`.

All four are the *host's* state, not a plugin's, and the distinction is the whole
architecture:

- `plugin_panes` holds each pane's identity, slot, visibility, input capability
  and last-pushed presentation. It is what a plugin *produced*, not the state the
  plugin reasons over — that never leaves its VM.
- `plugin_events` / `plugin_keys` are the channels across the thread boundary.
  Trees are produced off-thread and applied on the tick; painting only reads what
  is already here.
- `motion` (`app/motion_state.rs`) holds epochs and leases. Every animation is
  driven from here because a plugin has no API to ask for a frame; `MotionState`
  drops every key absent from the current trees each pass, which is the single
  rule behind "a hidden pane drops its lease" and "motion state cannot leak".

## 3. Pane state (11)

Read and written only by the owning pane. Each moves into that plugin's VM,
unreachable by anything but its own reducer.

| Field | Owning plugin | Why it is the pane's |
|---|---|---|
| `file_viewer` | files | Tree expansion, scroll, and the in-file find sub-mode; nothing outside the pane reads it |
| `code_reviews` | review | A `HashMap<SessionId, CodeReviewState>` reached only through `App::active_review`; see below |
| `automation_ui` | automations | Cached list, selection, run-history cursor and the in-pane editor |
| `task_ui` | tasks | Cached list, selection, editor and the related-session link marks |
| `global_search` | search | Query, debounce deadline, results and the restore snapshot |
| `cached_session_order` | session list | A render-only cache; but see §5 |
| `session_list_state` | session list | The ratatui scroll offset — *not* the cursor; see §5 |
| `theme_picker_page` | theme picker (overlay) | The rendered list height, written by the view so paging steps one screenful |
| `new_session` | repo picker / new-session flow (overlay) | The wizard's inputs across its modal steps |
| `modal` | **split by variant** | See §5 |
| `pending_editor_run` | **split into intent and execution** | See §5 |

`code_reviews` is the instructive one. It is keyed by `SessionId` and holds one
review per session so switching away and back keeps it open — which is exactly the
shape a plugin's own namespaced key/value storage takes. It needs no kernel table:
the durable half (comments, reviewed marks) is already in SQLite, and the
in-memory half is per-session pane state.

## 4. Service state (16)

Kernel-owned work that panes *request*. These do not become plugin fields; they
become host calls whose results arrive as events — which is the concrete reason a
non-trivial plugin is substantially cache management. It cannot call and wait, so
it holds the last answer it was given.

**Background task handles (9).** `git_stats`, `branch_list`, `worktree_create`,
`session_spawn`, `review_build`, `repo_dir_listing`, `repo_path_check`,
`repo_parent_import`, `automation_exec`.

Every one runs git, tmux, SSH or a shell — capabilities a plugin is never granted.
`review_build` is the clearest case: the review pane's own diff pipeline (base
resolution, commit listing, `git diff`, possibly over SSH) runs off-thread and is
applied by `App::poll_review_build`, so even the pane that *is* the consumer only
ever reads a result.

**In-flight flow state (7).** `pending_worktree_create`, `pending_session_spawn`,
`pending_spawn`, `pending_delete`, `repo_picker_gen`, `sync_state`,
`worktree_sync`.

`pending_spawn` is the field most likely to be mis-assigned, so it is worth
spelling out. It renders a **placeholder row inside the session list** while the
non-blocking new-session flow runs (ADR-P12) — which makes it look like
session-list pane state. It is the kernel's, because the flow spans three
background phases plus the modals between them and must survive the list being
hidden. Under v2 the session-list plugin learns about it through a kernel event
and renders the placeholder as an ordinary row. Getting this wrong means a spawn
silently losing its progress indicator when a user presses F9.

`repo_picker_gen` is the same shape at smaller scale: a generation stamp that lets
a result outliving its modal be recognised and dropped. A plugin cannot hold it,
because the point is that it survives the pane instance.

## 5. The fields that do not split cleanly

Five, each a decision rather than an oversight.

**`modal` splits by variant.** One enum covers fifteen overlays. Most become
`overlay`-slot plugin panes — theme picker, settings, repo picker, the
confirmations, the pickers. `Modal::Help`, the F1 keybinding editor, **stays
kernel**: its core operation is capturing the next physical keypress *including
chords the kernel would otherwise intercept*, and a plugin cannot receive a
keypress that is routed elsewhere. So the carve-out is exactly one modal, not
"modals are kernel".

**`pending_editor_run` splits into intent and execution.** `Ctrl+O` from a session
row is a pane's intent; running it requires suspending the TUI or opening a tmux
popup, which only the kernel can do — it is drained by
`App::take_pending_editor_run` from the main loop, not from `App`. The pane side
becomes a command invocation; the field stays.

**`active_index` is kernel; `session_list_state` is the pane's.** These read as
synonyms and are not. `active_index` is "which session is active", and it drives
the central pane, the info panel, the footer, and every command that defaults to
the current session — the session-list plugin *proposes* changes to it and renders
the highlight, but does not own the value. The session-list spike
(`docs/SPIKE-SESSION-LIST.md`) made this a condition of the pane being a plugin at
all: if the plugin owns the cursor, every `j`/`k` costs a full render round trip.
`session_list_state` is the ratatui scroll offset that keeps the highlighted row
on screen — a rendering detail of one pane.

**`cached_session_order` is pane state whose ordering rule the kernel also
consumes.** The cache itself is render-only: `App::view` rebuilds it when
`App::session_order_signature` changes and nothing else reads it. But the *rule*
it caches, `ui::project_list::compute_session_order`, is called a second time and
uncached by `App::session_order`, which backs `App::render_order_indices` — the
order `Ctrl+J`/`Ctrl+K` step through — and `App::move_active_session`. The spike's
Luau port implemented the grouping, ordering and nesting inside the plugin. Both
cannot be true: if the plugin owns ordering, kernel navigation has no source for
it. **This is a Phase 4 obligation, not a cache detail** — the resolution is that
the plugin's pushed row identities become the order the kernel navigates, which is
the third column of the spike's second condition ("the plugin supplies rows and
row identity"). It is recorded here because it is invisible until the pane is
ported and then blocks it.

**`session_terminal_views` is kernel, though it looks like central-pane state.**
It records agent-vs-shell per session. The central pane is where the PTY is
rendered, and PTY lifecycle is ADR-V1 kernel work; the field is keyed by
`SessionId` and must survive any pane arrangement.

## 6. What this does not answer

- **Where the `cfg` boundary falls for each Phase 0 item.** The view-tree types
  and the native renderer are ungated because they are behaviour-preserving and
  snapshot-identical; the host is gated. The general rule is still open.
- **The order panes migrate in.** That is ordered by coupling, not by field count,
  and `docs/PHASE4-PANE-READINESS.md` is the audit of what the API cannot yet
  express for the first of them.
- **How the remaining kernel fields are arranged.** This says which fields
  *leave*, not whether `App` is later decomposed into sub-structs.
- **What each service field's host call looks like.** The map says a service field
  becomes a call whose result arrives as an event; the API is the backend
  contract's problem.
- **Its own staleness.** This is a snapshot at 85 fields, and it will drift. It is
  deliberately not a Rust test asserting completeness against the struct: reading
  `App`'s field list from a test means either grepping `src/app/mod.rs` as text — a
  parse that breaks on the next `#[cfg]` — or adding a macro or derive over `App`,
  which is changing the thing being mapped in a document whose whole premise is
  that nothing moves. The map is read once per pane port by a human deciding a
  design, and a field added before Phase 4 is a field whose class its author
  knows.
