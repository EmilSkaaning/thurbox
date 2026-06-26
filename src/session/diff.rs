//! Pure data types for the file-viewer diff/review pane.
//!
//! These live in `session` (the dependency-free data layer) so the `git`
//! module can *produce* them and the `ui` module can *render* them without
//! `ui` ever referencing `git` — the same split [`GitStats`] uses. `ui` colors
//! a diff purely by [`DiffLineKind`]; it never parses a diff itself.
//!
//! [`GitStats`]: crate::session::GitStats

use std::path::PathBuf;

/// Git change status of a file in a worktree diff vs its base ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    /// Present in the worktree but not tracked by git.
    Untracked,
}

impl DiffStatus {
    /// Single-character marker shown in the file tree / changes list.
    pub fn glyph(self) -> char {
        match self {
            DiffStatus::Modified => 'M',
            DiffStatus::Added => 'A',
            DiffStatus::Deleted => 'D',
            DiffStatus::Renamed => 'R',
            DiffStatus::Untracked => '?',
        }
    }

    /// Parse git's porcelain status letter (from `--name-status` /
    /// `status --porcelain`). Returns `None` for unrecognized codes.
    pub fn from_code(code: char) -> Option<Self> {
        match code {
            'M' => Some(DiffStatus::Modified),
            'A' => Some(DiffStatus::Added),
            'D' => Some(DiffStatus::Deleted),
            'R' => Some(DiffStatus::Renamed),
            'C' => Some(DiffStatus::Added), // copy → treat as added
            'T' => Some(DiffStatus::Modified), // type-change → modified
            '?' => Some(DiffStatus::Untracked),
            _ => None,
        }
    }
}

/// One changed file in a worktree diff vs a base ref. Line counts come from
/// `git diff --numstat`; a binary file reports `0/0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: PathBuf,
    pub status: DiffStatus,
    pub insertions: usize,
    pub deletions: usize,
}

/// The set of changes in a worktree vs a resolved base ref, computed by the
/// app/git layer off the render path and cached for the file viewer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorktreeDiff {
    /// The base ref the diff was taken against (e.g. `origin/main`). Shown in
    /// the pane header so the comparison point is explicit.
    pub base_ref: String,
    /// Changed files, ordered most-significant-status first by the producer.
    pub files: Vec<FileChange>,
}

impl WorktreeDiff {
    /// Total `(files, insertions, deletions)` rollup for the header line.
    pub fn totals(&self) -> (usize, usize, usize) {
        let (mut ins, mut del) = (0usize, 0usize);
        for f in &self.files {
            ins += f.insertions;
            del += f.deletions;
        }
        (self.files.len(), ins, del)
    }

    /// Status of a specific path, if it changed. Used to annotate tree rows.
    pub fn status_of(&self, path: &std::path::Path) -> Option<DiffStatus> {
        self.files.iter().find(|f| f.path == path).map(|f| f.status)
    }
}

/// Kind of a single line in a rendered unified diff. The `ui` layer maps each
/// to a color (added=green, removed=red, hunk=accent, context/meta=muted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Added,
    Removed,
    Context,
    /// A `@@ -a,b +c,d @@` hunk header.
    Hunk,
    /// `diff --git`, `index`, `---`/`+++`, `Binary files …` etc.
    Meta,
}

/// One line of a rendered file diff, already classified so `ui` only colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

/// A single file's unified diff vs the base ref, parsed into classified lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: PathBuf,
    pub base_ref: String,
    pub lines: Vec<DiffLine>,
    /// True when git reported a binary file (no textual hunks available).
    pub binary: bool,
}

impl FileDiff {
    /// New-file line number of the first added/context line after a hunk
    /// header — the position to jump an editor to. `None` when no hunk is
    /// present (e.g. a binary or empty diff).
    pub fn first_changed_line(&self) -> Option<u32> {
        let mut new_line: Option<u32> = None;
        for l in &self.lines {
            match l.kind {
                DiffLineKind::Hunk => {
                    new_line = parse_hunk_new_start(&l.text);
                }
                DiffLineKind::Added => {
                    if let Some(n) = new_line {
                        return Some(n);
                    }
                }
                DiffLineKind::Context => {
                    if let Some(n) = new_line.as_mut() {
                        *n += 1;
                    }
                }
                DiffLineKind::Removed | DiffLineKind::Meta => {}
            }
        }
        // No added line found: fall back to the first hunk's start.
        self.lines
            .iter()
            .find(|l| l.kind == DiffLineKind::Hunk)
            .and_then(|l| parse_hunk_new_start(&l.text))
    }
}

/// Extract the new-file start line from a hunk header `@@ -a,b +c,d @@`.
fn parse_hunk_new_start(header: &str) -> Option<u32> {
    let plus = header.split('+').nth(1)?;
    let num: String = plus.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_glyph_and_code_roundtrip() {
        for s in [
            DiffStatus::Modified,
            DiffStatus::Added,
            DiffStatus::Deleted,
            DiffStatus::Renamed,
            DiffStatus::Untracked,
        ] {
            assert_eq!(DiffStatus::from_code(s.glyph()), Some(s));
        }
        assert_eq!(DiffStatus::from_code('X'), None);
    }

    #[test]
    fn worktree_diff_totals() {
        let d = WorktreeDiff {
            base_ref: "origin/main".into(),
            files: vec![
                FileChange {
                    path: "a.rs".into(),
                    status: DiffStatus::Modified,
                    insertions: 3,
                    deletions: 1,
                },
                FileChange {
                    path: "b.rs".into(),
                    status: DiffStatus::Added,
                    insertions: 10,
                    deletions: 0,
                },
            ],
        };
        assert_eq!(d.totals(), (2, 13, 1));
        assert_eq!(
            d.status_of(std::path::Path::new("a.rs")),
            Some(DiffStatus::Modified)
        );
        assert_eq!(d.status_of(std::path::Path::new("missing")), None);
    }

    #[test]
    fn hunk_new_start_parses() {
        assert_eq!(parse_hunk_new_start("@@ -1,2 +3,4 @@ fn foo()"), Some(3));
        assert_eq!(parse_hunk_new_start("@@ -0,0 +1 @@"), Some(1));
        assert_eq!(parse_hunk_new_start("not a hunk"), None);
    }

    #[test]
    fn first_changed_line_walks_context() {
        let fd = FileDiff {
            path: "x.rs".into(),
            base_ref: "origin/main".into(),
            binary: false,
            lines: vec![
                DiffLine {
                    kind: DiffLineKind::Meta,
                    text: "diff --git".into(),
                },
                DiffLine {
                    kind: DiffLineKind::Hunk,
                    text: "@@ -10,3 +10,4 @@".into(),
                },
                DiffLine {
                    kind: DiffLineKind::Context,
                    text: " ctx".into(),
                },
                DiffLine {
                    kind: DiffLineKind::Added,
                    text: "+new".into(),
                },
            ],
        };
        // hunk new-start 10, one context line → added at 11.
        assert_eq!(fd.first_changed_line(), Some(11));
    }
}
