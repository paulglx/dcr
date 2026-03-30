# Session Context

## User Prompts

### Prompt 1

Let's add a file explorer as a left pane in this ratatui app. Use https://github.com/tatounee/ratatui-explorer . The file explorer opens where dcr was opened, and allows navigating up and down in folders as well as highlighting a file. If the file is dicom, it shows all available information in a middle panel + preview in a right panel. If the file isn't dicom it shows nothing

### Prompt 2

Debounce the image preview (100ms)

### Prompt 3

This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.

Summary:
1. Primary Request and Intent:
   - **Request 1**: Add a file explorer as a left pane in the ratatui DICOM viewer app using the `ratatui-explorer` crate. The explorer opens at the current working directory, allows navigating folders, and when a DICOM file is highlighted it shows tag info in a middle panel + preview in a right panel. Non...

### Prompt 4

We need a way to focus the dicom tag list: enter key on a dicom file. We need a visual feedback to show which panel is focused

