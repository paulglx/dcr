# dcr — Specs

`dcr` is a TUI for inspecting DICOM files.

---

## 1. Invocation & CLI arguments

`dcr` is launched from the command line and supports three usage forms:

| Command                                      | Resulting mode                           |
| -------------------------------------------- | ---------------------------------------- |
| `dcr` (no positional argument)               | **Explorer mode** — opens a file browser |
| `dcr <FILE>`                                 | **Direct mode** — opens the single file  |
| `dcr -d <BASELINE> <MODIFIED>` (or `--diff`) | **Diff mode** — compares two files       |

- `--diff` takes **exactly two** path arguments (baseline first, modified second). Supplying a
  different count is an error.
- `-c` / `--changes-only` (requires `--diff`) starts the diff with the **changes-only** filter
  enabled (§7.2). It is a plain flag with no value.
- `--version`, `--help`, and the author/about text are provided automatically by the argument
  parser.

### DICOM file pre-validation

Files passed on the command line (the `<FILE>` argument, and **both** `--diff` paths) are
validated before the UI starts. Validation fails — printing an error and exiting non-zero —
when:

- the path does not exist → `file not found: <path>`
- the path is not a regular file → `not a file: <path>`
- the file is shorter than 132 bytes, or bytes 128–131 are not the ASCII magic `DICM`
  → `not a valid DICOM file: <path>`

> **Note:** This pre-check is applied only to command-line paths. Files opened from within the
> in-app file explorer are **not** pre-validated with the `DICM` magic check — they are simply
> opened, and failures are handled gracefully (see §10).

### Terminal lifecycle

On startup the app enables raw mode, switches to the alternate screen, and enables mouse
capture. It probes the terminal's image capability once at startup (this determines whether
image preview is available; see §8). On exit — including on error — it disables raw mode,
leaves the alternate screen, disables mouse capture, and restores the cursor. If the run loop
returns an error, the app prints `Error: <error>` to stderr and exits with a non-zero status.

---

## 2. Application modes & startup state

There are two top-level modes and two possible focus targets:

- **Modes:** `Direct` (single file or diff) and `Explorer` (file browser).
- **Focus:** `Explorer` (the file tree) or `TagTable` (the DICOM tag list).

Startup state by invocation:

| Invocation   | Mode     | Focus    | File loaded       | Preview                 |
| ------------ | -------- | -------- | ----------------- | ----------------------- |
| `dcr`        | Explorer | Explorer | no                | shown (when applicable) |
| `dcr <FILE>` | Direct   | TagTable | yes               | shown                   |
| `dcr -d a b` | Direct   | TagTable | yes (diff result) | shown                   |

Preview visibility defaults to **on** in all modes, but the preview pane is only laid out when
relevant (see §8 and §9).

---

## 3. Event loop

The app redraws, then blocks on a message channel fed by two threads: an input thread
forwarding terminal events, and the preview decoder signalling a finished decode (see §8).
Only key **press** events are handled (key release/repeat events are ignored). All messages
already queued are applied before the next redraw, so a burst of scroll events costs one
frame. If the selected file's preview is still decoding after the events are applied, the loop
waits up to **8 ms** for it so the image usually lands in the same frame as the selection
change. The app exits the loop and quits when a quit action sets the quit flag.

---

## 4. Keyboard & mouse controls

### 4.1 Mouse (all modes)

- **Scroll wheel down / up:** scrolls the tag table by **3 rows**.
- **Left click:**
  - In **Explorer mode**: if a DICOM is loaded and the click lands inside the tag-table area,
    focus moves to the tag table; otherwise the click is routed to the file explorer (see §10).
  - In **Direct mode**, or when the tag table is focused: a click inside the tag rows selects
    the clicked row. Row resolution accounts for the table border, the header row, and the
    current scroll offset; clicks outside the populated rows are ignored.

### 4.2 Direct mode keys

When the search input is **inactive**:

| Key         | Action                                                 |
| ----------- | ------------------------------------------------------ |
| `q` / `Esc` | If a search filter is active, clear it; otherwise quit |
| `↓` / `j`   | Move selection down 1                                  |
| `↑` / `k`   | Move selection up 1                                    |
| `→` / `l`   | Expand the selected tag (if expandable)                |
| `←` / `h`   | Collapse the nearest expanded parent                   |
| `/`         | Open the search input                                  |
| `p`         | Toggle the image preview pane                          |
| `c`         | Diff mode only: toggle the changes-only filter (§7.2)  |
| `t`         | DICOMDIR only: toggle directory view ↔ raw tags (§11)  |
| `Enter`     | DICOMDIR only: open the selected record's file (§11)   |

