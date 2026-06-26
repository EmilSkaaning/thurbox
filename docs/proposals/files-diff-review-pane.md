# Proposal: File viewer → diff/review pane

**Status:** Draft (design only — not implemented)
**Author:** (proposal)
**Scope:** Turn the read-only file tree (`Ctrl+E` / `F3`) into a
glance-able **diff/review pane** that answers *"what did this agent
change?"* without leaving thurbox.

---

## 1. Motivation

The file viewer today is a read-only directory tree. It walks the
session's worktree(s) + `additional_dirs` (`expected_root_paths`,
`src/ui/file_viewer.rs`), lazily expands directories, supports a `/`
substring search, and opens a file in `$EDITOR` on `Enter`
(`App::file_viewer_expand` → `open_file_in_editor`,
`src/app/key_handlers.rs:1632`). It never shells out to git and shows
nothing about *what changed*.

Meanwhile thurbox's whole value proposition is *multiple agents editing
code in parallel worktrees*. The single most common question a user has
when they switch to a session is **"what did this agent just do?"** —
and answering it means leaving thurbox for `git diff`, a terminal, or an
editor. The data is already computed for an unrelated feature: the info
panel shows `GitStats { files_changed, insertions, deletions, dirty,
ahead, behind }` via `git::worktree_stats` (`src/git/mod.rs:874`). We
surface the *counts* but not the *content*.

**Goal:** make the file viewer default to a diff-first view of the
session's worktree, mark changed files with status glyphs so they float
to the top, and let the user read a syntax-/diff-colored hunk inline.

---

## 2. Grounding: what exists today (verified against the code)

Every claim below was read out of the tree on this branch.

### 2.1 File viewer (`src/ui/file_viewer.rs`)

- `FileNode { path, is_dir, expanded, children: Option<Vec<FileNode>> }`
  (≈ line 25) — lazy tree node.
- `FileViewerState { roots: Vec<FileNode>, selected, search_active,
  search_query, search_cursor }` (≈ line 80) — the persistent state, held
  on `App` as `file_viewer` (`src/app/mod.rs:509`).
- `FlatRow { index_path, depth, label, is_dir, expanded }` (≈ line 72) —
  the flattened render row.
- `rebuild_from_session(&SessionInfo)` (≈ line 290) populates `roots`
  from `expected_root_paths` (worktrees → additional_dirs → cwd fallback).
- `read_dir_sorted` (≈ line 581): dirs-first, dotfiles skipped,
  case-insensitive name sort. **This sort is where status-based float-to-top
  ordering would hook in.**
- `render_file_viewer(frame, area, &FileViewerState, focus) ->
  (Option<ScrollbarGeom>, Vec<RowHitbox>)` (≈ line 616); per-row spans
  built in `build_row_line` (≈ line 749) — **this is where glyphs/colors
  attach.**
- `selected_file_with_root() -> Option<(PathBuf, PathBuf)>` (≈ line 235)
  and `activate() -> Activation::Open(PathBuf)` (≈ line 351) drive opening.
- `enumerate_paths()` (≈ line 542) feeds global search.
- **No syntax highlighting today.** Rows are colored only by search-match
  state.

### 2.2 App wiring

- `show_file_viewer: bool` (`src/app/mod.rs:508`); toggled by
  `act_toggle_file_viewer` (`src/app/key_handlers.rs:1590`), which rebuilds
  for the active session and resizes.
- `InputFocus::FileViewer` (`src/app/mod.rs:466`); part of the focus ring
  (`SessionList → Terminal → FileViewer → TaskList`, `focus_ring` in
  `key_handlers.rs`).
- Render hook: `App::render_file_viewer` (`src/app/view.rs:437`) lazily
  rebuilds via `needs_rebuild_for` and records scrollbar + row click
  targets (`ScrollTarget::FileViewer`, `ClickAction::SelectFileRow`).
- Keybindings: `ToggleFileViewer` (Ctrl+E / F3); scoped nav actions
  `FileViewerDown/Up/Collapse/Expand/Search/NextMatch/PrevMatch` under
  `KeyContext::FileViewer` (`src/session/keybindings.rs:71`).
- Layout: `PanelAreas::file_viewer: Option<Rect>`, rightmost ~20% column,
  only at width ≥ 120 (`src/ui/layout.rs`).

### 2.3 Git plumbing

