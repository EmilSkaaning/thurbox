//! Global search's Phase 4 verdict, enforced as a test.
//!
//! `docs/PHASE4-PANE-READINESS.md` records that global search **cannot** be a
//! bundled plugin under the current model: it is not a pane but a mode — it owns
//! the whole interface's input while it is up, restyles rows in three panes it
//! does not own, and moves their cursors. This file is that verdict in executable
//! form: [`BLOCKERS`] records one host power the surface needs and does not have,
//! per row, and re-derives each from the source.
//!
//! Why a test rather than only the document. `tests/teardown_gate.rs` states the
//! reason for its own table and it applies verbatim here: a verdict written in
//! markdown is a fact about a build that expires without telling anyone. "Global
//! search cannot be a plugin" stops being true the moment someone adds a band
//! slot or a write binding for an unrelated reason, and nothing would say so.
//! With a probe per blocker, the change that closes one fails here and is told
//! which — so the verdict is revisited by the person who invalidated it.
//!
//! Two things this gate is deliberately not. It is **not** the teardown gate,
//! which answers whether `src/ui/global_search.rs` may be *deleted* (it may not,
//! and that row is correct either way); merging them would make one table answer
//! two questions and a failure would not say which. And it is **not** a claim
//! that the strip's rendering is inexpressible — four of the rows below are
//! vocabulary gaps a small widening would close, which is why each row says
//! which kind it is.
//!
//! Probes read the source tree the way a human auditor would, so this gate runs,
//! and means the same thing, with or without the `plugins` Cargo feature.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Whether the pane model withholds a power **by design**, or merely has not
/// spelled it yet.
///
/// The distinction is the finding, not bookkeeping: closing every
/// [`Gap::Vocabulary`] row would let a plugin *draw* something that looks like
/// the strip while leaving it unable to search, highlight, preview or jump. So
/// portability is not "no rows left" by accident — it is a conjunction that the
/// structural rows dominate, which [`the_verdict_is_derived_from_the_blockers`]
/// asserts in both directions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gap {
    /// A power a plugin pane is not given, on purpose. Reversing it changes what
    /// a plugin *is*, so it cannot be closed by a further node or token.
    Structural,
    /// Something today's catalogue cannot say and could. Cheap, and worth
    /// nothing on its own.
    Vocabulary,
}

/// One host power the global-search strip needs and does not have.
#[derive(Clone, Copy)]
struct Blocker {
    /// Stable id, named in a failure.
    id: &'static str,
    /// What the strip does that needs it.
    needs: &'static str,
    /// Where the host stands today.
    stands: &'static str,
    gap: Gap,
    /// The recorded verdict: is this still missing?
    blocked: bool,
    /// Re-derives that verdict from the source, given the repo root. `true`
    /// means "still missing", so a probe and its row read the same way round.
    probe: fn(&Path) -> bool,
}

