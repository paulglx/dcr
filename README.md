# dcr : DICOM Reader

DICOM TUI viewer for inspecting DICOM file tags in a terminal interface.

<img src=".github/screenshot.png" alt="dcr TUI screenshot" />

## Building

```
cargo install --path . 
```

## Usage

```
dcr <FILE>
```

Opens the specified DICOM file in the viewer.

## Controls

- Arrow keys or hjkl: Navigate
- Right arrow or l: Expand selected tag
- Left arrow or h: Collapse parent tag
- /: Search tags
- q or Esc: Close search/Quit
