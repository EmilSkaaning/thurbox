# A plugin can trim a string the way thurbox trims one

## Why

`tests/session_list_pane_handover_gap.rs` refuses the session list's handover on three
rows, all of them vocabulary since ADR-63 closed the last structural one. This is the
smallest, and it is not about the session list:

> `non-ascii-whitespace-is-the-kernels-trim` — the kernel trims the agent's reported
> activity text with Rust's `str::trim`, which is Unicode-aware. The plugin trims with
> Luau's `%s`, which matches only the six ASCII whitespace characters. A no-break space
> around an activity title therefore survives in the plugin's copy, and the row it draws
> is off by a column against the row it reproduces
> (`non_ascii_whitespace_is_trimmed_by_the_kernel_only`).

The gap is Luau's, not thurbox's. Luau's pattern classes are byte classes: `%s` is
`isspace` over bytes, so U+00A0, U+2007, U+3000 and the rest of `White_Space` are simply
invisible to it. There is no pattern a plugin can write that closes this, and a plugin
that tries — enumerating the code points it happens to know — writes a table that is
wrong the next time Unicode adds one.

The port left it open rather than closing it by publishing the *trimmed* text, on the
rule that the kernel publishes no rendering. That rule is worth keeping: which of a row's
two reported strings is shown, and whether whitespace-only counts as nothing, are the
pane's decisions. What the pane cannot do is the *predicate*. So the closure is the
predicate, not the decision.

## What Changes

- **`thurbox.trim(s)`** — a Unicode-aware trim on the `@thurbox` module table, `str::trim`
  exactly: it removes leading and trailing characters for which `char::is_whitespace`
  holds, which is Unicode's `White_Space` property, and returns what is left.
- **It is ungated**, beside `ui`, and it grants **no capability**. It is a pure function
  of its argument: it reads no kernel state, reaches no host, and cannot fail. `Capability`
  is unchanged and no binding is added under one.
- **The bundled session-list plugin uses it** in place of `string.match(activity,
  "^%s*(.-)%s*$")`, so its row matches the kernel's for every whitespace character rather
  than for six of them.
- **`non-ascii-whitespace-is-the-kernels-trim` closes**, and the enumerated divergence it
  mirrors in `tests/bundled_session_list.rs` becomes its opposite: the two trees are
  asserted **equal** for an activity title padded with a no-break space.

## Non-goals

- **Publishing the trimmed text.** It would close this row by moving a presentation
  decision into the kernel, and it would leave the next pane that wants to trim something
  in the same position.
- **A general string library.** One function, because one function is what a pane cannot
  write. `upper`, `lower`, `len`-in-graphemes and the rest are not blocked by anything
  and are not added on speculation.
- **Trimming inside a node constructor.** A `text` node that trimmed its content would
  make a leading space impossible to draw, and the session list draws several
  deliberately (` ○ `, the tree prefix, the two-space separator).
- **Anything about the other two rows.** The pane's border chrome and the pending-spawn
  placeholder are untouched, and the gate goes on refusing the handover on them.

## Gate

No compile-time gate of its own: the binding is inside `#[cfg(feature = "plugins")]`
code, like every other module binding, and `plugins` is in the default feature set.
