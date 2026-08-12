//! The file viewer's **model**: the tree the pane shows, and what its keys do to it.
//!
//! It lived in `src/ui/file_viewer.rs` until that pane was handed over to its bundled
//! plugin (ADR-58), which deleted the renderer and left this behind. It is here rather
//! than in `session` — where the *types* would fit as comfortably as `session::review`'s
//! do — because it is not pure data: [`FileViewerState::activate`],
//! [`FileViewerState::reveal_path`] and the search's auto-expansion all **read
//! directories**, and `session` is the layer the architecture rules keep free of
//! effects. `App` owns the value, so `app` owns the state machine.
//!
//! Reading it is also the one place thurbox touches the filesystem on a plugin's
//! account, and it does so on the *kernel's* key: a pane declaring the `FileViewer`
//! keyboard is focused as [`InputFocus::FileViewer`](super::InputFocus), the kernel
//! resolves the seven scoped actions, and these methods run. Nothing here is reachable
//! from a plugin, and `Capability::Files` publishes a basename per row and no path.
//!
//! [`FileViewerState::rows`] yields the **published** row type
//! ([`FileNodeSnapshot`]) rather than a drawable one of its own: with the renderer gone
//! the publication is its only consumer, and two structs with the same five fields would
//! be one fact in two places.

use std::path::{Path, PathBuf};

use crate::session::pane_context::FileNodeSnapshot;
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
    /// A file was activated — the caller should open it in the editor.
    ///
    /// Carries no path: the caller needs the file **and** its root (an editor is
    /// launched against the repo it belongs to), and must resolve both *before*
    /// activating, since activating moves the cursor's node. A payload nobody could
    /// read was dead the moment the renderer stopped being in this module.
    Open,
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
    ///
    /// Read only by the search bar's title, which is seat chrome for a pane a plugin
    /// draws — so a build with no plugin host has no file viewer and never asks
    /// (ADR-58). Same for [`Self::current_match_index`] and [`Self::select_index`].
    #[cfg(feature = "plugins")]
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
    #[cfg(feature = "plugins")]
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
    /// The one traversal of the tree the published snapshot is built from, which is
    /// what the pane draws — so there is no second answer to "which rows, matched
    /// how". Reads nothing from disk: the tree it walks is whatever the user has
    /// already expanded.
    ///
    /// Yields the published row type directly. It carried a `FileRow` of its own
    /// while a native renderer consumed one; the two had the same five fields, and
    /// the handover left the publication as the only consumer (ADR-58).
    pub fn rows(&self) -> Vec<FileNodeSnapshot> {
        // A search that is not running, or has an empty query, matches every
        // row — which is how an unsearched tree keeps its ordinary colours.
        let query = (self.search_active && !self.search_query.is_empty())
            .then(|| self.search_query.to_lowercase());
        self.flatten()
            .into_iter()
            .map(|row| FileNodeSnapshot {
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

    /// Select the row at `index`, clamped to the current row count. Used by a click
    /// on the pane's *n*th row, which the kernel routes here because the pane declared
    /// its keyboard — the scrollbar drag that also used this went with the native
    /// renderer (ADR-58).
    #[cfg(feature = "plugins")]
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
            Activation::Open
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

#[cfg(test)]
mod tests {
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

    /// The counter the search bar's title shows, and so gated with it.
    #[cfg(feature = "plugins")]
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
