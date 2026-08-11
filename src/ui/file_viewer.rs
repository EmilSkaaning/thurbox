//! Right-side file viewer panel.
//!
//! Displays an expandable tree of every worktree and additional directory
//! associated with the active session. Selection is flat (one visible row
//! at a time), and callers drive navigation through [`FileViewerState`]
//! helpers. File I/O (reading directory entries) is performed lazily when a
//! folder is expanded.
//!
//! Like [`super::automations_panel`], the tree renders
//! through the **view tree**: [`file_tree`] describes the rows and
//! [`super::plugin_pane::render_tree`] paints them. The split is sharper here
//! than in the tasks pane, and that is the point of this port:
//!
//! - [`file_tree`] carries **no geometry at all** — not even a window. It takes
//!   every row and the index of the one the cursor is on, and the *renderer*
//!   scrolls the list to that row from the height it was given
//!   ([`ViewNode::List`]'s selected child). So a plugin reproducing this pane
//!   scrolls correctly without ever learning a dimension, which is what the
//!   tasks port recorded as the gap blocking every remaining pane.
//! - The pane still resolves that same window itself, for its click hitboxes and
//!   its scrollbar. Both go through one shared windowing helper, so the rows the
//!   user can click are the rows the renderer drew. The scroll **track** is the
//!   same arrangement one column over: the tree declares it, the renderer draws
//!   it, and the pane re-derives the reserved column only to record it as a drag
//!   target.
//!
//! The **search bar** below the tree is chrome, not tree, and is deliberately
//! outside this split: it needs a bordered sub-block, a cursor cell, and a
//! bottom-anchored region, none of which the view tree can describe. The
//! search's *effect on the rows* is in [`FileRow::matched`].

use std::path::{Path, PathBuf};

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
// Only the retained pre-port oracle (`legacy_line` and friends) still assembles
// styled spans by hand.
#[cfg(test)]
use ratatui::style::{Modifier, Style as LegacyStyle};

use super::scrollbar::{self, ScrollbarGeom};
use super::{focus_block, theme::Theme, FocusLevel};
use crate::session::motion::FrameTable;
use crate::session::view_tree::{StyleToken, TextStyle, ViewNode};
use crate::session::SessionInfo;

/// One node in the tree. `children = None` means "not yet expanded"; an
/// empty `Some(vec![])` means "expanded but empty".
pub struct FileNode {
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Option<Vec<FileNode>>,
}

impl FileNode {
    fn new_dir(path: PathBuf) -> Self {
        Self {
            path,
            is_dir: true,
            expanded: false,
            children: None,
        }
    }
}

/// Paths that should appear as roots for the given session: every worktree,
/// then every additional dir, falling back to `cwd` if both are empty.
fn expected_root_paths(info: &SessionInfo) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = info
        .worktrees
        .iter()
        .map(|w| w.worktree_path.clone())
        .chain(info.additional_dirs.iter().cloned())
        .collect();
    if out.is_empty() {
        if let Some(cwd) = &info.cwd {
            out.push(cwd.clone());
        }
    }
    out
}

/// Result of activating the currently-selected row.
pub enum Activation {
    /// A file was activated — caller should open it in the editor.
    Open(PathBuf),
    /// A directory was toggled (expanded or collapsed).
    Toggled,
    /// Nothing was done (empty tree, out-of-bounds, etc.).
    NoOp,
}

/// One flattened visible row. Depth drives indentation; `index_path` is the
/// traversal path into `roots` (sequence of child indices).
struct FlatRow {
    index_path: Vec<usize>,
    depth: usize,
    label: String,
    is_dir: bool,
    expanded: bool,
}

/// One row as the pane's view tree describes it.
///
/// Distinct from the pane's internal flattened row because this is the
/// *drawable* one: it has dropped the traversal path nothing visual needs and
/// gained the running search's verdict, so building the tree from it needs no
/// query, no matcher and no geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    /// The node's basename — what the row displays.
    pub name: String,
    /// Nesting depth; a root is 0. Two spaces of indent per level.
    pub depth: usize,
    /// Whether it is a directory.
    pub is_dir: bool,
    /// Whether it is expanded.
    pub expanded: bool,
    /// Whether the running search matched this row's name.
    ///
    /// `true` when no search is running, so an unsearched tree draws in its
    /// ordinary colours — the pane never has to ask whether a search exists.
    pub matched: bool,
}

pub struct FileViewerState {
    roots: Vec<FileNode>,
    selected: usize,
    pub search_active: bool,
    pub search_query: String,
    pub search_cursor: usize,
}

/// Maximum nodes traversed per search to keep typing responsive.
const SEARCH_NODE_LIMIT: usize = 5000;
/// Maximum directory depth traversed per search.
const SEARCH_DEPTH_LIMIT: usize = 6;

