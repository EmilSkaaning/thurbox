# Design — a Unicode-aware trim a plugin can reach

## Where it goes, and what that costs

`module.set("trim", …)` in `plugin::capabilities::build_module_table`, beside
`module.set("ui", …)` and above the `set_readonly(true)` that freezes the table.

Ungated for `ui`'s stated reason: it builds nothing and reaches nothing, so hiding it
behind a capability would be theatre. The test that matters is whether the binding lets a
plugin observe or affect anything outside its own argument, and it does not — it is
`str::trim` over a `String` the VM already holds. `Capability` gains no variant, no
existing grant gains a binding, and `GrantedCapabilities` is untouched.

That the file is called `capabilities.rs` is not the same as the function granting one.
`ui.center` landed in the same file one change ago under the same argument, and
`tests/session_list_pane_handover_gap.rs`'s `a_view_write_binding_exists` reads the whole
binding list: `trim` is neither a view verb nor a mutator naming a view noun, so the gate
that watches for a widening goes on answering "none".

## Why the predicate rather than the answer

Three ways to close the row, and only one of them leaves the rule intact.

| | What it does | Why not |
|---|---|---|
| Publish `activity` already trimmed | The kernel decides what the row shows | It moves a presentation decision into the publication, which is the rule the port kept. And the next pane that wants a trim is no better off. |
| A `trim` flag on the `text` constructor | The node trims its own content | A leading space becomes undrawable, and this pane draws several deliberately — the status glyph's padding, the tree prefix, the two-space separator. |
| **`thurbox.trim(s)`** | The plugin decides, with the kernel's predicate | The decision stays the pane's and the predicate stops being Luau's. |

## Exactly `str::trim`, and the test says so by construction

The binding is `Ok(s.trim().to_string())`. Not a hand-rolled loop over a whitespace set:
the whole point is that the plugin's answer and the kernel's are the *same* answer, and
two implementations of "whitespace" are how they stopped being the same in the first
place. `session::session_list::agent_status_text` calls `str::trim`; so does this.

The property test asserts the identity rather than a sample: for each character in a table
spanning ASCII whitespace, the Unicode separators Luau cannot see (U+00A0, U+2007,
U+202F, U+2028, U+3000), and a few non-whitespace controls that must **not** be trimmed,
padding a word with it and calling the binding gives what `str::trim` gives.

## What Luau's `%s` actually is

Worth recording, because the row reads as a rounding error until you know. Luau's pattern
classes are C `ctype` predicates applied to *bytes*, so `%s` is exactly
`{' ', '\t', '\n', '\v', '\f', '\r'}`. A UTF-8 no-break space is the two bytes `C2 A0`,
neither of which is in that set, so `"^%s*(.-)%s*$"` returns the string unchanged. No
pattern closes it — `[%s\194\160]` would close it for *one* code point and be wrong for
the twenty-odd others, and wrong again whenever Unicode adds one.

## The divergence inverts, and the fixture has to keep biting

`non_ascii_whitespace_is_trimmed_by_the_kernel_only` asserts `assert_ne!`. It becomes
`assert_eq!` under a new name — and it keeps a guard that the fixture is still a fixture:
the padded activity must still differ from its trimmed form, or the case would pass by
exercising nothing. That is the same shape the window divergence took when it closed one
change ago (`the_two_panes_window_a_long_list_by_one_rule` asserts its list actually
overflows).

## Rejected: leaving it to the handover

The row is small enough to fold into the handover, and that is the argument against it. A
handover claims that which code draws a pane changed and nothing else did; a commit that
also adds a host binding and moves a recording gives a reviewer two things to attribute.
The same reason the frame converges first (ADR-53) and the window converged first
(ADR-63).