`Esc` (and `q` when no filter is active) quits, except that `Esc` first **returns to the
DICOMDIR** when the current file was opened from one (§11.5).

When the search input is **active**:

| Key                   | Action                                                |
| --------------------- | ----------------------------------------------------- |
| _printable character_ | Append to the query and re-filter live                |
| `Backspace`           | Remove the last character and re-filter               |
| `Enter`               | Confirm — close the input but keep the current filter |
| `Esc`                 | Cancel — close the input and clear the filter         |

### 4.3 Explorer mode — Explorer focus

| Key                                    | Action                                                     |
| -------------------------------------- | ---------------------------------------------------------- |
| `q`                                    | Quit                                                       |
| `Esc`                                  | Exit diff mode if active; else clear a pending diff mark if any; else quit |
| `Tab`                                  | Switch focus to the tag table (only if a DICOM is loaded)  |
| `p`                                    | Toggle the image preview pane                              |
| `a`                                    | Mark the highlighted file as diff **baseline** (A) — see §7.1 |
| `b`                                    | Mark the highlighted file as diff **modified** (B) — see §7.1 |
| `x`                                    | In diff mode: swap A ↔ B and re-diff                       |
| `c`                                    | In diff mode: toggle the changes-only filter (§7.2)        |
| `s`                                    | Open the sort menu (§10.1)                                 |
| `↑` / `k`, `↓` / `j`                   | Move within the file tree                                  |
| `→` / `l` / `Enter`                    | Enter the selected directory, or select the file           |
| `←` / `h`                              | Go to the parent directory                                 |
| `Home` / `End` / `PageUp` / `PageDown` | Standard list navigation                                   |

After any navigation, the currently highlighted entry is evaluated: if it is a file it is
loaded (see §10); otherwise any loaded DICOM state is cleared. Pressing `Enter` on a file also
moves focus to the tag table once it has loaded. **Exception:** while interactive diff mode is
active (§7.1), navigation only moves the highlight — nothing is loaded or cleared, and `Enter`
does not move focus to the tag table.

### 4.4 Explorer mode — TagTable focus

Search behaves identically to Direct mode (§4.2). Otherwise:

| Key                  | Action                                                                 |
| -------------------- | ---------------------------------------------------------------------- |
| `Tab`                | Return focus to the explorer                                           |
| `Esc`                | Clear the active filter if any; otherwise return focus to the explorer |
| `q`                  | Quit                                                                   |
| `↓` / `j`, `↑` / `k` | Move selection down / up 1                                             |
| `→` / `l`            | Expand the selected tag                                                |
| `←` / `h`            | Collapse the nearest expanded parent                                   |
| `/`                  | Open the search input                                                  |
| `p`                  | Toggle the image preview pane                                          |
| `t`                  | DICOMDIR only: toggle directory view ↔ raw tags (§11)                  |
| `Enter`              | DICOMDIR only: open the selected record's file (§11)                   |

`Esc` with no filter active returns to the DICOMDIR if the file was opened from one (§11.5),
otherwise returns focus to the explorer. While a DICOMDIR directory view is shown, the
navigation keys (`↑/↓`, `→/←`, `/`, mouse) act on the directory tree instead of the tag table.

---

## 5. Tag tree

### Loading & ordering

Tags are read from the file in two passes: the **file-meta group first**, then the main
dataset, preserving each section's natural order.

### Per-tag formatting

- **Tag id:** `(GGGG,EEEE)` — group and element as uppercase 4-digit hex.
- **Name:** the standard DICOM dictionary alias for the tag, or empty if the tag is unknown.
- **VR:** the element's value representation string.
- **Value:** rendered as text. Specifically:
  - A primitive value is rendered as a string; if it cannot be stringified the value is
    `<Error>`.
  - A pixel-data fragment sequence is rendered as `<Pixel Data Sequence>`.
  - Any other non-primitive is rendered as `<Unknown>`.
  - Values are **truncated to 256 characters**, with a trailing `...` when truncated.

### Sequences (nesting)

Sequence (SQ) elements that contain items are **expandable**:

