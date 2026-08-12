--!strict
-- thurbox's code review, in Luau: the whole document the native view lists.
--
-- A faithful reproduction of `ui::code_review::review_stream_tree`: file headers
-- with their fold chevron, status letter, path, insertion and deletion counts,
-- reviewed mark and the rule that carries the row to the pane's edge; hunk headers
-- with their `@@` ranges and mark; the unified diff lines with their
-- `{old} {new} {sign} ` gutter, token-by-token body colouring and
-- insertion/deletion row tint; comments with their classification badges; the
-- review summary's heading and its comments; and informational rows — each in the
-- selection appearance when the review's cursor is on it.
-- `tests/bundled_code_review.rs` asserts the two view trees are *equal*, and the
-- kernel's builder is itself painted against the untouched native renderer — so
-- this is measured against what thurbox draws today rather than against a refactor
-- of it.
--
-- Four things are worth reading for what they imply about writing a dense pane:
--
-- 1. **The highlighter is here, not in the host.** The host publishes a line's
--    text and its file's path; which runs are keywords, strings, numbers,
--    comments or type names is decided below. That is deliberate: colouring a
--    diff body is presentation, and a pane handed pre-coloured runs would be an
--    arrangement of the kernel's decisions. The cost is that this lexer and
--    `ui::syntax` must agree token for token, which is what the equality test
--    checks.
-- 2. **The glyphs are here too.** The host publishes a file's status as a *name*
--    and whether it is folded and reviewed; `M`, `▸`/`▾` and `✓` are chosen below,
--    exactly as a diff line's `+`/`-` sign is. The one string that crosses whole is
--    the summary heading's, because it names a thurbox keystroke this pane never
--    receives.
-- 3. **Two things reach the pane's edge without a width.** A row's tint is what
--    distinguishes an insertion from a deletion — the sign is one character — and a
--    file header's rule is what separates one file from the next. Both are
--    `ui.fill`, which the *host* expands to whatever the line has left. Nothing
--    here knows how wide the pane is.
-- 4. **It scrolls by naming its cursor**, like the file viewer: the list carries
--    the row the review's cursor is on and the host windows it.
--
-- What this pane deliberately does **not** do is act. It draws the review's
-- document and reaches none of its behaviour: no mark toggled, no comment composed,
-- no target retargeted, no find, and neither the side-by-side layout, the wrap mode
-- nor the horizontal scroll — those three need a resolved width, which is also why
-- an over-long header clips here where thurbox writes an ellipsis. See
-- `plugin.toml`.

local thurbox = require("@thurbox")
local ui = thurbox.ui

type ViewNode = thurbox.ViewNode
type StyleToken = thurbox.StyleToken
type DiffTint = thurbox.DiffTint
type ReviewRow = thurbox.ReviewRow
type ReviewFileRow = thurbox.ReviewFileRow
type ReviewDiff = thurbox.ReviewDiff

-- ── the syntax lexer ────────────────────────────────────────────────────────
--
-- A port of `src/ui/syntax.rs`: a small language-agnostic lexer colouring the
-- things that matter when skimming a diff. Runs of the same colour are not
-- merged, matching the host, because the pane renders one run per token.

-- The same union of keywords across common languages the host carries. A set
-- rather than an array so the lookup is one hash rather than eighty comparisons
-- for every identifier of every visible line.
local KEYWORD_LIST = {
    "fn",
    "let",
    "mut",
    "pub",
    "use",
    "mod",
    "impl",
    "struct",
    "enum",
    "trait",
    "match",
    "if",
    "else",
    "for",
    "while",
    "loop",
    "return",
    "const",
    "static",
    "async",
    "await",
    "move",
    "ref",
    "where",
    "as",
    "dyn",
    "self",
    "Self",
    "crate",
    "super",
    "break",
    "continue",
    "in",
    "def",
    "class",
    "function",
    "var",
    "import",
    "from",
    "export",
    "public",
    "private",
    "protected",
    "void",
    "new",
    "delete",
    "try",
    "catch",
    "finally",
    "throw",
    "throws",
    "extends",
    "implements",
    "interface",
    "package",
    "func",
    "defer",
    "chan",
    "go",
    "range",
    "type",
    "nil",
    "null",
    "none",
    "true",
    "false",
    "and",
    "or",
    "not",
    "is",
    "lambda",
    "yield",
    "with",
    "pass",
    "elif",
    "then",
    "do",
    "begin",
    "module",
    "require",
    "local",
    "val",
    "when",
    "case",
    "switch",
    "default",
    "print",
    "echo",
    "end",
    "unless",
    "until",
}

