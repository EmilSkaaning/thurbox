# Design

## 1. The problem stated precisely

`tests/bundled_info_panel.rs` proves an equality between two expressions:

```text
plugin_render(context(case))  ==  ui::info_panel::info_tree(case…)
```

A handover deletes the right-hand side. Every repair that keeps the file
compiling either deletes the assertion or replaces the right-hand side with
something. This change chooses what that something is, and — more importantly —
chooses *when* it is captured, because a baseline recorded after the native
builder is gone can only be a recording of the plugin.

## 2. Rejected: keep the native builder alive as a test-only oracle

Move `info_tree` behind `#[cfg(test)]` or into a `test-support` module, delete
only `render_info_panel`, and keep comparing against it forever.

Rejected for three reasons, the first decisive:

1. `migration/phase-4` already forbids it in terms: "A port MUST NOT satisfy this
   by keeping both renderers and selecting between them on the compile-time
   feature. That leaves two renderings of one pane which differ by build rather
   than one pane." A builder kept only for the test is the same object with a
   thinner excuse — one pane, two implementations, and the one nothing paints is
   the one the proof trusts.
2. It drifts in the direction that cannot be caught. Nothing renders the retained
   builder, so a change that makes it wrong makes the *oracle* wrong, and the
   test keeps passing while agreeing about the wrong pane.
3. It keeps 2,000 lines of pane alive to serve one test, which is precisely the
   deletion the handover exists to perform.

## 3. Rejected: a hand-written expected tree in the test

Construct the expected `ViewNode` in the test with the same constructors the
plugin uses.

Rejected: the full case is a 25-row pane with six gauges. A hand-written twin of
it is a second implementation with no user, drifting against nothing — the same
objection as §2 with more typing. It also cannot be *derived* from the native
pane, so it never had a proven baseline at all.

## 4. Rejected: `insta::assert_debug_snapshot!` of the tree

`ViewNode` derives `Debug`, so `{:#?}` is a faithful, zero-effort recording.

Rejected on reviewability. The full case's `{:#?}` is several thousand lines of
nested struct literals, most of it `TextStyle` fields at their defaults. A
reviewer cannot tell a correct snapshot from a subtly wrong one at that size, so
every future `INSTA_UPDATE=always` becomes a rubber stamp — which converts the
oracle into a record of whatever the code last did. An oracle nobody can read is
an oracle nobody checks.

Faithfulness was never the scarce property here; **legibility** was.

## 5. Chosen: a compact line-per-node rendering, snapshotted

One line per node: indentation for depth, the node's kind, its content, and only
the style facts that are *not* default. The full info panel becomes ~40 readable
lines in which a wrong colour role or a lost bold is visible at a glance:

```text
list selected=none scrollbar=false
  line
    text "Session" accent bold
  line
    text "  name " muted
    text "demo"
  gauge "CPU" 42% suffix=none
```

The risk this shape carries is the mirror of §4's: a compact format is compact
because it omits, and an omission is a hole in the oracle. Two rules close it:

- **Exhaustive destructuring.** The formatter matches every `ViewNode` variant and
  binds every field by name, with no `..` rest pattern and no wildcard arm; the
  same for `TextStyle`. Adding a field to the view tree therefore *fails to
  compile* in this file until the oracle decides how to print it. The compiler is
  what keeps the format honest, not a reviewer's memory.
- **Defaults are printed as absence, but absence is total.** A style prints only
  its non-default facts, so the common case stays short — but every fact is
  printed when it is set, which is what the exhaustive match guarantees.

## 6. Chosen: record while both sides exist, and check the recording

The recording step is the part that is easy to get wrong, and getting it wrong is
invisible. If the snapshot is generated *after* the native builder is deleted, it
records the plugin's output; a plugin bug is then frozen as the expectation and
the test passes forever. If it is generated *before*, from the plugin, the same
is true.

So the snapshot is generated from the **native builder**, and while the native
builder still exists the test asserts both edges:

```text
native_tree(case)             == snapshot     (the recording is the native pane's)
plugin_render(context(case))  == native_tree  (unchanged, today's proof)
```

The two together give the transitive fact the handover needs
(`plugin == snapshot`) *and* prove the baseline is the pane's rather than the
plugin's. After the handover deletes `native_tree`, the first assertion goes with
it and the second is rewritten against the snapshot — inheriting a baseline this
change proved, not one it assumed.

This is why the change is worth landing separately from the handover: the
recording has to happen while the thing being recorded is still there, and a
change that both records and deletes cannot show that it recorded the right
thing.

## 7. Why the snapshot lives in `tests/snapshots/`

insta's default for an integration test, and separate from `src/app/snapshots/`
so the acceptance snapshots keep their own directory. Nothing else in `tests/`
uses insta today; this is the first, which is stated in the tasks so the reviewer
knows the directory is new rather than moved.

## 8. What this change deliberately does not do

- It does not touch the other five bundled pane oracles. Each has the same hole,
  and each will need the recording captured while *its* native builder exists —
  which is work belonging to its own handover, not to this one. §2 of the spec
  delta states the rule so the next port cannot miss it.
- It does not delete, hide, or reseat anything. `src/` is untouched, so the
  interface after this change is byte-identical to before it, and the teardown
  gate's verdict for the info panel is unchanged and still blocked.