- The sequence row's value reads `<Sequence with N item(s)>`.
- Its children are item rows labeled `Item #k` (1-based) whose value reads `<M element(s)>`.
  When an item is a DICOMDIR directory record (it carries `(0004,1430)` Directory Record
  Type), its **Name** column shows a summary of the record instead of being empty, e.g.
  `PATIENT · Doe John`, `STUDY · 2024-03-12 · CT ABDOMEN`, `SERIES · #11 CT`,
  `IMAGE · #17 IM000017` (see §11.2 for the key per record type).
- Each item's contents are extracted recursively, increasing depth at each level.

### Tree presentation

- Only the children of **expanded** nodes are visible.
- Depth drives indentation of **2 spaces per level**.
- An expand indicator precedes the tag id: `▶` when collapsed, `▼` when expanded, and two
  spaces for leaf (non-expandable) rows.
- When a non-empty tree is loaded, the first row (index 0) is selected. Scrolling clamps to the
  valid range.
- **Expand** opens the selected node if it is expandable and currently collapsed.
- **Collapse** finds the nearest expanded ancestor of the selected row, collapses it, and moves
  the selection to that ancestor.

### Private tags

A tag with an **odd group number** is a private tag. In non-diff mode, private tag rows are
rendered in dark gray.

---

## 6. Search & filtering

- Activated with `/`. The query matches **case-insensitively** as a substring against either
  the tag id or the tag name.
- An empty query clears the filter and restores the full tree.
- Applying a filter rebuilds the visible tree from the matching tags and resets the selection
  to the first row.
- While the search input is active, the bottom help line is replaced by the prompt
  `/<query>_` rendered in yellow.

---

## 7. Diff mode

Diff mode compares a baseline file against a modified file, matching tags by id at the
**top level**. Each resulting tag is assigned a status:

| Status      | Meaning                           |
| ----------- | --------------------------------- |
| `Added`     | Present only in the modified file |
| `Deleted`   | Present only in the baseline file |
| `Changed`   | Present in both, values differ    |
| `Unchanged` | Present in both, values equal     |

The combined result is **sorted by tag id**.

### Rendering

- A leading status column is shown: `+` (green) for added, `-` (red) for deleted, `M` (blue)
  for changed, and blank for unchanged.
- Added rows are colored green; deleted rows red.
- **Changed** rows display a **word-level inline diff** of baseline → modified: deleted words
  are red, struck-through, on a dark-gray background; inserted words are green and bold;
  unchanged words use the default style.
- For date/time value representations (`DA`, `TM`, `DT`), a delta suffix is appended to a
  changed value: `(+N ms)` when later, `(N ms)` when earlier, where `N` is the difference in
  milliseconds. The delta is computed from the **first component** of multi-valued
  (backslash-separated) values:
  - `DA` parsed as `%Y%m%d`; delta expressed in milliseconds across whole days.
  - `TM` parsed as `%H%M%S` (or `%H%M%S%.f` with a fractional part).
  - `DT` parsed as `%Y%m%d%H%M%S` (or with `%.f`).
  - If either side fails to parse, no suffix is added.

### Title

- Diff mode: `DICOM Diff: <baseline name> ↔ <modified name>`.
- Single-file mode: `DICOM Viewer: <file name>`.

### Preview & validation in diff mode

In diff mode (CLI and interactive), the validation pane reflects the **modified** file (B) —
the file under investigation. The preview pane shows a **visual image diff** of both files
(see §8.1).

### 7.1 Interactive diff (Explorer mode)

Diff mode can be entered from the file explorer without restarting, using two **marks**:

- With the explorer focused, `a` marks the highlighted file as **baseline (A)** and `b` marks
  it as **modified (B)**. Marks are order-independent: either may be set first.
- The diff opens as soon as **both** marks are set, replacing the right-hand pane with the
  diff table (rendered exactly as CLI diff mode, §7). Focus stays on the explorer.
- Re-pressing `a` (or `b`) on a different file **moves** that mark; if a diff is open it is
  recomputed immediately with the new pair. This is how diffs are navigated.
- Marking the file already carrying the **other** mark is rejected with a status message
  (`Already marked as A` / `Already marked as B`). Marking a directory is rejected with
  `Only files can be marked for diff`.
- `x` (diff open only) swaps baseline and modified, and re-diffs.
- If computing a diff fails (e.g. an unreadable file), a `Diff failed: <error>` status message
  is shown and the current view is left unchanged; the marks are kept.

**While a diff is open**, explorer navigation is browse-only: moving the highlight does not
load or clear files (§4.3), so the diff persists while the user picks new targets for `a`/`b`.
The tag table behaves as usual (focusable via `Tab`/click, searchable, expandable).

