# Session Context

## User Prompts

### Prompt 1

Implement the following plan:

# Add File Meta Information (group 0002) tags to the viewer

## Context

The DICOM viewer currently only displays dataset tags (group 0008+). File Meta Information tags (group 0002) like `MediaStorageSOPInstanceUID` `(0002,0003)` and `TransferSyntaxUID` `(0002,0010)` are stored separately by the `dicom` crate in a `FileMetaTable` and are not iterated by the default `FileDicomObject` iterator.

## Changes

### `src/dicom.rs`

In `extract_tags` (line 118), prepend me...