local KEYWORDS: { [string]: boolean } = {}
for _, word in KEYWORD_LIST do
    KEYWORDS[word] = true
end

-- The line-comment marker for a path's extension, by the host's table. The
-- extension is the trailing run of characters that is neither `.` nor `/`, which
-- is what splitting the path on both separators and taking the last piece means.
local HASH_EXTENSIONS: { [string]: boolean } = {
    py = true,
    rb = true,
    sh = true,
    bash = true,
    zsh = true,
    fish = true,
    toml = true,
    yaml = true,
    yml = true,
    ini = true,
    cfg = true,
    conf = true,
    nix = true,
    pl = true,
    r = true,
    ex = true,
    exs = true,
    rake = true,
    gemspec = true,
}

local DASH_EXTENSIONS: { [string]: boolean } = {
    sql = true,
    lua = true,
    hs = true,
    elm = true,
    adb = true,
    ads = true,
}

local function lineComment(path: string): string
    local ext = string.match(path, "[^./]*$") or ""
    if HASH_EXTENSIONS[ext] then
        return "#"
    elseif DASH_EXTENSIONS[ext] then
        return "--"
    end
    return "//"
end

-- Codepoints, because the host lexes by *character*: scanning bytes would split a
-- multi-byte character and every run after it would be a byte adrift.
local function codepoints(text: string): { number }
    local out: { number } = {}
    for _, code in utf8.codes(text) do
        table.insert(out, code)
    end
    return out
end

local function slice(codes: { number }, from: number, past: number): string
    local parts: { string } = {}
    for i = from, past - 1 do
        table.insert(parts, utf8.char(codes[i]))
    end
    return table.concat(parts)
end

local function isAsciiDigit(c: number): boolean
    return c >= 48 and c <= 57
end

local function isAsciiAlpha(c: number): boolean
    return (c >= 65 and c <= 90) or (c >= 97 and c <= 122)
end

-- The host asks Rust's `char::is_alphabetic`, which is Unicode-aware. A sandboxed
-- VM has no Unicode tables, so anything past ASCII is taken as a letter: exact
-- for the accented identifiers real code contains, and different only for
-- non-ASCII punctuation — recorded as a divergence rather than hidden.
local function isAlphabetic(c: number): boolean
    return isAsciiAlpha(c) or c >= 128
end

local function isAlphanumeric(c: number): boolean
    return isAlphabetic(c) or isAsciiDigit(c)
end

local function isQuote(c: number): boolean
    return c == 34 or c == 39 or c == 96
end

-- Whether `codes` at `i` begins with the marker `pat` (also codepoints).
local function startsWithAt(codes: { number }, i: number, pat: { number }): boolean
    if #pat == 0 or i + #pat - 1 > #codes then
        return false
    end
    for k = 1, #pat do
        if codes[i + k - 1] ~= pat[k] then
            return false
        end
    end
    return true
end