**Exiting.** `Esc` with the explorer focused follows this precedence:

1. A diff is open → exit diff mode: both marks are cleared and **file A is re-opened** as a
   normal single file.
2. A mark is pending (only one of A/B set) → clear the pending mark.
3. Otherwise → quit.

`q` always quits directly. `Esc` with the tag table focused keeps its usual behavior (clear
filter, else return focus to the explorer).

**Feedback.**

- A marked file shows an `A` or `B` badge at the right edge of its row in the file list,
  padded with one space on each side. Badges are colored to indicate the diffing direction,
  matching the diff colors of §7: `A` (baseline) is **red**, `B` (modified) is **green**;
  both bold.
- While exactly one mark is set, the help line is replaced by a yellow hint, e.g.
  `A: <file> — press 'b' on another file to diff | Esc: Cancel` (mirrored for a pending B).
- Rejection / failure status messages are rendered in yellow on the help line and cleared on
  the next key press.

Search filters, preview toggling, and mouse behavior are unaffected; an active search filter
is re-applied to the diff result when a diff opens or is recomputed. Interactive diff exists
only in Explorer mode; CLI diff mode (`dcr -d`) is unchanged and has no marks.

### 7.2 Changes-only filter

While a diff is open, `c` toggles a **changes-only** filter that hides every row whose status
is `Unchanged`, leaving only `Added`, `Deleted` and `Changed` rows. It is off by default;
`dcr -d -c` (or `--changes-only`) starts CLI diff mode with it on.

- `c` works in Direct mode and, in Explorer mode, with either the explorer or the tag table
  focused. Outside diff mode the key does nothing.
- The filter composes with the search filter (§6): a row is visible only if it matches both.
  Toggling it rebuilds the visible tree and resets the selection to the first row, like a
  search. Rows that carry no diff status (children of a top-level sequence) are never hidden
  by this filter, so an expanded changed sequence still shows all of its items.
- The filter is kept while the diff is re-marked or swapped (`a`/`b`/`x`) and is reset when
  diff mode is exited.
- While active, the tag table title reads `DICOM Diff: <baseline> ↔ <modified> [changes only]`.
- Help lines in diff mode include `c: Changes only` (§9).

---

## 8. Image preview

- Toggled with `p`; **on by default**.
- Requires a **Kitty-compatible terminal**. If the terminal lacks the capability, the pane
  shows `Preview requires a Kitty-compatible terminal`.
- When available, the preview decodes the file's pixel data and renders **frame 0** as an
  8-bit grayscale (or RGB) image.
