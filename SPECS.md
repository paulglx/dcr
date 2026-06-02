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

The app redraws, then waits for input with a **100 ms poll interval**. Only key **press**
events are handled (key release/repeat events are ignored). After each poll cycle, the preview
debounce timer is ticked (see §8). The app exits the loop and quits when a quit action sets the
quit flag.

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

When the search input is **active**:

| Key                   | Action                                                |
| --------------------- | ----------------------------------------------------- |
| _printable character_ | Append to the query and re-filter live                |
| `Backspace`           | Remove the last character and re-filter               |
| `Enter`               | Confirm — close the input but keep the current filter |
| `Esc`                 | Cancel — close the input and clear the filter         |

### 4.3 Explorer mode — Explorer focus

| Key                                    | Action                                                    |
| -------------------------------------- | --------------------------------------------------------- |
| `q` / `Esc`                            | Quit                                                      |
| `Tab`                                  | Switch focus to the tag table (only if a DICOM is loaded) |
| `p`                                    | Toggle the image preview pane                             |
| `↑` / `k`, `↓` / `j`                   | Move within the file tree                                 |
| `→` / `l` / `Enter`                    | Enter the selected directory, or select the file          |
| `←` / `h`                              | Go to the parent directory                                |
| `Home` / `End` / `PageUp` / `PageDown` | Standard list navigation                                  |

After any navigation, the currently highlighted entry is evaluated: if it is a file it is
loaded (see §10); otherwise any loaded DICOM state is cleared. Pressing `Enter` on a file also
moves focus to the tag table once it has loaded.

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

---

## 8. Image preview

- Toggled with `p`; **on by default**.
- Requires a **Kitty-compatible terminal**. If the terminal lacks the capability, the pane
  shows `Preview requires a Kitty-compatible terminal`.
- When available, the preview decodes the file's pixel data and renders **frame 0** as an
  image.
- Error states (shown in red within the pane):
  - `No DICOM file path available` — no file path to decode.
  - `Failed to open DICOM file: <error>`
  - `Failed to decode pixel data: <error>`
  - `Failed to convert to image: <error>`
- While a decode is in progress (or pending), the pane shows `Decoding...`.
- **Debounce:** loading a new file clears the current image and, if the preview is shown,
  schedules a re-decode after a **100 ms** debounce. The debounce is checked each event-loop
  cycle, so rapid file changes do not trigger a decode for every intermediate selection.

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

### Explorer mode

- The screen is split **horizontally 25 / 75**: file explorer on the left, content on the
  right. The explorer title shows the current working directory.
- When a DICOM is loaded **and** the preview is shown, the left column reserves a **14-row**
  preview pane below the explorer.
- Right side:
  - With a file loaded: the tag table (minimum height 5) above the validation pane.
  - With no file loaded: a placeholder pane titled `DICOM Viewer` reading
    `Select a DICOM file to view its tags`.
- Help line, depending on focus and whether a file is loaded:
  - Explorer focus, no file: `↑/↓: Navigate | →: Enter dir | ←: Parent dir | q: Quit`
  - Explorer focus, file loaded: `↑/↓: Navigate | →: Enter | ←: Back | Tab: Tags | p: Preview | q: Quit`
  - TagTable focus: `Tab/Esc: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview`

### Validation pane height

The validation pane is **3 rows** tall normally and **4 rows** tall when the result is invalid
(to fit the list of missing fields).

---

## 10. File explorer behavior

- The explorer provides standard directory-tree navigation (keyboard per §4.3, plus mouse).
- **Selecting a file loads it.** Re-selecting the path that is already loaded is a no-op
  (no reload).
- **Landing on a non-file** (e.g. a directory) clears any loaded DICOM: the tag table, file
  metadata, preview, and search query/state are reset. If the tag table had focus, focus
  returns to the explorer.
- A file that fails to open is handled gracefully (the display is cleared rather than crashing).

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