/// Every power the port would have needed, structural rows first.
///
/// Each probe is scoped to the *declaration* it reads rather than to a whole
/// file — copied from `tests/teardown_gate.rs` for its reason: an unrelated
/// mention elsewhere in a file must not flip a verdict. A probe that read
/// `capabilities.rs` for the word "search", for instance, would already be
/// wrong, because a comment there mentions what a file search matched.
const BLOCKERS: &[Blocker] = &[
    Blocker {
        id: "no-band-slot",
        needs: "a full-width band docked above the footer",
        stands: "`PaneSlot` is a closed set whose only member is the right-hand column",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // The strip's own region (`RegionId::GlobalSearch`) exists; what is
            // missing is any way for a *plugin* to be seated in one.
            variant_names(&block(
                root,
                "src/session/plugin_manifest.rs",
                "pub enum PaneSlot",
            )) == ["Right"]
        },
    },
    Blocker {
        id: "no-search-state",
        needs: "the query the user typed and the results it matched",
        stands: "no capability publishes either, and none can be scoped honestly — computing the \
                 search reads every session's live terminal screen, while publishing the kernel's \
                 results publishes the strip's rendering rather than kernel state",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            let capabilities = variant_names(&block(
                root,
                "src/session/plugin_manifest.rs",
                "pub enum Capability",
            ));
            let named = |needle: &str| {
                capabilities
                    .iter()
                    .any(|c| c.to_lowercase().contains(needle))
            };
            let bound = |needle: &str| {
                module_bindings(root)
                    .iter()
                    .any(|b| b.to_lowercase().contains(needle))
            };
            !named("search") && !named("query") && !bound("search") && !bound("query")
        },
    },
    Blocker {
        id: "no-cross-pane-styling",
        needs: "restyling rows in the session list, tasks and automations panes",
        stands: "the search's verdict already crosses *outward* as a property of each published \
                 row, but each native pane applies it itself from a query the view hands it, and \
                 nothing carries a query the other way",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Both halves, because either one alone would be the wrong claim:
            // the first says where v1's highlighting lives, the second that a
            // plugin's tree cannot carry it. The outward direction — a published
            // row's `dimmed`/`match_positions` — is asserted separately, since
            // it is this row's *evidence* rather than its verdict.
            let native_panes_highlight_themselves = [
                "src/ui/project_list.rs",
                "src/ui/tasks_panel.rs",
                "src/ui/automations_panel.rs",
            ]
            .iter()
            .all(|pane| source(root, pane).contains("highlight"));
            let renderer = source(root, "src/ui/plugin_pane.rs").to_lowercase();
            native_panes_highlight_themselves
                && !renderer.contains("highlight")
                && !renderer.contains("query")
        },
    },
    Blocker {
        id: "read-only-state-channel",
        needs: "moving another pane's cursor, taking focus, and restoring both on cancel",
        stands: "the kernel-state channel is read-only by construction — every granted binding \
                 reads a published snapshot",
        gap: Gap::Structural,
        blocked: true,
        probe: |root| {
            // Matched as a *verb* in the host's camelCase binding names, so
            // `setFocus` is caught while a reader that merely begins with the
            // same letters (`settings`) is not. A plugin's own `stateWrite` and
            // `stateDelete` need no exception: neither begins with a verb.
            const WRITES: [&str; 8] = [
                "set", "select", "focus", "move", "open", "write", "jump", "goto",
            ];
            module_bindings(root).iter().all(|binding| {
                !WRITES.iter().any(|verb| {
                    binding
                        .strip_prefix(verb)
                        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_uppercase))
                })
            })
        },
    },
    Blocker {
        id: "no-frame-node",
        needs: "the strip's own bordered block, titled ` Search `",
        stands: "the catalogue has no frame node, and a pane's frame is drawn by the host",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let kinds = view_node_kinds(root);
            !kinds
                .iter()
                .any(|k| k == "Frame" || k == "Block" || k == "Border")
        },
    },
    Blocker {
        id: "no-bottom-anchored-region",
        needs: "a hint row pinned to the last line, under a result list that takes the rest",
        stands: "`Column` stacks children at their natural height from the top; nothing anchors \
                 to the far edge",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            let kinds = view_node_kinds(root);
            // `Fill` is on the list because a fill *could* be the shape that
            // anchors a row to the far edge — but the one this catalogue has is
            // an **inline** run whose width is the residue of a *line*, added by
            // the code-review port to carry a diff row's background to the pane's
            // right edge (ADR-31). A horizontal residue anchors nothing
            // vertically, so it does not close this row. Asked of the tree rather
            // than asserted here: a fill that stopped being inlineable would be a
            // different node, and would flip this verdict as it should.
            let inline_fill = source(root, "src/session/view_tree.rs")
                .contains("ViewNode::Text { .. } | ViewNode::Fill { .. } => None");
            !kinds.iter().any(|k| {
                k == "Anchor" || k == "Flex" || k == "Dock" || (k == "Fill" && !inline_fill)
            })
        },
    },
    Blocker {
        id: "no-search-accent-token",
        needs: "the search accent the strip's frame, prompt and caret are drawn in",
        stands: "`StyleToken` names no search-bar role, and a plugin may name no colour",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            !variant_names(&block(
                root,
                "src/session/view_tree.rs",
                "pub enum StyleToken",
            ))
            .iter()
            .any(|t| t.to_lowercase().contains("search"))
        },
    },
    Blocker {
        id: "no-italic-emphasis",
        needs: "the italic snippet line under a content match",
        stands: "`TextStyle` carries bold, dim, underline and the selection role — no italic",
        gap: Gap::Vocabulary,
        blocked: true,
        probe: |root| {
            !block(root, "src/session/view_tree.rs", "pub struct TextStyle")
                .to_lowercase()
                .contains("italic")
        },
    },
];