- `git::worktree_stats(cwd) -> Option<GitStats>` (`src/git/mod.rs:874`).
  **Important nuance:** its file/line counts come from `git diff --numstat
  HEAD` — i.e. **uncommitted working-tree changes vs HEAD**, *not* vs the
  base branch. Only the `ahead/behind` pair (`ahead_behind`, line 854) is
  base-relative, via `rev-list --left-right --count`.
- The base ref is **not persisted anywhere per worktree.** It is *derived
  on demand* by `resolve_sync_base_ref(worktree_path)` (`src/git/mod.rs:709`,
  private): `@{upstream}` → `origin/HEAD` → `origin/main` → `origin/master`,
  else `None`. `ahead_behind` (line 854) does its own inline version of the
  same fallback chain.
- `git_command(host, cwd, args)` (≈ line 14) builds `git -C <cwd> …`
  locally or `ssh <dest> git -C <cwd> …` for `ssh:<host>` backends.
  `run_git_capture(args, cwd)` (line 806) is the local stdout helper
  (**no host variant yet** — see Risk R4).
- `WorktreeInfo { repo_path, worktree_path, branch }`
  (`src/session/mod.rs:47`); `SessionInfo.worktrees: Vec<WorktreeInfo>`
  and `SessionInfo.git_stats: Option<GitStats>` (line 218).
- `worktree_branch`/`base_branch` *are* persisted, but only for an
  `AutomationAction::Spawn` row (schema columns in `storage/tasks.rs`) —
  not on a live `SessionInfo`/`WorktreeInfo`. So at runtime we have the
  worktree's *current* branch but must derive its *base*.

### 2.4 Markdown renderer (`src/ui/markdown.rs`)

- `render_markdown(src: &str) -> Vec<Line<'static>>` (line 20), used by
  `ui/task_detail.rs:129`. Handles headings, emphasis, lists, blockquotes,
  links, fenced code blocks — but **code blocks are merely dimmed, no
  syntax/diff coloring.** Backed by `pulldown_cmark` only; the project
  pulls in **no** `syntect` / `tree-sitter` / `two-face` (verified in
  `Cargo.toml`).

### 2.5 Architecture constraint (hard rule)

`tests/architecture_rules.rs` allows `ui` to import only `session`, `app`,
`fuzzy`, `paths` — **`ui` may not reference `git` in any form** (not even a
fully-qualified `crate::git::…` path; `allowed_path_only` is empty for
`ui`). `git` may import `session`, `paths`, `shell`. Therefore **all git
work happens in `app`/`git`; `ui` only renders pre-computed view-model
data handed to it.** This mirrors the existing `git_stats` flow: `app`
computes off-thread, stows the result on `SessionInfo`, `ui` reads it.

---

## 3. Design

### 3.1 UX

**Default to diff.** When the pane opens (`Ctrl+E`/`F3`) for a session
that has a resolvable base and any changes, it opens in **Changes mode**
instead of the full tree. The pane has two modes, toggled with a new
scoped key (proposed `t` = "tree", rebindable
`Action::FileViewerToggleMode`):

