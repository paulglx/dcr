# Session Context

## User Prompts

### Prompt 1

Implement the following plan:

# Add Tests for Untested Modules

## Context

The SOLID reorganization is complete. The codebase has 44 passing tests but several modules have zero or minimal test coverage. This plan adds tests for the most impactful gaps: datetime parsing, diff comparison, app scroll/search/tree navigation, and the `new_with_diff` constructor.

Skipped: `handle_events` (requires terminal/crossterm), `decode_preview` (requires Kitty terminal), `ui.rs` rendering functions (require ...