-- One past a string literal opened at `i`, honouring backslash escapes and
-- clamping to the line's end on an unterminated string.
local function scanString(codes: { number }, i: number): number
    local quote = codes[i]
    local j = i + 1
    while j <= #codes do
        if codes[j] == 92 then
            j += 2
        elseif codes[j] == quote then
            return math.min(j + 1, #codes + 1)
        else
            j += 1
        end
    end
    return #codes + 1
end

-- One past a number run (hex `0x…`, floats, digit separators).
local function scanNumber(codes: { number }, i: number): number
    local j = i
    while j <= #codes do
        local c = codes[j]
        if not (isAsciiAlpha(c) or isAsciiDigit(c) or c == 46 or c == 95) then
            break
        end
        j += 1
    end
    return j
end

-- One past an identifier or keyword run.
local function scanWord(codes: { number }, i: number): number
    local j = i
    while j <= #codes and (isAlphanumeric(codes[j]) or codes[j] == 95) do
        j += 1
    end
    return j
end

-- One past a run of operators, punctuation and whitespace, stopping at the next
-- token start. Always advances at least one character.
local function scanPlain(codes: { number }, i: number, comment: { number }): number
    local j = i
    while j <= #codes do
        local cj = codes[j]
        if isQuote(cj) or isAlphanumeric(cj) or cj == 95 or startsWithAt(codes, j, comment) then
            break
        end
        j += 1
    end
    if j == i then
        return i + 1
    end
    return j
end

-- The role an identifier takes: keyword, capitalised type, or plain — and plain
-- means *no* token, which is the theme's primary foreground.
local function wordToken(codes: { number }, from: number, past: number): StyleToken?
    local word = slice(codes, from, past)
    if KEYWORDS[word] then
        return "accent"
    elseif codes[from] >= 65 and codes[from] <= 90 then
        return "accent_bright"
    end
    return nil
end

type Token = { run: string, token: StyleToken? }

local function highlight(text: string, path: string): { Token }
    local codes = codepoints(text)
    local comment = codepoints(lineComment(path))
    local out: { Token } = {}
    local i = 1
    while i <= #codes do
        -- A line comment swallows the marker and everything after it.
        if startsWithAt(codes, i, comment) then
            local rest: Token = { run = slice(codes, i, #codes + 1), token = "muted" }
            table.insert(out, rest)
            break
        end
        local c = codes[i]
        local past: number
        local token: StyleToken? = nil
        if isQuote(c) then
            past = scanString(codes, i)
            token = "branch"
        elseif isAsciiDigit(c) then
            past = scanNumber(codes, i)
            token = "status_working"
        elseif isAlphabetic(c) or c == 95 then
            past = scanWord(codes, i)
            token = wordToken(codes, i, past)
        else
            past = scanPlain(codes, i, comment)
        end
        local piece: Token = { run = slice(codes, i, past), token = token }
        table.insert(out, piece)
        i = past
    end
    return out
end

-- ── rows ────────────────────────────────────────────────────────────────────

-- The sign and the tint role for a line's kind. A context line carries no tint at
-- all, which is why the field is absent rather than a third value.
local function signAndTint(kind: string): (string, DiffTint?)
    if kind == "add" then
        return "+", "added"
    elseif kind == "del" then
        return "-", "removed"
    end
    return " ", nil
end

-- `{old} {new} {sign} `, each number right-aligned in its own column. An absent
-- number is an empty column, not a zero: a deletion has no new-side number.
local function gutter(row: ReviewRow, numberWidth: number, sign: string): string
    local pad = "%" .. tostring(numberWidth) .. "s"
    local old = if row.oldNo then tostring(row.oldNo) else ""
    local new = if row.newNo then tostring(row.newNo) else ""
    return string.format(pad .. " " .. pad .. " %s ", old, new, sign)
end

-- A style builder for one row: every run of the row declares the row's selection
-- and tint, because the host resolves the selection first — the cursor's row draws
-- in one appearance rather than two backgrounds, which is the precedence thurbox's
-- own renderer applies.
local function styler(selected: boolean, tint: DiffTint?): (StyleToken?) -> thurbox.TextStyle
    return function(token: StyleToken?): thurbox.TextStyle
        return { token = token, selected = selected, tint = tint }
    end
end

local function lineRow(row: ReviewRow, numberWidth: number, selected: boolean): ViewNode
    local sign, tint = signAndTint(row.kind or "context")
    local style = styler(selected, tint)

    local runs: { ViewNode } = { ui.text(gutter(row, numberWidth, sign), style("muted")) }
    for _, piece in highlight(row.text or "", row.path or "") do
        table.insert(runs, ui.text(piece.run, style(piece.token)))
    end
    -- The row's background has to reach the pane's edge or an insertion and a
    -- context line read alike. This is the only run whose width is the host's.
    table.insert(runs, ui.fill(" ", style(nil)))
    return ui.line(runs)
end

-- The letter and the colour role for a file's status. The host publishes the
-- status as a *name*; which letter stands for it is this pane's decision, exactly
-- as a diff line's `+`/`-` sign is.
local function fileStatus(status: string?): (string, StyleToken)
    if status == "added" then
        return "A", "diff_added"
    elseif status == "deleted" then
        return "D", "diff_removed"
    elseif status == "renamed" then
        return "R", "accent"
    elseif status == "modified" then
        return "M", "status_working"
    end
    return "?", "status_working"
end

-- A file header: the fold chevron, the status letter, the path, the two counts,
-- the reviewed mark, and a rule to the pane's edge.
--
-- The rule is the second `ui.fill` in this pane and the reason a file header is
-- expressible at all: thurbox repeats `─` across whatever the row has left, and a
-- fill is the host resolving that residue for a pane never told its width. The
-- single space before it is a run of its own so both panes spend the same column
-- on it.
local function fileRow(row: ReviewRow, selected: boolean): ViewNode
    local style = styler(selected, nil)
    local function header(token: StyleToken?): thurbox.TextStyle
        local s = style(token)
        s.bold = true
        return s
    end
    local letter, statusToken = fileStatus(row.status)
    local chevron = if row.folded then "▸" else "▾"
    local runs: { ViewNode } = {
        ui.text(chevron .. " ", header("accent_bright")),
        ui.text(letter, header(statusToken)),
        ui.text(" " .. (row.path or "") .. "  ", header("accent_bright")),
        ui.text("+" .. tostring(row.added or 0), style("diff_added")),
        ui.text(" -" .. tostring(row.removed or 0), style("diff_removed")),
    }
    if row.reviewed then
        table.insert(runs, ui.text(" ✓", style("status_done")))
    end
    table.insert(runs, ui.text(" ", header("accent_bright")))
    table.insert(runs, ui.fill("─", header("accent_bright")))
    return ui.line(runs)
end

-- A hunk header: the `@@` ranges, the section heading, and the reviewed mark.
local function hunkRow(row: ReviewRow, selected: boolean): ViewNode
    local mark = if row.reviewed then " ✓" else ""
    local text = string.format(
        "  @@ -%d,%d +%d,%d @@ %s%s",
        row.oldStart or 0,
        row.oldSpan or 0,
        row.newStart or 0,
        row.newSpan or 0,
        row.heading or "",
        mark
    )
    local style = styler(selected, nil)("accent")
    style.dim = true
    return ui.text(text, style)
end

-- A comment's badge label and colour role, by classification.
local function commentClass(classification: string?): (string, StyleToken)
    if classification == "issue" then
        return "Issue", "danger"
    elseif classification == "suggestion" then
        return "Suggestion", "accent"
    elseif classification == "praise" then
        return "Praise", "status_done"
    end
    return "Note", "secondary"
end

-- A comment: its classification badge, then the first line of its body. No fill —
-- thurbox does not pad this row either, so the columns past the body keep the
-- terminal's own style.
local function commentRow(row: ReviewRow, selected: boolean): ViewNode
    local style = styler(selected, nil)
    local label, token = commentClass(row.classification)
    local badge = style(token)
    badge.bold = true
    local more = if row.more then " …" else ""
    return ui.line({
        ui.text("  ▸ [" .. label .. "] ", badge),
        ui.text((row.text or "") .. more, style("secondary")),
    })
end

-- The summary heading. Its text is the host's, verbatim: it names a thurbox
-- keystroke this pane never receives, so composing it here would advertise an
-- action the pane cannot perform.
local function summaryRow(row: ReviewRow, selected: boolean): ViewNode
    local style = styler(selected, nil)("accent")
    style.bold = true
    return ui.text(row.label or "", style)
end

-- An informational row, never drawn selected: thurbox applies no selection style
-- to one, because a cursor cannot rest on a row that is not selectable.
local function infoRow(row: ReviewRow): ViewNode
    return ui.text(row.text or "", "muted")
end

local function rowNode(row: ReviewRow, numberWidth: number, selected: boolean): ViewNode
    if row.row == "file" then
        return fileRow(row, selected)
    elseif row.row == "hunk" then
        return hunkRow(row, selected)
    elseif row.row == "comment" then
        return commentRow(row, selected)
    elseif row.row == "summaryHeader" then
        return summaryRow(row, selected)
    elseif row.row == "info" then
        return infoRow(row)
    end
    return lineRow(row, numberWidth, selected)
end

-- ── the changed-files list ──────────────────────────────────────────────────
--
-- The review's *second* list, and a second pane: thurbox draws it in the
-- file-viewer column while a review is open, with its own cursor — the file the
-- diff's cursor is in — and its own keys. Nothing here derives it from the diff's
-- rows: that stream is windowed by the host and ordered by the review, while this
-- one is every changed file, grouped by directory.

local function folderRow(row: ReviewFileRow): ViewNode
    local style: thurbox.TextStyle = { token = "muted", bold = true, ellipsize = true }
    return ui.text(string.rep("  ", row.depth) .. (row.name or "") .. "/", style)
end

local function fileRowNode(row: ReviewFileRow, current: boolean): ViewNode
    local base: thurbox.TextStyle = { bold = current, selected = current }
    -- The selection appearance owns the whole row: a tinted glyph on a
    -- highlighted line is two backgrounds' worth of decision, and thurbox's rule
    -- is that the cursor's row is one appearance.
    local function tint(token: StyleToken): thurbox.TextStyle
        return { token = if current then nil else token, bold = current, selected = current }
    end
    local path = row.path or ""
    local name = string.match(path, "[^/]*$") or path
    local mark = if row.reviewed then "✓ " else "  "
    -- The same derivation a file header makes, from the status *name* the host
    -- publishes: one table, so the two rows cannot disagree about a rename.
    local letter, statusToken = fileStatus(row.status)
    return ui.line({
        ui.text(string.rep("  ", row.depth) .. mark, base),
        ui.text(letter, tint(statusToken)),
        ui.text(" " .. name .. "  ", base),
        ui.text("+" .. tostring(row.added or 0), tint("diff_added")),
        ui.text(" -" .. tostring(row.removed or 0), tint("diff_removed")),
    })
end

local function filesPane(review: ReviewDiff?): ViewNode
    local rows = if review then review.files else {}
    if #rows == 0 then
        -- Nothing to list, and nothing to say about it: the native column draws
        -- an empty box for a review with no files, so this one does too.
        return ui.list({})
    end
    local cursor = if review then review.fileCursor else nil
    local nodes: { ViewNode } = {}
    for i, row in ipairs(rows) do
        if row.row == "folder" then
            table.insert(nodes, folderRow(row))
        else
            table.insert(nodes, fileRowNode(row, i == cursor))
        end
    end
    -- The window opens at the top when the diff's cursor is on a row belonging to
    -- no file (the summary section), which is what naming the first row does.
    return ui.list(nodes, cursor or 1)
end

-- ── the panes ───────────────────────────────────────────────────────────────

return {
    render = function(paneId: string): ViewNode
        local readReview = thurbox.review
        local review = if readReview then readReview() else nil
        if paneId == "files" then
            return filesPane(review)
        end
        local rows = if review then review.rows else {}

        if #rows == 0 then
            -- The same line the native pane draws for a target with nothing in
            -- it — and what a plugin sees before a review has been opened at
            -- all, since the host publishes the review the pane has open.
            return ui.list({ ui.text("No changes to show for this target.", "muted") })
        end

        local numberWidth = if review then review.numberWidth else 2
        local cursor = if review then review.cursor else nil

        local nodes: { ViewNode } = {}
        for i, row in ipairs(rows) do
            table.insert(nodes, rowNode(row, numberWidth, i == cursor))
        end
        -- The cursor's row goes back to the host, which is what makes this list
        -- scroll: nothing here is told how many rows it got.
        return ui.list(nodes, cursor)
    end,
}