| Mode | Shows |
|------|-------|
| **Changes** (default when changes exist) | A flat, status-sorted list of changed files vs base, each prefixed with a status glyph and `+a/-d` line deltas. |
| **Tree** (today's behavior) | The full worktree tree, now *annotated* with the same status glyphs. |

Falling back: if the base can't be resolved or there are zero changes,
the pane opens in **Tree** mode (today's behavior) with a one-line header
note (`no base branch` / `no changes vs <base>`), so nothing regresses
for non-git or pristine sessions.

**Header line.** Both modes show a compact header:
`Δ <base> · 4 files +120 −18` (base ref short name + rollup, reusing the
`GitStats` numbers). This makes the comparison point explicit — important
because the base is *derived*, not chosen by the user (§3.4).

**Status glyphs** (new `git_glyph`/`git_color` in `ui`, fed pre-computed
status — see §3.3):

| Status | Glyph | Color (theme field) |
|--------|-------|---------------------|
| Modified | `M` | `status_working` (yellow) |
| Added | `A` | `status_idle` (green) |
| Deleted | `D` | `status_blocked` (red) |
| Renamed | `R` | accent |
| Untracked | `?` | muted |

Reuse the existing theme palette (`status_working`/`status_blocked`/
`status_idle`) rather than inventing new theme fields, to keep the patch
small; a follow-up can add dedicated `diff_added`/`diff_removed` fields.

**Float to top.** In Tree mode, `read_dir_sorted` is extended so that,
when status data is present, entries are key-sorted by *(has-changes-desc,
dirs-first, name)* — a changed file/dir sorts above unchanged siblings,
and a directory that *contains* a change is marked and floats up so the
path to a change is visible without hunting. In Changes mode the list is
already only changed files, sorted by status then path.

**Inline hunk preview.** `Enter` (or `FileViewerExpand`) on a changed
file no longer only shells to `$EDITOR`. Instead the **central pane**
shows a read-only, scrollable **diff view** of that file vs base — exactly
the way the tasks panel takes over the central pane with a markdown
preview (`view::render_task_workspace`, `ui/task_detail.rs`). The diff is
rendered with per-line coloring (added = green, removed = red, hunk
header = accent, context = muted). `PageUp`/`PageDown` scroll it (new
`App::diff_preview_scroll`, mirroring `task_preview_scroll`). A second
`Enter` (or a dedicated `o`/`Action::OpenInEditor`-style key) opens the
real file in `$EDITOR` **at the first changed line** (§3.5). `Esc`/`Ctrl+H`
returns to the pane.

**Where the diff renders.** Two options were considered:

- *(A — recommended)* Diff in the **central pane** when the file viewer is
  focused, mirroring how `InputFocus::TaskList` repurposes the central
  pane. Most screen real estate, consistent with an existing pattern, and
  keeps the narrow 20% file-viewer column for the file list.
- *(B)* Diff **inside** the 20% file-viewer column under the list. Rejected
  for v1: 20% width can't show a useful unified diff.

### 3.2 Affected modules (per CLAUDE.md architecture)

```text
session  + DiffStatus enum, FileChange struct, FileDiff struct (pure data)
git      + changed_files(cwd, base) , file_diff(cwd, base, path),
           resolve_base_ref (promote resolve_sync_base_ref to pub)
app      + diff cache (BackgroundTask), base-ref resolution, mode state,
           central-pane diff render dispatch, open-at-line
ui       + render the diff view + status glyphs from PRE-COMPUTED data only
```

- **`session/`** (pure data, no deps): add the view-model types so both
  `git` (producer) and `ui` (consumer) can name them without crossing the
  `ui ✗ git` boundary — same trick `GitStats` uses (it lives in `session`,
  produced by `git`, rendered by `ui`).
  - `enum DiffStatus { Modified, Added, Deleted, Renamed, Untracked }`
  - `struct FileChange { path: PathBuf, status: DiffStatus, insertions:
    usize, deletions: usize }`
  - `struct WorktreeDiff { base_ref: String, files: Vec<FileChange> }`
  - `struct FileDiff { path, base_ref, lines: Vec<DiffLine> }` where
    `DiffLine { kind: DiffLineKind, text: String }` and
    `DiffLineKind { Added, Removed, Context, Hunk, Meta }`. Keeping the
    parsed line model in `session` means `ui` colors by `kind` and never
    parses a diff (and never imports git).
- **`git/`**: add producers.
  - `pub fn changed_files(cwd: &Path, base: &str) -> Vec<FileChange>` —
    `git diff --numstat -z <base>` + `git diff --name-status -z <base>`
    (or one `--numstat --name-status` pass), parsed into `FileChange`.
    Untracked files come from `git status --porcelain` (the existing
    `worktree_is_dirty` already runs this). Reuse `parse_numstat`'s shape
    (line 826).
  - `pub fn file_diff(cwd: &Path, base: &str, path: &Path) -> FileDiff` —
    `git diff <base> -- <path>`, parsed line-by-line into `DiffLine`s.
  - `pub fn resolve_base_ref(cwd: &Path) -> Option<String>` — **promote the
    existing private `resolve_sync_base_ref` to public** (rename for the
    broader use), so diff and sync share one base-resolution policy and can
    never drift. The Ctrl+S sync path keeps calling it.
  - Add `*_on(host, …)` variants later (Risk R4); v1 may scope diff to
    local sessions.
- **`app/`**: the bridge (allowed to import everything).
  - New `BackgroundTask`-style cache `diff: BackgroundTask<(SessionId,
    Option<WorktreeDiff>)>` next to the existing `git_stats` task
    (`src/app/mod.rs:524`), refreshed on the same cadence
    (`GIT_REFRESH_TICKS ≈ 500`) and on session switch / file-viewer open.
    Store the resolved `WorktreeDiff` on `App` (or on `SessionInfo`
    alongside `git_stats`).
  - Lazily compute `FileDiff` for the *selected* changed file only (cheap;
    one `git diff -- <path>`), cached by `(session, path, base, mtime)`.
  - Mode state: `file_viewer_mode: FileViewerMode { Changes, Tree }` on
    `App` (or inside `FileViewerState`).
  - Dispatch: when `InputFocus::FileViewer` and the selection is a changed
    file, `view.rs` renders the diff in the central pane.
- **`ui/`**: pure rendering only.
  - `file_viewer::build_row_line` gains an optional `&[FileChange]` (or a
    `path → DiffStatus` map) param to prefix glyphs/colors. Defaulted/None
    keeps current behavior.
  - New `ui/diff_view.rs`: `render_diff(frame, area, &FileDiff, focus,
    scroll)` — colors each `DiffLine` by `kind`. Pure; receives only
    `session`-owned data. (We deliberately do **not** route the diff
    through `render_markdown`: it can't color +/- lines — §2.4 — and a
    purpose-built renderer is simpler and faster than faking a fenced code
    block.)

### 3.3 Data flow (no rule violations)

```text
tick (≈ every 5s) / session-switch / Ctrl+E
  └─ app: spawn off-thread  git::changed_files(wt, base)         ─┐
                            base = git::resolve_base_ref(wt)      │  git layer
  └─ result (SessionId, WorktreeDiff) → cached on App/SessionInfo ┘
selection moves to a changed file
  └─ app: git::file_diff(wt, base, path) (lazy, cached)
view(model)
  └─ ui::file_viewer::render_file_viewer(.., Some(&changes))   ← glyphs
  └─ ui::diff_view::render_diff(.., &file_diff, ..)            ← central pane
```

`ui` only ever sees `session`-owned `WorktreeDiff`/`FileDiff`; `git`
calls happen exclusively on `app`/`git` threads. This is byte-for-byte
the `git_stats` pattern that already passes `architecture_rules.rs`.

### 3.4 The base-branch question (the crux)

The task framing assumes "base branch is tracked per worktree." **It is
not** (§2.3): at runtime we only have `WorktreeInfo.branch` (the worktree's
*own* branch), and the base is *derived* by `resolve_sync_base_ref`. Three
consequences:

1. **v1 derives the base** the same way sync does, and shows it in the
   header so the user knows the comparison point. This is zero new
   persistence and consistent with Ctrl+S.
2. **Edge case — no upstream/origin:** a worktree off a *local* base
   branch with no remote resolves to `None`. v1 falls back to Tree mode
   with a `no base branch` note (and could offer a future "diff vs HEAD"
   secondary mode reusing the existing `--numstat HEAD` path).
3. **Future — persist the base.** The clean fix is to store the base on
   the worktree at creation: `create_worktree(repo, new_branch,
   base_branch)` *already receives* the base (`src/git/mod.rs:229`) — it's
   just dropped after the `git worktree add`. A follow-up adds a
   `base_branch` column to the session/worktree persistence and threads it
   into `WorktreeInfo`, making the diff base exact rather than derived.
   Out of scope for v1 (schema migration); called out as the principled
   end-state.

### 3.5 Syntax-awareness and open-at-line

- **Diff coloring** (v1): per-line by `DiffLineKind` — green/red/accent/
  muted. No language grammar needed; ships in the first cut.
- **Syntax highlighting of code within the diff** (future): the project
  has *no* highlighter dependency today (§2.4). Adding `syntect` (or
  `two-face`) pulls in a sizable dep + theme handling and touches
  `cargo deny` policy. Proposed as an opt-in follow-up gated behind a
  `[features] syntax_highlight` flag, applied as a second styling pass over
  the `Context`/`Added`/`Removed` line bodies. Explicitly **not** in v1.
- **Open at a specific line:** `git diff` hunk headers (`@@ -a,b +c,d @@`)
  give the new-file line of the first change. `open_file_in_editor` already
  shells to `$EDITOR`; extend it to pass a `+<line>` argument (e.g.
  `${EDITOR} +<line> <file>` — vim/nvim/nano/VS Code-`--goto` styles
  differ, so gate on a small editor-flavor map or honor a
  `THURBOX_EDITOR_LINE_FMT`). Modest, isolated change in `app`.

---

## 4. Phased implementation plan

**Phase 0 — git producers + data types (no UI).**
Add `DiffStatus`/`FileChange`/`WorktreeDiff`/`FileDiff`/`DiffLine` to
`session/`. Add `git::changed_files`, `git::file_diff`, promote
`resolve_base_ref` to public. Unit-test the parsers with fixture strings
(mirrors the existing `parse_numstat` tests at `git/mod.rs:~895`). No
behavior change yet. *Lowest risk; fully testable headless.*

**Phase 1 — status glyphs in Tree mode.**
Thread a `path → DiffStatus` map from the new `app` diff cache into
`build_row_line`; render glyphs + colors. Extend `read_dir_sorted` to
float changed entries. The pane still opens as a tree — purely additive.

**Phase 2 — Changes mode + header + default.**
Add `FileViewerMode`, the header line, the `t` toggle, and make `Ctrl+E`
default to Changes when a base resolves and changes exist (else Tree).
Update the F1 help + CLAUDE.md "Architecture / file viewer" notes.

**Phase 3 — inline diff in the central pane.**
`ui/diff_view.rs` + `view.rs` dispatch when a changed file is selected.
Lazy `file_diff` cache, `diff_preview_scroll`, `PageUp`/`PageDown`.
insta snapshot of a small fixed diff (the acceptance harness renders to a
`TestBackend`; see `src/app/acceptance.rs`).

**Phase 4 — open-at-line.**
Extend `open_file_in_editor` with the first-changed-line jump.

**Phase 5 (follow-up, separate PR) — persisted base + syntax highlight.**
Persist `base_branch` on the worktree (schema migration) for an exact
base; optional `syntect` behind `[features] syntax_highlight`; remote
`*_on(host)` diff variants.

Phases 0–4 are the proposal's v1. Each phase is independently shippable
and leaves the pane working.

---

## 5. Testing

- **Parsers (Phase 0):** unit tests on `changed_files`/`file_diff` over
  canned `git diff` output, alongside the existing `parse_numstat` tests.
- **Glyph/sort (Phase 1):** pure tests on the extended `read_dir_sorted`
  comparator and `build_row_line` span output (no git needed — feed a
  synthetic status map).
- **Acceptance (Phases 2–3):** drive the in-process `Harness`
  (`src/app/acceptance.rs`) — open the pane, assert mode/header state, and
  insta-snapshot the diff render for a fixed `FileDiff` fixture (avoid live
  git in the snapshot to keep it deterministic).
- **Architecture:** `cargo test --test architecture_rules` must stay green
  — proves `ui` never gained a `git` reference.

---

## 6. Risks & mitigations

- **R1 — base ref ambiguity.** Derived base can surprise (e.g. a stacked
  branch whose `@{upstream}` is another feature branch). *Mitigation:*
  always show the resolved base in the header; Phase 5 persists an exact
  base. Reusing one `resolve_base_ref` for both diff and sync keeps the
  surprise consistent with existing Ctrl+S behavior.
- **R2 — performance.** `git diff` per tick on a large worktree could
  stall. *Mitigation:* run in the existing off-thread `BackgroundTask` on
  the `GIT_REFRESH_TICKS` cadence (never on the render path), exactly like
  `git_stats`; lazy `file_diff` only for the selected file; cap rendered
  diff lines (a `DIFF_LINE_CAP`, mirroring global search's
  `CONTENT_LINE_CAP`).
- **R3 — architecture regression.** Easy to accidentally `use crate::git`
  from `ui`. *Mitigation:* all diff types live in `session`; the
  `architecture_rules` test fails the build if violated.
- **R4 — remote (`ssh:<host>`) sessions.** `run_git_capture` has no host
  variant, so v1 diff is **local-only**; remote sessions fall back to Tree
  mode with a note. *Mitigation:* Phase 5 adds `changed_files_on` /
  `file_diff_on` using `git_command(host, …)` (which already supports SSH).
- **R5 — narrow terminals.** The pane only exists at width ≥ 120, and the
  central-pane diff needs room. *Mitigation:* reuse the existing layout
  gating; below threshold the feature is simply unavailable (as the file
  viewer already is).
- **R6 — binary / huge files.** `git diff` on a binary yields `Binary
  files differ`. *Mitigation:* detect and render a one-line placeholder;
  `parse_numstat` already treats binaries as `-\t-` (line 826).
- **R7 — scope creep into a full review tool** (comments, staging,
  approvals). *Mitigation:* v1 is strictly read-only "what changed";
  anything write-side is explicitly out of scope.

---

## 7. Out of scope (v1)

Staging/committing from the pane, inline review comments, side-by-side
(split) diff, word-level intra-line diff, syntax highlighting of code
bodies, remote-session diff, and persisting an exact base branch — all
deferred to follow-ups (§4 Phase 5).