impl FileViewerState {
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            selected: 0,
            search_active: false,
            search_query: String::new(),
            search_cursor: 0,
        }
    }

    /// Enter search mode. Preserves current selection.
    pub fn start_search(&mut self) {
        self.search_active = true;
        self.search_query.clear();
        self.search_cursor = 0;
    }

    /// Exit search mode, keeping selection.
    pub fn end_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        self.search_cursor = 0;
    }

    /// Append a char to the query and jump selection to the first match.
    pub fn search_push(&mut self, c: char) {
        self.search_query.push(c);
        self.search_cursor = self.search_query.chars().count();
        self.expand_for_search();
        self.jump_to_first_match();
    }

    /// Backspace in the query.
    pub fn search_pop(&mut self) {
        self.search_query.pop();
        self.search_cursor = self.search_query.chars().count();
        if !self.search_query.is_empty() {
            self.expand_for_search();
        }
        self.jump_to_first_match();
    }

    /// Count flat rows currently matching the query.
    pub fn match_count(&self) -> usize {
        if self.search_query.is_empty() {
            return 0;
        }
        let q = self.search_query.to_lowercase();
        self.flatten()
            .iter()
            .filter(|r| r.label.to_lowercase().contains(&q))
            .count()
    }

    /// 1-based index of the currently selected row among matches, or 0 if
    /// selection is not on a match or query is empty.
    pub fn current_match_index(&self) -> usize {
        if self.search_query.is_empty() {
            return 0;
        }
        let q = self.search_query.to_lowercase();
        let rows = self.flatten();
        let mut idx = 0;
        for (i, row) in rows.iter().enumerate() {
            if row.label.to_lowercase().contains(&q) {
                idx += 1;
                if i == self.selected {
                    return idx;
                }
            }
        }
        0
    }

    /// Walk roots (bounded) and auto-expand ancestors of any node whose name
    /// matches the current query. Reads directories lazily.
    fn expand_for_search(&mut self) {
        if self.search_query.is_empty() {
            return;
        }
        let q = self.search_query.to_lowercase();
        let mut budget = SEARCH_NODE_LIMIT;
        for root in &mut self.roots {
            expand_matches(root, &q, 0, &mut budget);
            if budget == 0 {
                break;
            }
        }
    }

    /// Cycle to the next match (wrapping), starting after the current selection.
    pub fn next_match(&mut self) {
        self.step_match(true);
    }

    /// Cycle to the previous match (wrapping), starting before the current selection.
    pub fn prev_match(&mut self) {
        self.step_match(false);
    }

    fn step_match(&mut self, forward: bool) {
        if self.search_query.is_empty() {
            return;
        }
        let rows = self.flatten();
        if rows.is_empty() {
            return;
        }
        let q = self.search_query.to_lowercase();
        let n = rows.len();
        let start = self.selected;
        for offset in 1..=n {
            let i = if forward {
                (start + offset) % n
            } else {
                (start + n - offset) % n
            };
            if rows[i].label.to_lowercase().contains(&q) {
                self.selected = i;
                return;
            }
        }
    }

    fn jump_to_first_match(&mut self) {
        if self.search_query.is_empty() {
            return;
        }
        let rows = self.flatten();
        let q = self.search_query.to_lowercase();
        if let Some((i, _)) = rows
            .iter()
            .enumerate()
            .find(|(_, r)| r.label.to_lowercase().contains(&q))
        {
            self.selected = i;
        }
    }

    /// Return the selected file path along with its root worktree path.
    /// Returns `None` if selection is a directory or out of bounds.
    pub fn selected_file_with_root(&self) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
        let rows = self.flatten();
        let row = rows.get(self.selected)?;
        let index_path = &row.index_path;
        let root_idx = *index_path.first()?;
        let root = self.roots.get(root_idx)?;
        let mut node = root;
        for idx in &index_path[1..] {
            node = node.children.as_ref()?.get(*idx)?;
        }
        if node.is_dir {
            return None;
        }
        Some((node.path.clone(), root.path.clone()))
    }

    /// Expand the ancestors of `target` and move the selection onto it. Used by
    /// the global search to jump to a file/dir result. Best-effort: silently
    /// no-ops if `target` isn't under any current root.
    pub fn reveal_path(&mut self, target: &Path) {
        for root_idx in 0..self.roots.len() {
            let root_path = self.roots[root_idx].path.clone();
            let Ok(rel) = target.strip_prefix(&root_path) else {
                continue;
            };
            expand_ancestors(&mut self.roots[root_idx], &root_path, rel);
            self.select_target(target);
            return;
        }
    }

    /// Locate `target` in the flattened view and move the selection onto it.
    fn select_target(&mut self, target: &Path) {
        if let Some(row) = self.flatten().iter().position(|r| {
            self.path_for_index(&r.index_path)
                .map(|p| p == target)
                .unwrap_or(false)
        }) {
            self.selected = row;
        }
    }

    /// Resolve a flat row's `index_path` back to its filesystem path.
    fn path_for_index(&self, index_path: &[usize]) -> Option<PathBuf> {
        let root_idx = *index_path.first()?;
        let mut node = self.roots.get(root_idx)?;
        for idx in &index_path[1..] {
            node = node.children.as_ref()?.get(*idx)?;
        }
        Some(node.path.clone())
    }

    /// Rebuild roots from the active session's worktrees + additional_dirs.
    /// Selection is reset to 0.
    pub fn rebuild_from_session(&mut self, info: &SessionInfo) {
        self.roots = expected_root_paths(info)
            .into_iter()
            .map(|path| {
                // Auto-expand the top level of each root so the viewer opens to a
                // populated tree instead of a single collapsed folder that reads
                // as empty. Only the first level is read (no recursion) to keep
                // the rebuild cheap.
                let mut node = FileNode::new_dir(path);
                node.children = Some(read_dir_sorted(&node.path));
                node.expanded = true;
                node
            })
            .collect();
        self.selected = 0;
    }

    pub fn clear(&mut self) {
        self.roots.clear();
        self.selected = 0;
    }

    /// Return true if the current roots don't match the session's expected roots.
    /// Used by the render layer to rebuild lazily when the active session changes.
    pub fn needs_rebuild_for(&self, info: &SessionInfo) -> bool {
        let expected = expected_root_paths(info);
        if self.roots.len() != expected.len() {
            return true;
        }
        self.roots
            .iter()
            .zip(expected.iter())
            .any(|(root, exp)| root.path != *exp)
    }

    /// The drawable rows of the tree, in the order the pane lists them.
    ///
    /// The one traversal of the tree that both the pane and the published
    /// snapshot use, so the rows a plugin reads are the rows the pane draws.
    /// Reads nothing from disk: the tree it walks is whatever the user has
    /// already expanded.
    pub fn rows(&self) -> Vec<FileRow> {
        // A search that is not running, or has an empty query, matches every
        // row — which is how an unsearched tree keeps its ordinary colours.
        let query = (self.search_active && !self.search_query.is_empty())
            .then(|| self.search_query.to_lowercase());
        self.flatten()
            .into_iter()
            .map(|row| FileRow {
                matched: query
                    .as_deref()
                    .map(|q| row.label.to_lowercase().contains(q))
                    .unwrap_or(true),
                name: row.label,
                depth: row.depth,
                is_dir: row.is_dir,
                expanded: row.expanded,
            })
            .collect()
    }

    /// Which row the cursor is on.
    ///
    /// Not clamped to the row count: an index past the last row highlights
    /// nothing and windows to the end of the list, which is what this pane has
    /// always done.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    fn flatten(&self) -> Vec<FlatRow> {
        let mut out = Vec::new();
        for (i, root) in self.roots.iter().enumerate() {
            push_flat(root, vec![i], 0, &mut out, true);
        }
        out
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.flatten().len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        let next = (self.selected as i32 + delta).clamp(0, len as i32 - 1);
        self.selected = next as usize;
    }

    /// Select the row at `index`, clamped to the current row count. Used by the
    /// scrollbar drag to jump the selection to a mapped position.
    pub fn select_index(&mut self, index: usize) {
        let len = self.flatten().len();
        self.selected = if len == 0 { 0 } else { index.min(len - 1) };
    }

    /// Activate the current row: toggle directory expansion or return a file to open.
    pub fn activate(&mut self) -> Activation {
        let rows = self.flatten();
        let Some(row) = rows.get(self.selected) else {
            return Activation::NoOp;
        };
        let index_path = row.index_path.clone();
        let Some(node) = traverse_mut(&mut self.roots, &index_path) else {
            return Activation::NoOp;
        };
        if node.is_dir {
            if node.expanded {
                node.expanded = false;
            } else {
                if node.children.is_none() {
                    node.children = Some(read_dir_sorted(&node.path));
                }
                node.expanded = true;
            }
            Activation::Toggled
        } else {
            Activation::Open(node.path.clone())
        }
    }

    /// Collapse the selected directory (or jump up to parent if selection is a file or a closed dir).
    pub fn collapse(&mut self) {
        let rows = self.flatten();
        let Some(row) = rows.get(self.selected) else {
            return;
        };
        let index_path = row.index_path.clone();
        if let Some(node) = traverse_mut(&mut self.roots, &index_path) {
            if node.is_dir && node.expanded {
                node.expanded = false;
                return;
            }
        }
        if index_path.len() > 1 {
            let parent_path = &index_path[..index_path.len() - 1];
            let new_rows = self.flatten();
            if let Some((i, _)) = new_rows
                .iter()
                .enumerate()
                .find(|(_, r)| r.index_path.as_slice() == parent_path)
            {
                self.selected = i;
            }
        }
    }
}