- **Decoding** runs on a background thread; the UI never blocks on it. Only the most recent
  request is served (intermediate selections from a fast scroll are dropped). Results are kept
  in a **cache** of the last 32 files (invalidated when a file's size or mtime changes), and the
  files around the selection (3 rows above and below in the DICOMDIR view, 3 files above and
  below in the explorer) are **prefetched** so stepping through them shows the image instantly.
- **Transport:** the image is sent to the terminal once, as a fast-compressed **PNG** through
  the Kitty graphics protocol (wrapped in tmux passthrough when under tmux), as a virtual
  placement over a fixed cell grid that the terminal scales to. The previous image is deleted
  from the terminal in the same write.
- **Scaling:** the cell grid is the largest one inside the pane with the image's aspect ratio.
  Images smaller than the grid's pixel size are sent at **native resolution** and scaled up by
  the terminal (never resampled by the app); larger images are shrunk (box filter) to the grid's
  pixels first. The image is letterboxed with black to the grid's exact aspect ratio, so the
  terminal's stretch onto the grid introduces no distortion. The encoded image is rebuilt only
  when the pane area changes (e.g. terminal resize, layout toggle).
- Error states (shown in red within the pane):
  - `No DICOM file path available` — no file path to decode.
  - `Failed to open DICOM file: <error>`
  - `Failed to decode pixel data: <error>`
  - `Failed to convert to image: <error>`
- While a decode is in progress, the pane shows `Decoding...`. Selecting a file clears the
  current image and requests the new one immediately (served from the cache when possible).

### 8.1 Visual image diff (diff mode)

While a diff is open (CLI `-d` or interactive marks), the preview pane renders a **red/green
composite** of the two images instead of a single file:

- Both files are decoded through the normal preview pipeline (frame 0, each file's own
  VOI LUT / windowing applied) and converted to 8-bit grayscale.
- The composite is built per pixel as `RGB(A, B, min(A, B))`, where `A` is the baseline
  luminance and `B` the modified luminance. Consequences:
  - pixels **equal** in both files render as neutral **gray** (shared anatomy stays visible);
  - pixels **brighter in B** (content added to the modified file) tint **green**;
  - pixels **brighter in A** (content removed from the modified file) tint **red**;
  - tint intensity scales with the magnitude of the difference — no thresholds.
- The pane title carries a legend: ` Preview — A only: red · B only: green ` (colored to
  match). Outside diff mode the title is ` Preview ` as before.
- If the two images have **different dimensions**, the pane shows the error
  `Image dimensions differ: <W>×<H> (A) vs <W>×<H> (B)` — images are never resized for
  diffing. Any per-file decode failure surfaces the corresponding §8 error message.
- All other §8 behavior (toggle with `p`, Kitty requirement, 100 ms debounce, `Decoding...`)
  is unchanged. Exiting diff mode returns the preview to the single re-opened file.

> **Known limitation:** because each file is windowed with its **own** VOI LUT before
> comparison, two files whose raw pixels are identical but whose window settings differ will
> show a global tint. Comparing against a shared normalization is a possible future
> refinement.

---

## 9. Layout & rendering

Focus is indicated by border color: the **focused** pane has a **green** border, unfocused
panes a **dark-gray** border.

### Direct mode

- If the preview is shown, the screen is split **horizontally 50 / 50**: tag table on the
  left, preview on the right. If the preview is hidden, the tag area uses the full width.
- The left/main column is split **vertically** into the tag table (minimum height 5) above and
  the validation pane below.
- A context-sensitive help line is drawn on the bottom row of the tag area:
  `↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview | q/Esc: Quit`
  - In diff mode: `↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | c: Changes only | p: Preview | q/Esc: Quit`

### Explorer mode

- The screen is split **horizontally 25 / 75**: file explorer on the left, content on the
  right. The explorer title shows the current working directory.
- When a DICOM is loaded **and** the preview is shown, the left column reserves a **14-row**
  preview pane below the explorer.
- Right side:
  - With a file loaded: the tag table (minimum height 5) above the validation pane.
  - With no file loaded: a placeholder pane titled `DICOM Viewer` reading
    `Select a DICOM file to view its tags`.
- Help line (on the bottom border of the right-hand pane — the content pane, or the
  placeholder when no file is loaded), depending on focus and whether a file is loaded:
  - Explorer focus, no file: `↑/↓: Navigate | →: Enter dir | ←: Parent dir | a/b: Mark diff | s: Sort | q: Quit`
  - Explorer focus, file loaded: `↑/↓: Navigate | →: Enter | ←: Back | Tab: Tags | a/b: Mark diff | s: Sort | p: Preview | q: Quit`
  - Explorer focus, diff open: `↑/↓: Navigate | a/b: Re-mark | x: Swap | c: Changes only | s: Sort | Tab: Tags | Esc: Exit diff | q: Quit`
  - TagTable focus: `Tab/Esc: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview`
  - TagTable focus, diff open: `Tab/Esc: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | c: Changes only | p: Preview`
  - TagTable focus, DICOMDIR directory view: `Tab: Explorer | ↑/↓: Navigate | →/←: Expand/Collapse | Enter: Open file | t: Raw tags | /: Search | p: Preview`
  - TagTable focus, DICOMDIR raw view: `Tab/Esc: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | t: Directory | /: Search | p: Preview`
  - TagTable focus, file opened from a DICOMDIR: `... | p: Preview | Esc: Back to DICOMDIR`
  - A pending diff mark or a diff status message overrides the help line (yellow, see §7.1).
    Direct mode shows the same variants (with `q/Esc: Quit`) and also displays status
    messages (e.g. `Referenced file not found: <name>`) in yellow on its help line.

### Validation pane height

The validation pane is **3 rows** tall normally and **4 rows** tall when the result is invalid
(to fit the list of missing fields). While a DICOMDIR is loaded the pane shows the file-set
health instead (§11.4) and is 4 rows tall when there is at least one issue.

---

## 10. File explorer behavior

- The explorer provides standard directory-tree navigation (keyboard per §4.3, plus mouse).
- **Selecting a file loads it.** Re-selecting the path that is already loaded is a no-op
  (no reload). While interactive diff mode is active (§7.1), selection does **not** load or
  clear anything — keyboard and mouse selection only move the highlight.
- **Landing on a non-file** (e.g. a directory) clears any loaded DICOM: the tag table, file
  metadata, preview, and search query/state are reset. If the tag table had focus, focus
  returns to the explorer.
- A file that fails to open is handled gracefully (the display is cleared rather than crashing).

### 10.1 Sorting

`s` (explorer focus) opens a **Sort by** menu over the bottom of the explorer pane. The
current key is marked `●`. `↑/↓` + `Enter` (or the digit of the row) applies a key; `Esc` /
`s` closes the menu. Any mouse click also closes it.

| Key               | Order of files                                                        |
| ----------------- | --------------------------------------------------------------------- |
| `Filename`        | Default: the explorer's own name order                                |
| `InstanceNumber`  | Instance Number, by value                                             |
| `SOPInstanceUID`  | SOP Instance UID, digit runs compared by value (`1.2.9` < `1.2.10`)   |
| `ContentTime`     | ContentDate, then ContentTime                                         |
| `AcquisitionTime` | AcquisitionDate, then AcquisitionTime; else AcquisitionDateTime       |
| `Other tag…`      | A typed tag: keyword (`InstanceNumber`, case-sensitive), `(gggg,eeee)`, `gggg,eeee` or `ggggeeee`. A tag that has its own row (InstanceNumber, SOPInstanceUID) selects that row. An unknown tag shows `Unknown tag` and keeps the prompt open; `Esc` goes back to the menu |

- Only files move: `../` and folders stay on top in name order. Ties, and files that lack
  the attribute or are not DICOM (listed last), fall back to the file name.
- Numeric VRs (IS, DS, US, SS, UL, SL, UV, SV, FL, FD) compare by value, value by value;
  DA / TM / DT compare as text; anything else compares with digit runs by value.
- Values are read from each file's header (up to Pixel Data) and cached per key; choosing a
  key again from the menu re-reads the files. The highlighted entry stays highlighted.
- The sort **stays active across folders**: entering another folder sorts it too.
- While a sort other than `Filename` is active:
  - the explorer's top border shows a **`⇅ <key>`** badge (black on yellow, bold) on the right;
  - each file shows its sort value dimmed and right-aligned (cut from the left with `…`
    when too long, so the end of a UID or time stays visible).
- Keyboard and mouse navigation follow the displayed order.

### Mouse interaction details

- Clicking inside the explorer pane gives it focus.
- Clicking a row selects that entry. Clicking the **already-selected** entry, when it is a
  directory, enters that directory.
- Click resolution accounts for the explorer border and the current scroll offset; clicks
  outside the populated rows are ignored.

---

## 11. Appendix — colors & key constants

### Colors

| Element                     | Color                                         |
| --------------------------- | --------------------------------------------- |
| Focused pane border         | Green                                         |
| Unfocused pane border       | Dark gray                                     |
| Table header cells          | Yellow, bold                                  |
| Selected row highlight      | Dark-gray background, bold                    |
| Private tag rows (non-diff) | Dark gray                                     |
| Diff: added                 | Green                                         |
| Diff: deleted               | Red                                           |
| Diff: changed indicator     | Blue                                          |
| Inline diff: deletion       | Red, strikethrough, dark-gray background      |
| Inline diff: insertion      | Green, bold                                   |
| Datetime delta suffix       | Dark gray                                     |
| Validation valid            | Blue border, `✓ All required fields present`  |
| Validation invalid          | Red border, `✗ Missing required fields`       |
| Validation not applicable   | Dark-gray border, `Validation not applicable` |
| Preview errors              | Red                                           |
| Help line                   | Cyan                                          |
| Search prompt               | Yellow                                        |
| Diff mark badge `A`         | Red, bold (baseline side)                     |
| Diff mark badge `B`         | Green, bold (modified side)                   |
| Diff hint / status message  | Yellow                                        |

### Key constants

| Constant                     | Value                                               |
| ---------------------------- | --------------------------------------------------- |
| Event poll interval          | 100 ms                                              |
| Preview re-decode debounce   | 100 ms                                              |
| Mouse scroll step            | 3 rows                                              |
| Keyboard scroll step         | 1 row                                               |
| Value truncation length      | 256 characters                                      |
| DICOM magic                  | `DICM` at byte offset 128 (132-byte minimum header) |
| Indentation per tree level   | 2 spaces                                            |
| Direct mode split            | 50 / 50 (tags / preview)                            |
| Explorer mode split          | 25 / 75 (explorer / content)                        |
| Explorer preview pane height | 14 rows                                             |
| Tag area minimum height      | 5 rows                                              |
| Validation pane height       | 3 rows (4 when invalid)                             |

### Validation rules (Type-1 fields)

SOP class is derived from `SOPClassUID`:

| UID                         | Reported class               |
| --------------------------- | ---------------------------- |
| `1.2.840.10008.5.1.4.1.1.2` | CT Image Storage             |
| `1.2.840.10008.5.1.4.1.1.4` | MR Image Storage             |
| any other UID               | `Other` (UID shown verbatim) |
| absent                      | `Unknown`                    |

Type-1 (required, non-empty) validation runs **only** for CT or MR Image Storage; any other
SOP class (or a missing `SOPClassUID`) yields **Validation not applicable**. A field counts as
present when the element exists and, if it stringifies, is non-empty.

Required groups checked for every CT/MR file:

- **SOP Common:** `SOPClassUID`, `SOPInstanceUID`
- **General Study:** `StudyInstanceUID`
- **General Series:** `Modality`, `SeriesInstanceUID`
- **Frame of Reference:** `FrameOfReferenceUID`
- **Image Plane:** `ImagePositionPatient`, `ImageOrientationPatient`, `PixelSpacing`
- **Image Pixel:** `SamplesPerPixel`, `PhotometricInterpretation`, `Rows`, `Columns`,
  `BitsAllocated`, `BitsStored`, `HighBit`, `PixelRepresentation`, `PixelData`

Modality-specific groups:

- **CT:** `ImageType`, `RescaleIntercept`, `RescaleSlope`
- **MR:** `ImageType`, `ScanningSequence`, `SequenceVariant`, `MRAcquisitionType`

If any required field is missing, the validation pane lists them as `Missing: a, b, ...`.

---

## 11. DICOMDIR directory view

A DICOMDIR (Media Storage Directory) stores a whole file-set as one flat
`(0004,1220) Directory Record Sequence` whose items are linked to each other by **byte
offsets** (next sibling, first child). As a tag table this is unreadable, so dcr renders it as
a tree of patients, studies, series and instances, plus a summary of how the referenced files
are laid out on disk.

### 11.1 Detection & parsing

- A loaded file is treated as a DICOMDIR when its media storage SOP class is
  `1.2.840.10008.1.3.10`, **or** when it is named `DICOMDIR` (case-insensitive). Detection
  applies to `dcr <FILE>` and to files selected in the explorer; **never in diff mode**.
- Records are read from the sequence in order. The byte offset of every item is recovered by
  re-reading the file through a byte-counting token stream, so the tree can be rebuilt from
  `(0004,1200)` root offset, `(0004,1400)` next-record offset and `(0004,1420)` lower-level
  offset — the ground truth. Records unreachable from the root are kept as extra roots so
  nothing silently disappears.
- If the offsets cannot be read (or their count does not match the record count), the
  hierarchy is rebuilt from **record order** instead: each record hangs under the closest
  preceding record of a lower rank (`PATIENT` < `STUDY` < `SERIES` < everything else). The
  summary strip then says `(tree from record order)`.
- For every record with a `(0004,1500)` Referenced File ID, the file's existence under the
  DICOMDIR's folder is checked. The folder is also walked (up to 50 000 entries, hidden files
  skipped, the DICOMDIR itself excluded) to find files that **no record references**; path
  comparison is case-insensitive.
