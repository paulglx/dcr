# Session Context

## User Prompts

### Prompt 1

Implement the following plan:

# Graceful exit for invalid path arguments

## Context
When non-existent paths or non-DICOM files are passed as arguments, the app crashes with raw library errors (e.g., dicom crate parse errors or OS file-not-found errors). We need early validation with clear, user-friendly error messages and a clean exit (via `std::process::exit(1)` or early return with `eprintln!`).

## Approach
Add a `validate_path` helper in `src/main.rs` that checks each path argument before ...

