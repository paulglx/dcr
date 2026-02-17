# Session Context

## User Prompts

### Prompt 1

Implement the following plan:

# Add Mouse Scroll & Click-to-Select

## Context
The DICOM TUI viewer currently only supports keyboard navigation. Adding mouse scroll and click-to-highlight will make it more intuitive to browse tags.

## Changes

### 1. Enable mouse capture (`src/main.rs`)
- Import `EnableMouseCapture` / `DisableMouseCapture` from `crossterm::event`
- Add `EnableMouseCapture` to the `execute!` call on startup
- Add `DisableMouseCapture` to the cleanup `execute!` call

### 2. Stor...