impl Default for FileViewerState {
    fn default() -> Self {
        Self::new()
    }
}

fn push_flat(
    node: &FileNode,
    index_path: Vec<usize>,
    depth: usize,
    out: &mut Vec<FlatRow>,
    is_root: bool,
) {
    let label = if is_root {
        short_root_label(&node.path)
    } else {
        node.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| node.path.to_string_lossy().into_owned())
    };
    out.push(FlatRow {
        index_path: index_path.clone(),
        depth,
        label,
        is_dir: node.is_dir,
        expanded: node.expanded,
    });
    if node.is_dir && node.expanded {
        if let Some(children) = &node.children {
            for (i, child) in children.iter().enumerate() {
                let mut ip = index_path.clone();
                ip.push(i);
                push_flat(child, ip, depth + 1, out, false);
            }
        }
    }
}

fn traverse_mut<'a>(roots: &'a mut [FileNode], index_path: &[usize]) -> Option<&'a mut FileNode> {
    let (first, rest) = index_path.split_first()?;
    let mut node = roots.get_mut(*first)?;
    for idx in rest {
        let children = node.children.as_mut()?;
        node = children.get_mut(*idx)?;
    }
    Some(node)
}

/// Expand `root` and every directory along `rel` (the target's path relative to
/// `root_path`), reading child directories on demand. Stops early if a level is
/// missing. Used by [`FileViewerState::reveal_path`].
fn expand_ancestors(root: &mut FileNode, root_path: &Path, rel: &Path) {
    let mut node = root;
    let mut current = root_path.to_path_buf();
    node.expanded = true;
    for comp in rel.components() {
        current.push(comp);
        if node.children.is_none() {
            node.children = Some(read_dir_sorted(&node.path));
        }
        let Some(children) = node.children.as_mut() else {
            break;
        };
        let Some(pos) = children.iter().position(|c| c.path == current) else {
            break;
        };
        node = &mut children[pos];
        if node.is_dir {
            node.expanded = true;
        }
    }
}

/// Recursively walk `node` and auto-expand directories that contain any
/// descendant whose name matches `q_lc`. Returns true if a match was found at
/// or below `node`. Reads child directories on demand.
fn expand_matches(node: &mut FileNode, q_lc: &str, depth: usize, budget: &mut usize) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;

    let self_matches = node
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase().contains(q_lc))
        .unwrap_or(false);

    if !node.is_dir || depth >= SEARCH_DEPTH_LIMIT {
        return self_matches;
    }

    // Lazily load children on first search traversal.
    if node.children.is_none() {
        node.children = Some(read_dir_sorted(&node.path));
    }

    let child_match = expand_matching_children(node, q_lc, depth, budget);
    if child_match {
        node.expanded = true;
    }
    self_matches || child_match
}

/// Recurse into `node`'s loaded children, returning true if any descendant
/// matches `q_lc`. Honors the shared `budget`. Split out of [`expand_matches`].
fn expand_matching_children(
    node: &mut FileNode,
    q_lc: &str,
    depth: usize,
    budget: &mut usize,
) -> bool {
    let Some(children) = node.children.as_mut() else {
        return false;
    };
    let mut child_match = false;
    for child in children.iter_mut() {
        if expand_matches(child, q_lc, depth + 1, budget) {
            child_match = true;
        }
        if *budget == 0 {
            break;
        }
    }
    child_match
}