- Raw tags are still extracted, so the tag table remains available (`t`).

### 11.2 Rows

The right-hand pane is titled ` DICOMDIR · <folder name> `. Below a two-line **summary strip**
and a rule, the tree is a single-column table using the same `▼`/`▶` indicators, 2-space
indentation, highlight and focus-border rules as the tag table. One row per record, with a
fixed grammar per level (columns padded, long values truncated with `…`):

| Type                | Columns                                                                                        |
| ------------------- | ---------------------------------------------------------------------------------------------- |
| `PATIENT` (magenta) | name (`^` → space), `ID <id>`, `<n> study/studies`                                              |
| `STUDY` (cyan)      | date as `YYYY-MM-DD`, description, `acc <accession>`, `<n> series · <m> img`                    |
| `SERIES` (yellow)   | `#<number>`, modality, description, `<n> img`, instance range, folder                           |
| leaf (`IMAGE`, …)   | `#<instance>`, file name, SOP class name, transfer syntax name, `✓` (green) or `✗ missing` (red) |

- Missing values show a dark-gray placeholder: `(no name)`, `(no date)`, `(no description)`,
  `(no file)`, `-`.
- The **instance range** of a series is `min–max` over its children's Instance Numbers,
  followed by `(N gap(s)[, N dup])` in yellow when numbers are missing or repeated; `#n` when
  min = max; empty when no child has an instance number.
