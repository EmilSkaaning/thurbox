--!strict
-- thurbox's unified diff stream, in Luau.
--
-- A faithful reproduction of `ui::code_review::diff_row_tree`: the same
-- `{old} {new} {sign} ` gutter, the same token-by-token colouring of the body,
-- the same insertion/deletion row tint carried to the pane's right edge, and the
-- same selection appearance on the cursor's row. `tests/bundled_code_review.rs`
-- asserts the two view trees are *equal*, and the kernel's builder is itself
-- painted against the untouched native renderer — so this is measured against
-- what thurbox draws today rather than against a refactor of it.
--
-- Three things are worth reading for what they imply about writing a dense pane:
--
-- 1. **The highlighter is here, not in the host.** The host publishes a line's
--    text and its file's path; which runs are keywords, strings, numbers,
--    comments or type names is decided below. That is deliberate: colouring a
--    diff body is presentation, and a pane handed pre-coloured runs would be an
--    arrangement of the kernel's decisions. The cost is that this lexer and
--    `ui::syntax` must agree token for token, which is what the equality test
--    checks.
-- 2. **The tint reaches the edge without a width.** A row's background is what
--    distinguishes an insertion from a deletion — the sign is one character — so
--    the last run of every row is `ui.fill(" ")`, which the *kernel* expands to
--    whatever the line has left. Nothing here knows how wide the pane is.
-- 3. **It scrolls by naming its cursor**, like the file viewer: the list carries
--    the row the review's cursor is on and the host windows it.
--
-- What this pane deliberately does **not** draw: file headers, hunk headers,
-- comments and their classification badges, the review summary, reviewed marks,
-- folding, the find bar, the target picker, the footer and the compose box; and
-- it draws neither the side-by-side layout, the wrap mode nor the horizontal
-- scroll, all three of which need a resolved width. See `plugin.toml`.

local thurbox = require("@thurbox")
local ui = thurbox.ui

type ViewNode = thurbox.ViewNode
type StyleToken = thurbox.StyleToken
type DiffTint = thurbox.DiffTint
type ReviewLine = thurbox.ReviewLine

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
local function gutter(line: ReviewLine, numberWidth: number, sign: string): string
    local pad = "%" .. tostring(numberWidth) .. "s"
    local old = if line.oldNo then tostring(line.oldNo) else ""
    local new = if line.newNo then tostring(line.newNo) else ""
    return string.format(pad .. " " .. pad .. " %s ", old, new, sign)
end

local function rowNode(line: ReviewLine, numberWidth: number, selected: boolean): ViewNode
    local sign, tint = signAndTint(line.kind)
    -- Both roles are declared on every run of the row: the host resolves the
    -- selection first, so the cursor's row draws in one appearance rather than
    -- two backgrounds — the precedence thurbox's own renderer applies.
    local function style(token: StyleToken?): thurbox.TextStyle
        return { token = token, selected = selected, tint = tint }
    end

    local runs: { ViewNode } = { ui.text(gutter(line, numberWidth, sign), style("muted")) }
    for _, piece in highlight(line.text, line.path) do
        table.insert(runs, ui.text(piece.run, style(piece.token)))
    end
    -- The row's background has to reach the pane's edge or an insertion and a
    -- context line read alike. This is the only run whose width is the host's.
    table.insert(runs, ui.fill(" ", style(nil)))
    return ui.line(runs)
end

-- ── the pane ────────────────────────────────────────────────────────────────

return {
    render = function(_paneId: string): ViewNode
        local readReview = thurbox.review
        local review = if readReview then readReview() else nil
        local lines = if review then review.lines else {}

        if #lines == 0 then
            -- The same line the native pane draws for a target with nothing in
            -- it — and what a plugin sees before a review has been opened at
            -- all, since the host publishes the diff the pane has open.
            return ui.list({ ui.text("No changes to show for this target.", "muted") })
        end

        local numberWidth = if review then review.numberWidth else 2
        local cursor = if review then review.cursor else nil

        local rows: { ViewNode } = {}
        for i, line in ipairs(lines) do
            table.insert(rows, rowNode(line, numberWidth, i == cursor))
        end
        -- The cursor's row goes back to the host, which is what makes this list
        -- scroll: nothing here is told how many rows it got.
        return ui.list(rows, cursor)
    end,
}