fn short_root_label(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Enumerate `(root, path, name)` triples under a session's roots for the global
/// search Files group: a bounded walk (same node/depth limits as the in-viewer
/// search) so a huge tree can't stall the search strip. `name` is the file/dir
/// basename used for matching + display.
pub fn enumerate_paths(info: &SessionInfo) -> Vec<(PathBuf, PathBuf, String)> {
    let mut out: Vec<(PathBuf, PathBuf, String)> = Vec::new();
    let mut budget = SEARCH_NODE_LIMIT;
    for root in expected_root_paths(info) {
        walk_paths(&root, &root, 0, &mut budget, &mut out);
        if budget == 0 {
            break;
        }
    }
    out
}

fn walk_paths(
    root: &Path,
    dir: &Path,
    depth: usize,
    budget: &mut usize,
    out: &mut Vec<(PathBuf, PathBuf, String)>,
) {
    if depth > SEARCH_DEPTH_LIMIT || *budget == 0 {
        return;
    }
    for node in read_dir_sorted(dir) {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let name = node
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push((root.to_path_buf(), node.path.clone(), name));
        if node.is_dir {
            walk_paths(root, &node.path, depth + 1, budget, out);
        }
    }
}

fn read_dir_sorted(path: &Path) -> Vec<FileNode> {
    let mut entries: Vec<(PathBuf, bool, String)> = match std::fs::read_dir(path) {
        Ok(iter) => iter
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    return None;
                }
                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                Some((e.path(), is_dir, name))
            })
            .collect(),
        Err(_) => return Vec::new(),
    };
    // Dirs first, then files; each sorted by name.
    entries.sort_by(|a, b| match (a.1, b.1) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.2.to_lowercase().cmp(&b.2.to_lowercase()),
    });
    entries
        .into_iter()
        .map(|(p, is_dir, _)| FileNode {
            path: p,
            is_dir,
            expanded: false,
            children: None,
        })
        .collect()
}

/// Render the file-tree viewer. Returns the scrollbar geometry when the tree
/// overflows the visible area (so the caller can record it as a drag target),
/// else `None`.
pub fn render_file_viewer(
    frame: &mut Frame,
    area: Rect,
    state: &FileViewerState,
    focus: FocusLevel,
) -> (Option<ScrollbarGeom>, Vec<super::RowHitbox>) {
    let inner = render_chrome(frame, area, state, focus);

    if inner.height == 0 || inner.width == 0 {
        return (None, Vec::new());
    }

    if state.roots.is_empty() {
        // The empty state is part of the pane's tree, so a plugin reproducing the
        // pane reproduces this line too rather than only the populated case.
        paint(
            &file_tree(&[], None, Theme::nerd_font_enabled()),
            inner,
            frame,
        );
        return (None, Vec::new());
    }

    render_rows(frame, inner, state)
}

/// Paint a view tree into `area` through the shared renderer.
///
/// A fresh frame table every paint: nothing in this pane animates, and a motion
/// the kernel is not driving draws frame 0 anyway.
fn paint(tree: &ViewNode, area: Rect, frame: &mut Frame) {
    super::plugin_pane::render_tree(
        tree,
        area,
        &super::theme::current(),
        &FrameTable::default(),
        frame.buffer_mut(),
    );
}

/// Lay out the viewer chrome — the bordered `Files` block plus the optional
/// search bar below it — and return the inner list area.
fn render_chrome(
    frame: &mut Frame,
    area: Rect,
    state: &FileViewerState,
    focus: FocusLevel,
) -> Rect {
    use ratatui::layout::{Constraint, Direction, Layout};

    let search_visible = state.search_active || !state.search_query.is_empty();
    let constraints: Vec<Constraint> = if search_visible {
        vec![Constraint::Min(0), Constraint::Length(3)]
    } else {
        vec![Constraint::Min(0)]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    let list_outer = chunks[0];

    let block = focus_block(" Files ", focus);
    let inner = block.inner(list_outer);
    frame.render_widget(block, list_outer);

    if search_visible {
        render_search_bar(
            frame,
            chunks[1],
            &state.search_query,
            state.search_active,
            state.search_cursor,
            state.current_match_index(),
            state.match_count(),
        );
    }

    inner
}

/// Render the tree rows into `list_area` (with a scrollbar when they overflow).
/// Returns the scrollbar geometry and one hitbox per visible row.
fn render_rows(
    frame: &mut Frame,
    list_area: Rect,
    state: &FileViewerState,
) -> (Option<ScrollbarGeom>, Vec<super::RowHitbox>) {
    let rows = state.rows();
    let height = list_area.height as usize;
    if height == 0 {
        return (None, Vec::new());
    }

    // The tree carries *every* row, the cursor's index, and the fact that this
    // pane wants a scroll track — so the renderer reserves the rightmost column,
    // draws the thumb, and resolves the window from what is left. This second
    // reservation is for the numbers only the pane can answer: which rows a click
    // may land on, and the geometry the app records as a drag target. Both go
    // through the same helpers the paint does, so the column a user can drag and
    // the rows a user can click cannot drift from what was drawn.
    let (rows_area, track) = scrollbar::reserve_track(list_area, rows.len(), height);
    let (start, end) = visible_window(rows.len(), state.selected, rows_area.height as usize);

    paint(
        &file_tree(
            &rows,
            Some(state.selected),
            super::theme::current().nerd_font_enabled,
        ),
        list_area,
        frame,
    );

    // One hitbox per visible tree row; `rows_area` already excludes the
    // reserved scrollbar column, so row clicks never overlap the track.
    let hitboxes = super::windowed_row_hitboxes(rows_area, start, end);

    let geom = track.and_then(|track| scrollbar::geom_for(track, rows.len(), height));
    (geom, hitboxes)
}

/// Build the pane's view tree from every row plus the cursor's index.
///
/// Carries no geometry: no row is fitted to a width and no row is windowed away,
/// because the list node's selected child is what lets the *kernel* scroll it to
/// the cursor from the height it has. A populated tree also *declares* that it
/// wants a scroll track, so the renderer reserves the rightmost column and draws
/// the thumb — the pane still resolves the same reservation as numbers, for the
/// hitboxes and the drag target, but nothing draws it twice. `nerd_font` is a
/// parameter rather than a global read so the tree is a pure function of what it
/// is told — which is also what lets both glyph sets be tested without a theme
/// switch.
///
/// `selected` is not required to be in range: an index past the last row
/// highlights nothing and windows to the end, which is what this pane has always
/// done with a stale cursor.
pub fn file_tree(rows: &[FileRow], selected: Option<usize>, nerd_font: bool) -> ViewNode {
    if rows.is_empty() {
        return ViewNode::list(vec![ViewNode::token("No folders", StyleToken::Muted)]);
    }
    ViewNode::scrolling_list(
        rows.iter()
            .enumerate()
            .map(|(i, row)| row_node(row, Some(i) == selected, nerd_font))
            .collect(),
        selected,
    )
}

/// One row: `<indent><marker><name>`, as two runs so the marker and the name can
/// be styled apart.
fn row_node(row: &FileRow, selected: bool, nerd_font: bool) -> ViewNode {
    let indent = "  ".repeat(row.depth);
    let prefix = format!(
        "{indent}{}",
        row_marker(row.is_dir, row.expanded, nerd_font)
    );
    ViewNode::Line(vec![
        ViewNode::styled(prefix, prefix_style(selected)),
        ViewNode::styled(&row.name, name_style(row, selected)),
    ])
}

/// The glyph in front of a row: a disclosure triangle, or a nerd-font folder or
/// file when the user has those enabled.
fn row_marker(is_dir: bool, expanded: bool, nerd_font: bool) -> &'static str {
    match (is_dir, expanded, nerd_font) {
        (true, true, false) => "▾ ",
        (true, false, false) => "▸ ",
        (false, _, false) => "  ",
        (true, true, true) => "\u{f07c} ",
        (true, false, true) => "\u{f07b} ",
        (false, _, true) => "\u{f15b} ",
    }
}