- The series **folder** is shown as `<dir>/` (or `(root)`) when all its instances live in one
  folder, and `<n> folders` in yellow otherwise.
- Records whose `(0004,1410)` in-use flag is 0 are rendered dark gray with ` (inactive)`
  appended; they are counted but never hidden.
- Leaf record types other than `IMAGE` (e.g. `SR DOCUMENT`, `PRESENTATION`) use the leaf
  grammar with their own type label. Unknown SOP class / transfer syntax UIDs fall back to
  `SOP …<tail>` / the raw UID.

**Default expansion:** patients and studies are open, series are folded (they may hold
hundreds of instances), `LAYOUT` is folded. `→` expands, `←` collapses the selected row if it
is open, otherwise its nearest expanded ancestor (moving the selection there).

**`LAYOUT` node** (blue, last root row): `LAYOUT  <f> folder(s) · <n> files referenced ·
<u> unreferenced on disk` (`u` red when > 0). Expanded, it lists one row per referenced folder
— `<dir>/  <count> files  ← SERIES #11 CT, …` naming the parent records stored there — and,
when applicable, an `unreferenced on disk: N file(s)` row (red) that expands to the first 200
stray files (dark gray). This is the "what the folder should look like" versus "what it does
look like" comparison.

### 11.3 Summary strip