/// Whether the surface could be ported at all: every blocker closed.
///
/// Pure over the table so both answers are testable — today's, and a tree where
/// each row landed.
fn is_portable(blockers: &[Blocker]) -> bool {
    blockers.iter().all(|b| !b.blocked)
}

/// The structural rows still blocking — the ones no widening of the catalogue
/// reaches.
fn structural_blockers(blockers: &[Blocker]) -> Vec<&'static str> {
    blockers
        .iter()
        .filter(|b| b.blocked && b.gap == Gap::Structural)
        .map(|b| b.id)
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read a source file, panicking if it is unreadable — a probe that silently
/// read nothing would report a gap as closed for the wrong reason.
fn source(root: &Path, rel: &str) -> String {
    let path = root.join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The body of the `struct`/`enum` whose declaration starts with `header`.
///
/// rustfmt closes a top-level item with a `}` in column zero, which is the
/// terminator used here.
fn block(root: &Path, rel: &str, header: &str) -> String {
    let text = source(root, rel);
    let start = text
        .find(header)
        .unwrap_or_else(|| panic!("{rel} no longer declares `{header}`"));
    let rest = &text[start..];
    let end = rest
        .find("\n}\n")
        .unwrap_or_else(|| panic!("{rel}: `{header}` has no top-level close"));
    rest[..end].to_string()
}

/// The variant names declared in an item body, payloads and attributes ignored.
///
/// A variant is a line at exactly one level of indentation starting with an
/// uppercase letter, which is what rustfmt produces for every enum in the tree —
/// so this reads `Right,` and `Gauge {` alike without a parser.
fn variant_names(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let unindented = line.strip_prefix("    ")?;
            if unindented.starts_with(' ') || !unindented.starts_with(char::is_uppercase) {
                return None;
            }
            let name: String = unindented
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// The node kinds the view tree defines.
fn view_node_kinds(root: &Path) -> Vec<String> {
    variant_names(&block(
        root,
        "src/session/view_tree.rs",
        "pub enum ViewNode",
    ))
}

/// Every name inserted into a plugin's `@thurbox` module table.
///
/// The granted surface is exactly this list — enforcement in the host is by
/// absence, so what a plugin holds is what was `set` on the module — which makes
/// it the honest thing to probe for "is there a binding that does X".
fn module_bindings(root: &Path) -> Vec<String> {
    let text = source(root, "src/plugin/capabilities.rs");
    let mut names = Vec::new();
    for (at, matched) in text.match_indices("module.set(\"") {
        let tail = &text[at + matched.len()..];
        if let Some(name) = tail.split('"').next() {
            names.push(name.to_string());
        }
    }
    assert!(
        names.iter().any(|n| n == "ui"),
        "no module bindings found — the probe's anchor (`module.set(\"…\"`) changed shape, so \
         every verdict derived from it is meaningless"
    );
    names
}

/// A recorded verdict is only worth having if it cannot drift from the tree.
#[test]
fn recorded_blockers_match_the_tree() {
    let root = repo_root();
    let mut failures = String::new();

    for b in BLOCKERS {
        let probed = (b.probe)(&root);
        if probed == b.blocked {
            continue;
        }
        let _ = write!(
            failures,
            "\n{}: recorded {}, but the tree says {} — the strip needs {} ({})",
            b.id,
            if b.blocked { "missing" } else { "available" },
            if probed { "missing" } else { "available" },
            b.needs,
            b.stands,
        );
    }

    assert!(
        failures.is_empty(),
        "global search's port verdict disagrees with the source tree.\n\
         A row that became available may unblock part of the port: re-verdict it here and \
         revisit docs/PHASE4-PANE-READINESS.md in the same change.{failures}"
    );
}

/// Whether the port is possible follows from the rows alone, so all three
/// answers are checkable: today's, a tree where only the cheap rows landed, and
/// one where everything did.
#[test]
fn the_verdict_is_derived_from_the_blockers() {
    assert!(
        !is_portable(BLOCKERS),
        "every blocker is recorded closed — global search may now be portable, so retire this \
         gate deliberately (and port the pane) rather than leaving it passing vacuously"
    );

    // The load-bearing half: the reason is structural. Closing every vocabulary
    // gap must not read as progress toward the port, because a pane that can
    // draw the strip still cannot search, highlight, preview or jump.
    let structural = structural_blockers(BLOCKERS);
    assert!(
        structural.len() >= 4,
        "the recorded reason is supposed to be structural, but only {structural:?} are"
    );
    let vocabulary_closed: Vec<Blocker> = BLOCKERS
        .iter()
        .map(|b| Blocker {
            blocked: b.blocked && b.gap == Gap::Structural,
            ..*b
        })
        .collect();
    assert!(!is_portable(&vocabulary_closed));
    assert_eq!(structural_blockers(&vocabulary_closed), structural);

    // The other direction: a fully landed table would permit the port.
    let all_closed: Vec<Blocker> = BLOCKERS
        .iter()
        .map(|b| Blocker {
            blocked: false,
            ..*b
        })
        .collect();
    assert!(is_portable(&all_closed));
    assert!(structural_blockers(&all_closed).is_empty());
}

/// The other half of the record: nobody shipped the pane anyway.
///
/// Without this the gate would pass while a bundled `global-search` plugin sat
/// in the tree contradicting it — and `tests/teardown_gate.rs` would then read
/// that plugin as one step toward deleting the native strip.
#[test]
fn no_bundled_plugin_claims_global_search() {
    let root = repo_root();
    for dir in ["global-search", "global_search", "search"] {
        let manifest = root
            .join("src/plugin/bundled")
            .join(dir)
            .join("plugin.toml");
        assert!(
            !manifest.exists(),
            "{} exists: a bundled plugin claims global search while the record says the surface \
             cannot be one. Either the port happened (re-verdict this gate and \
             docs/PHASE4-PANE-READINESS.md) or the plugin is not what it looks like.",
            manifest.display()
        );
    }
    assert!(
        !source(&root, "src/plugin/discovery.rs").contains("global-search"),
        "discovery ships a bundled plugin named for global search"
    );
}

/// The evidence behind the cross-pane row, asserted on its own because the
/// precise claim is narrower than "a plugin cannot see the search" — and the
/// narrower claim is the interesting one.
///
/// The search's **verdict already crosses outward**: a published task row carries
/// `dimmed` and `match_positions`, and a file row carries `matched`, which is how
/// the bundled tasks and file-viewer panes reproduce v1's dim-and-underline
/// appearance exactly. So a plugin pane *does* show a running search's effect on
/// its own rows.
///
/// What has no path is the way back. v1's highlighting is applied inside each
/// native pane from a query the view hands it; a plugin's tree mentions no query,
/// the renderer that paints it is told of none, and no binding submits one. So a
/// plugin can be a pane the search *affects* and cannot be the surface that
/// *produces* the effect — which is the direction global search needs.
#[test]
fn the_search_verdict_crosses_outward_but_no_query_comes_back() {
    let root = repo_root();
    let published = source(&root, "src/session/pane_context.rs");
    for field in ["match_positions", "dimmed", "matched"] {
        assert!(
            published.contains(field),
            "the published sections no longer carry `{field}` — the outward half of the \
             cross-pane row's evidence moved, so the row's wording needs revisiting"
        );
    }
    for pane in [
        "src/ui/project_list.rs",
        "src/ui/tasks_panel.rs",
        "src/ui/automations_panel.rs",
    ] {
        assert!(
            source(&root, pane).contains("highlight"),
            "{pane} no longer highlights its own rows — the cross-pane blocker's evidence moved"
        );
    }
    let renderer = source(&root, "src/ui/plugin_pane.rs").to_lowercase();
    assert!(
        !renderer.contains("highlight") && !renderer.contains("query"),
        "the plugin renderer now knows about a search: revisit the cross-pane blocker"
    );
}

/// A row with no probe distinction, or a table with only one kind of gap, would
/// make the structural/vocabulary split decorative.
#[test]
fn the_record_is_well_formed() {
    let mut ids: Vec<&str> = BLOCKERS.iter().map(|b| b.id).collect();
    ids.sort_unstable();
    let unique = {
        let mut u = ids.clone();
        u.dedup();
        u
    };
    assert_eq!(ids, unique, "duplicate blocker id");

    assert!(
        BLOCKERS.iter().any(|b| b.gap == Gap::Structural),
        "a record with no structural row would say the port is merely deferred"
    );
    assert!(
        BLOCKERS.iter().any(|b| b.gap == Gap::Vocabulary),
        "a record with no vocabulary row would overstate the finding — part of the strip's \
         rendering is a small widening away"
    );
    for b in BLOCKERS {
        assert!(!b.needs.is_empty() && !b.stands.is_empty(), "{}", b.id);
    }
}