/// The indent and marker: muted, or the selection's appearance — and unlike the
/// name, never bold.
fn prefix_style(selected: bool) -> TextStyle {
    if selected {
        TextStyle {
            selected: true,
            ..TextStyle::default()
        }
    } else {
        TextStyle {
            token: Some(StyleToken::Muted),
            ..TextStyle::default()
        }
    }
}

/// The row's name. Selection wins over everything; otherwise a row a running
/// search excluded recedes to muted, a matched directory takes the accent and
/// bold, and a matched file is the theme's primary text.
fn name_style(row: &FileRow, selected: bool) -> TextStyle {
    if selected {
        return TextStyle {
            selected: true,
            bold: true,
            ..TextStyle::default()
        };
    }
    if !row.matched {
        return TextStyle {
            token: Some(StyleToken::Muted),
            ..TextStyle::default()
        };
    }
    if row.is_dir {
        return TextStyle {
            token: Some(StyleToken::Accent),
            bold: true,
            ..TextStyle::default()
        };
    }
    // No token: the theme's primary foreground, which is what a file has always
    // been drawn in.
    TextStyle::default()
}

fn render_search_bar(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    is_active: bool,
    cursor: usize,
    current: usize,
    total: usize,
) {
    use ratatui::widgets::{Block, Borders};

    let style = if is_active {
        Style::default().fg(Theme::search_bar())
    } else {
        Style::default().fg(Theme::text_muted())
    };

    let block = Block::default()
        .title(Line::from(Span::styled(
            search_title(query, current, total),
            style,
        )))
        .borders(Borders::ALL)
        .border_style(style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_width = inner.width as usize;
    if max_width == 0 || inner.height == 0 {
        return;
    }

    let prefix = "/ ";
    let display_query = truncate_left(query, max_width.saturating_sub(prefix.len()));
    let (before, after) = split_at_cursor(display_query, cursor);

    let mut spans = vec![Span::styled(prefix, style), Span::styled(before, style)];
    append_cursor_spans(&mut spans, after, is_active, style);

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn search_title(query: &str, current: usize, total: usize) -> String {
    if query.is_empty() {
        " Search ".to_string()
    } else if total == 0 {
        " Search (no matches) ".to_string()
    } else {
        format!(" Search ({current}/{total}) ")
    }
}

/// Keep the trailing `budget` characters of `query` so the cursor end stays
/// visible. Counts and slices on `char` boundaries — never raw byte indices —
/// so a multi-byte (non-ASCII) query never panics on a mid-codepoint slice.
fn truncate_left(query: &str, budget: usize) -> &str {
    let char_count = query.chars().count();
    if char_count <= budget {
        return query;
    }
    let start = query
        .char_indices()
        .nth(char_count - budget)
        .map_or(query.len(), |(i, _)| i);
    &query[start..]
}

fn split_at_cursor(text: &str, cursor: usize) -> (&str, &str) {
    if cursor > text.chars().count() {
        return (text, "");
    }
    let byte_pos = text
        .char_indices()
        .nth(cursor)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    (&text[..byte_pos], &text[byte_pos..])
}

fn append_cursor_spans<'a>(
    spans: &mut Vec<Span<'a>>,
    after: &'a str,
    is_active: bool,
    style: Style,
) {
    if !is_active {
        spans.push(Span::styled(after, style));
        return;
    }
    let first_len = after.chars().next().map_or(0, |c| c.len_utf8());
    let cursor_char = if first_len == 0 {
        " "
    } else {
        &after[..first_len]
    };
    spans.push(Span::styled(cursor_char, Theme::cursor()));
    let rest = &after[first_len..];
    if !rest.is_empty() {
        spans.push(Span::styled(rest, style));
    }
}

/// Compute the `start..end` slice of a `total`-row list that keeps `selected`
/// visible within a `height`-row viewport (with a small margin). Shared by the
/// file tree and the automation run-history list.
pub(crate) fn visible_window(total: usize, selected: usize, height: usize) -> (usize, usize) {
    if total <= height {
        return (0, total);
    }
    // Center-ish: keep selected in view with a small margin.
    let margin = (height / 4).min(3);
    let start = selected.saturating_sub(margin);
    let start = start.min(total.saturating_sub(height));
    let end = (start + height).min(total);
    (start, end)
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

    use super::*;
    use crate::session::{SessionInfo, WorktreeInfo};
    use std::path::PathBuf;

    fn sample_session() -> SessionInfo {
        let mut info = SessionInfo::new("t".into());
        info.worktrees.push(WorktreeInfo {
            repo_path: PathBuf::from("/tmp/a"),
            worktree_path: PathBuf::from("/tmp/a/wt"),
            branch: "main".into(),
        });
        info.additional_dirs.push(PathBuf::from("/tmp/b"));
        info
    }

    #[test]
    fn rebuild_from_session_collects_worktrees_and_additional_dirs() {
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&sample_session());
        assert_eq!(st.roots.len(), 2);
        assert_eq!(st.roots[0].path, PathBuf::from("/tmp/a/wt"));
        assert_eq!(st.roots[1].path, PathBuf::from("/tmp/b"));
        // The top level of each root is auto-expanded on rebuild so the viewer
        // opens to a populated tree (children read even if the dir is empty).
        for root in &st.roots {
            assert!(root.expanded, "root should be expanded after rebuild");
            assert!(root.children.is_some(), "root children should be read");
        }
    }

    #[test]
    fn move_selection_is_bounded() {
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&sample_session());
        st.move_selection(-5);
        assert_eq!(st.selected, 0);
        st.move_selection(100);
        assert_eq!(st.selected, 1);
    }

    #[test]
    fn activate_on_empty_is_noop() {
        let mut st = FileViewerState::new();
        assert!(matches!(st.activate(), Activation::NoOp));
    }

    #[test]
    fn activate_on_missing_dir_toggles() {
        // /nonexistent path: read_dir returns empty. The root starts auto-expanded
        // after rebuild, so the first activate collapses it and the next re-expands.
        let mut info = SessionInfo::new("t".into());
        info.additional_dirs
            .push(PathBuf::from("/this-path-does-not-exist-xyz"));
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&info);
        assert!(st.roots[0].expanded, "root is auto-expanded on rebuild");
        match st.activate() {
            Activation::Toggled => {}
            _ => panic!("expected Toggled"),
        }
        assert!(
            !st.roots[0].expanded,
            "activate collapses the expanded root"
        );
        match st.activate() {
            Activation::Toggled => {}
            _ => panic!("expected Toggled"),
        }
        assert!(
            st.roots[0].expanded,
            "activate re-expands the collapsed root"
        );
    }

    #[test]
    fn visible_window_fits_all_when_shorter_than_height() {
        assert_eq!(visible_window(5, 0, 10), (0, 5));
    }

    #[test]
    fn visible_window_scrolls_when_overflow() {
        let (s, e) = visible_window(100, 50, 10);
        assert!(s <= 50 && e > 50);
        assert_eq!(e - s, 10);
    }

    #[test]
    fn expected_root_paths_falls_back_to_cwd_when_empty() {
        let mut info = SessionInfo::new("t".into());
        info.cwd = Some(PathBuf::from("/tmp/only-cwd"));
        let roots = expected_root_paths(&info);
        assert_eq!(roots, vec![PathBuf::from("/tmp/only-cwd")]);
    }

    #[test]
    fn expected_root_paths_ignores_cwd_when_worktrees_present() {
        let mut info = sample_session();
        info.cwd = Some(PathBuf::from("/tmp/cwd"));
        let roots = expected_root_paths(&info);
        assert_eq!(roots.len(), 2);
        assert!(!roots.contains(&PathBuf::from("/tmp/cwd")));
    }

    #[test]
    fn needs_rebuild_detects_root_changes() {
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&sample_session());
        assert!(!st.needs_rebuild_for(&sample_session()));

        let mut other = SessionInfo::new("t".into());
        other.additional_dirs.push(PathBuf::from("/tmp/different"));
        assert!(st.needs_rebuild_for(&other));
    }

    #[test]
    fn search_push_updates_cursor_and_query() {
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&sample_session());
        st.start_search();
        st.search_push('a');
        st.search_push('b');
        assert_eq!(st.search_query, "ab");
        assert_eq!(st.search_cursor, 2);
    }

    #[test]
    fn end_search_clears_query_and_cursor() {
        let mut st = FileViewerState::new();
        st.start_search();
        st.search_push('x');
        st.end_search();
        assert!(!st.search_active);
        assert_eq!(st.search_query, "");
        assert_eq!(st.search_cursor, 0);
    }

    #[test]
    fn match_count_and_current_index_on_flat_roots() {
        let mut info = SessionInfo::new("t".into());
        info.additional_dirs.push(PathBuf::from("/tmp/alpha"));
        info.additional_dirs.push(PathBuf::from("/tmp/beta"));
        info.additional_dirs.push(PathBuf::from("/tmp/alphabet"));
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&info);
        st.start_search();
        st.search_push('a');
        st.search_push('l');
        // "alpha" and "alphabet" contain "al".
        assert_eq!(st.match_count(), 2);
        assert_eq!(st.current_match_index(), 1);
        st.next_match();
        assert_eq!(st.current_match_index(), 2);
        st.next_match();
        // Wraps back to first.
        assert_eq!(st.current_match_index(), 1);
        st.prev_match();
        assert_eq!(st.current_match_index(), 2);
    }

    #[test]
    fn next_match_noop_when_query_empty() {
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&sample_session());
        let before = st.selected;
        st.next_match();
        assert_eq!(st.selected, before);
    }

    #[test]
    fn truncate_left_keeps_trailing_chars_within_budget() {
        assert_eq!(truncate_left("hello", 10), "hello");
        assert_eq!(truncate_left("hello", 5), "hello");
        assert_eq!(truncate_left("hello", 3), "llo");
        assert_eq!(truncate_left("hello", 0), "");
    }

    #[test]
    fn truncate_left_handles_multibyte_query_without_panicking() {
        // A multi-byte (non-ASCII) query whose byte length exceeds the budget
        // used to slice on a raw byte index and panic mid-codepoint. Slicing on
        // char boundaries must keep the trailing chars and stay valid UTF-8.
        // "héllo wörld 你好" — mix of 1-, 2-, and 3-byte code points.
        let q = "héllo wörld 你好";
        assert!(q.len() > q.chars().count(), "query must be multi-byte");

        // Budget smaller than the char count would put a naive byte slice in
        // the middle of a multi-byte char. We keep the trailing `budget` chars.
        let out = truncate_left(q, 4);
        assert_eq!(out.chars().count(), 4);
        assert_eq!(out, "d 你好");

        // A budget that lands exactly on a multi-byte boundary, and one of 0.
        assert_eq!(truncate_left(q, 2), "你好");
        assert_eq!(truncate_left(q, 0), "");
        // Budget >= char count returns the whole string unchanged.
        assert_eq!(truncate_left(q, q.chars().count()), q);
        assert_eq!(truncate_left(q, 999), q);
    }

    // ── the pre-port renderer, retained as an oracle ────────────────────────
    //
    // The span-building row builder the pane used before it rendered through the
    // view tree, including its windowing. Kept so the port is a *checked*
    // refactor: the differential test below asserts the tree paints the same
    // cells, which is the only way to know that moving a pane onto the tree — and
    // moving its scroll window into the renderer — changed nothing a user sees.

    fn legacy_marker(row: &FileRow, nerd: bool) -> &'static str {
        match (row.is_dir, row.expanded, nerd) {
            (true, true, false) => "▾ ",
            (true, false, false) => "▸ ",
            (false, _, false) => "  ",
            (true, true, true) => "\u{f07c} ",
            (true, false, true) => "\u{f07b} ",
            (false, _, true) => "\u{f15b} ",
        }
    }

    fn legacy_label_color(is_match: bool, is_dir: bool) -> ratatui::style::Color {
        if !is_match {
            Theme::text_muted()
        } else if is_dir {
            Theme::accent()
        } else {
            Theme::text_primary()
        }
    }

    fn legacy_line(row: &FileRow, selected: bool, nerd: bool) -> Line<'static> {
        let indent = "  ".repeat(row.depth);
        let marker = legacy_marker(row, nerd);

        let label_style = if selected {
            LegacyStyle::default()
                .bg(Theme::selection_bg())
                .fg(Theme::selection_fg())
                .add_modifier(Modifier::BOLD)
        } else {
            let mut s = LegacyStyle::default().fg(legacy_label_color(row.matched, row.is_dir));
            if row.is_dir && row.matched {
                s = s.add_modifier(Modifier::BOLD);
            }
            s
        };
        let prefix_style = if selected {
            LegacyStyle::default()
                .bg(Theme::selection_bg())
                .fg(Theme::selection_fg())
        } else {
            LegacyStyle::default().fg(Theme::text_muted())
        };

        Line::from(vec![
            Span::styled(format!("{indent}{marker}"), prefix_style),
            Span::styled(row.name.clone(), label_style),
        ])
    }

    fn legacy_body(rows: &[FileRow], selected: usize, nerd: bool, height: usize) -> Paragraph<'_> {
        if rows.is_empty() {
            return Paragraph::new(Line::from(Span::styled(
                "No folders",
                LegacyStyle::default().fg(Theme::text_muted()),
            )));
        }
        let (start, end) = visible_window(rows.len(), selected, height);
        let lines: Vec<Line> = rows[start..end]
            .iter()
            .enumerate()
            .map(|(i, row)| legacy_line(row, start + i == selected, nerd))
            .collect();
        Paragraph::new(lines)
    }

    fn frow(name: &str, depth: usize, is_dir: bool, expanded: bool, matched: bool) -> FileRow {
        FileRow {
            name: name.into(),
            depth,
            is_dir,
            expanded,
            matched,
        }
    }

    /// The differential cases: every row appearance the pane has, plus the two
    /// cases where geometry decides something.
    fn differential_cases() -> Vec<(&'static str, Vec<FileRow>, usize, bool)> {
        vec![
            ("an empty tree", Vec::new(), 0, false),
            (
                "a root, a nested dir and files",
                vec![
                    frow("repo", 0, true, true, true),
                    frow("src", 1, true, true, true),
                    frow("main.rs", 2, false, false, true),
                    frow("target", 1, true, false, true),
                    frow("README.md", 1, false, false, true),
                ],
                0,
                false,
            ),
            (
                "the same tree with nerd glyphs",
                vec![
                    frow("repo", 0, true, true, true),
                    frow("src", 1, true, false, true),
                    frow("main.rs", 1, false, false, true),
                ],
                1,
                true,
            ),
            (
                "the cursor on a file, then on a directory",
                vec![
                    frow("repo", 0, true, true, true),
                    frow("main.rs", 1, false, false, true),
                ],
                1,
                false,
            ),
            (
                "a running search: matched and excluded rows",
                vec![
                    frow("host", 0, true, true, true),
                    frow("hosts.toml", 1, false, false, true),
                    frow("unrelated.rs", 1, false, false, false),
                    frow("also-out", 1, true, false, false),
                ],
                1,
                false,
            ),
            (
                "a selected row the search excluded",
                vec![
                    frow("repo", 0, true, true, true),
                    frow("nope.rs", 1, false, false, false),
                ],
                1,
                false,
            ),
            (
                "multi-byte and wide names",
                vec![
                    frow("répertoire", 0, true, true, true),
                    frow("日本語.md", 1, false, false, true),
                ],
                1,
                false,
            ),
            (
                "a cursor past the last row",
                vec![frow("only", 0, true, true, true)],
                7,
                false,
            ),
        ]
    }

    fn render_both(
        rows: &[FileRow],
        selected: usize,
        nerd: bool,
        w: u16,
        h: u16,
    ) -> (Buffer, Buffer) {
        let area = Rect::new(0, 0, w, h);
        let mut tree_buf = Buffer::empty(area);
        let tree = file_tree(rows, (!rows.is_empty()).then_some(selected), nerd);
        super::super::plugin_pane::render_tree(
            &tree,
            area,
            &super::super::theme::current(),
            &FrameTable::default(),
            &mut tree_buf,
        );

        // The legacy side reserves and draws its own track, which is what
        // `render_rows` did around this body before the tree declared one. Without
        // it the comparison would only prove that the tree still paints rows at
        // the pane's full width — the property the reservation deliberately
        // changes.
        let (legacy_rows, legacy_track) =
            super::super::scrollbar::reserve_track(area, rows.len(), h as usize);
        let mut legacy_buf = Buffer::empty(area);
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|f| {
                f.render_widget(legacy_body(rows, selected, nerd, h as usize), legacy_rows);
                if let Some(track) = legacy_track {
                    super::super::scrollbar::render_into(
                        f,
                        track,
                        rows.len(),
                        h as usize,
                        selected,
                    );
                }
            })
            .unwrap();
        legacy_buf.merge(terminal.backend().buffer());
        (tree_buf, legacy_buf)
    }

    /// The port's own guarantee: the tree paints the pane the pre-port renderer
    /// painted, cell for cell and style for style — at a height that fits and at
    /// one that forces the window to scroll.
    #[test]
    fn the_view_tree_paints_what_the_span_renderer_painted() {
        for (name, rows, selected, nerd) in differential_cases() {
            for height in [8u16, 3, 2] {
                let (tree, legacy) = render_both(&rows, selected, nerd, 28, height);
                assert_eq!(
                    tree, legacy,
                    "the tree diverges from the span renderer for `{name}` at height {height}"
                );
            }
        }
    }

    /// The window moved into the renderer, so the property that mattered is that
    /// the *renderer's* window still keeps the cursor visible — asserted on the
    /// painted cells rather than on the helper both sides call.
    #[test]
    fn the_renderer_scrolls_the_tree_to_its_cursor() {
        let rows: Vec<FileRow> = (0..12)
            .map(|i| frow(&format!("f{i}.rs"), 1, false, false, true))
            .collect();
        let area = Rect::new(0, 0, 12, 4);
        let mut buf = Buffer::empty(area);
        super::super::plugin_pane::render_tree(
            &file_tree(&rows, Some(11), false),
            area,
            &super::super::theme::current(),
            &FrameTable::default(),
            &mut buf,
        );
        let text: String = (0..area.height)
            .flat_map(|y| (0..area.width).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol().to_string())
            .collect();
        assert!(text.contains("f11.rs"), "the cursor's row must be drawn");
        assert!(
            !text.contains("f0.rs"),
            "and the top must have scrolled off"
        );
    }

    /// The window is resolved twice — once by the renderer to paint, once by the
    /// pane for its hitboxes and scrollbar — so the two must agree. A drift here
    /// is a click that selects a different row than the one under the pointer.
    #[test]
    fn the_click_hitboxes_cover_the_rows_the_renderer_drew() {
        let mut info = SessionInfo::new("t".into());
        for name in ["/tmp/a", "/tmp/b", "/tmp/c", "/tmp/d", "/tmp/e"] {
            info.additional_dirs.push(PathBuf::from(name));
        }
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&info);
        st.select_index(4);

        // 5 rows into 3 inner lines: the window has to scroll, and the hitboxes
        // must be the scrolled window's.
        let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();
        let mut rows = Vec::new();
        terminal
            .draw(|f| {
                rows = render_file_viewer(f, Rect::new(0, 0, 20, 5), &st, FocusLevel::Inactive).1;
            })
            .unwrap();

        let indices: Vec<usize> = rows.iter().map(|r| r.index).collect();
        let (start, end) = visible_window(5, 4, 3);
        assert_eq!(indices, (start..end).collect::<Vec<_>>());
        assert!(indices.contains(&4), "the cursor's row is clickable");
    }

    /// The tree must be a description of content only: nothing in it may depend
    /// on a width, which shows up as a row padded to fill one.
    #[test]
    fn the_tree_carries_no_geometry() {
        let (_, rows, selected, nerd) = differential_cases().remove(1);
        fn assert_no_padding(node: &ViewNode) {
            if let ViewNode::Text { content, .. } = node {
                assert!(
                    !content.ends_with("  ") || content.trim().is_empty(),
                    "a padded run means the tree resolved a width: {content:?}"
                );
            }
            node.children().iter().for_each(assert_no_padding);
        }
        assert_no_padding(&file_tree(&rows, Some(selected), nerd));
    }

    /// `rows` is the one traversal both the pane and the published snapshot use,
    /// so the search's verdict has to be in it — the pane must not need the query.
    #[test]
    fn rows_carry_the_searchs_verdict_and_not_its_query() {
        let mut info = SessionInfo::new("t".into());
        info.additional_dirs.push(PathBuf::from("/tmp/alpha"));
        info.additional_dirs.push(PathBuf::from("/tmp/beta"));
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&info);

        // No search: every row matches, which is what keeps an unsearched tree in
        // its ordinary colours.
        assert!(st.rows().iter().all(|r| r.matched));

        st.start_search();
        st.search_push('a');
        st.search_push('l');
        let rows = st.rows();
        assert!(rows[0].matched, "alpha matches `al`");
        assert!(!rows[1].matched, "beta does not");

        // An empty query is not a running search.
        st.search_pop();
        st.search_pop();
        assert!(st.rows().iter().all(|r| r.matched));
    }

    #[test]
    fn rows_report_depth_and_kind() {
        let mut info = SessionInfo::new("t".into());
        info.additional_dirs.push(PathBuf::from("/tmp"));
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&info);
        let rows = st.rows();
        assert_eq!(rows[0].depth, 0, "a root is depth zero");
        assert!(rows[0].is_dir && rows[0].expanded);
        assert!(rows.iter().skip(1).all(|r| r.depth == 1));
    }

    /// A row's name is a basename, never a path — the boundary the published
    /// section rests on, checked at the source that fills it.
    #[test]
    fn a_rows_name_is_a_basename() {
        let mut info = SessionInfo::new("t".into());
        info.additional_dirs.push(PathBuf::from("/tmp/some-dir"));
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&info);
        for row in st.rows() {
            assert!(
                !row.name.contains(std::path::MAIN_SEPARATOR),
                "{:?} is a path, not a basename",
                row.name
            );
        }
    }

    #[test]
    fn clear_resets_state() {
        let mut st = FileViewerState::new();
        st.rebuild_from_session(&sample_session());
        assert_eq!(st.roots.len(), 2);
        st.clear();
        assert_eq!(st.roots.len(), 0);
        assert_eq!(st.selected, 0);
    }
}