- Line 1 (bold): `<p> patient(s) · <s> study/studies · <n> series · <i> image(s)`, followed by
  `<k> <type>` for other leaf types, then the series modalities as `CT ×3  MR ×2` (yellow),
  most frequent first.
- Line 2: `File-set ID: <id|(none)>   consistency: <flag>   in use: <a>/<n>   on disk: <f>/<r>`.
  The consistency flag is red when non-zero, `in use` yellow when some records are inactive,
  `on disk` green when every referenced file exists and red otherwise.

### 11.4 Health pane

While a DICOMDIR is loaded (directory **or** raw view), the validation pane is replaced by:

- Title ` ✓ DICOMDIR consistent ` (blue) or ` ✗ DICOMDIR: N issue(s) ` (red).
- `Records: <n> (<in use> in use) · Files: <f>/<r> on disk · Unreferenced: <u> · Links: byte offsets|record order`
- When there are issues, a red `Issues:` line listing them, comma-separated. Possible issues:
  `consistency flag <n>`, `N inactive record(s)`, `N missing file(s)`, `N unreferenced file(s)`,
  `N dangling offset(s)` (links pointing outside the record set, including cycles),
  `N unreachable record(s)`, `N hierarchy violation(s)` (a child not ranked deeper than its
  parent, or a STUDY/SERIES/IMAGE at the root), `N duplicate instance UID(s)`,
  `N series with mixed transfer syntax(es)`, `offsets unreadable, tree rebuilt from record order`.

### 11.5 Interaction

- **Preview follows the selection.** A leaf row previews its referenced file; a branch row
  previews the *middle* child recursively (the middle instance of the middle series — the first
  instance is often a localizer). Rows without a file (LAYOUT, folders) show no image. Changing
  the selection to a different file clears the preview and re-decodes after the usual 100 ms
  debounce (§8). In the raw view the preview targets the DICOMDIR file itself, as for any file.
- **`Enter`** on a leaf (or an unreferenced-file row) opens that file in the normal tag view,
  remembering the DICOMDIR and its view state. In Explorer mode focus moves to the tag table.
  If the file does not exist on disk the status message `Referenced file not found: <name>` is
  shown instead. **`Esc`** (with no filter active) then **returns to the DICOMDIR** with its
  expansion state and selection intact and without re-parsing it. Loading any other file, or
  clearing the display, forgets the return target.
- **`t`** toggles between the directory view and the raw tag table (titled
  ` DICOMDIR (raw tags): <name> `). Toggling clears the search filter, since the two views
  filter different things.
- **Search (`/`)** in the directory view matches the query case-insensitively against a
  record's type, every primitive attribute value and its file path. Matching records are shown
  together with their ancestors and descendants, all expanded; the `LAYOUT` node is hidden while
  a filter is active. An empty query restores the full tree and default selection.
- **Mouse:** scroll moves the directory selection by 3 rows; a click inside the rows selects the
  clicked row; in Explorer mode a click inside the pane focuses it.
- Diff mode ignores DICOMDIRs (both CLI and interactive marks show raw tags).
