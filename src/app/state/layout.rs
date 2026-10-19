use super::sort::{ExplorerSort, SortKey, SortMenu};
use ratatui::layout::Rect;
use ratatui_explorer::{File, FileExplorer, Input};
use std::path::PathBuf;

#[derive(PartialEq)]
pub enum AppMode {
    Explorer,
    Direct,
}

#[derive(PartialEq)]
pub enum Focus {
    Explorer,
    TagTable,
}

pub struct Layout {
    pub mode: AppMode,
    pub focus: Focus,
    pub explorer: Option<FileExplorer>,
    pub explorer_area: Rect,
    /// Order the explorer entries are shown in (see `sort`).
    pub sort: ExplorerSort,
    /// Open while the user picks a sort key.
    pub sort_menu: Option<SortMenu>,
}

/// Rows moved by PageUp / PageDown, as in the explorer widget.
const PAGE: usize = 12;

impl Layout {
    /// Path highlighted in the file explorer, if there is one.
    pub fn current_path(&self) -> Option<PathBuf> {
        self.explorer.as_ref().map(|e| e.current().path.clone())
    }

    /// Explorer entries in display order.
    pub fn explorer_rows(&self) -> Vec<&File> {
        let Some(explorer) = &self.explorer else {
            return Vec::new();
        };
        let files = explorer.files();
        (0..files.len())
            .map(|row| &files[self.sort.index_at(row, files.len())])
            .collect()
    }

    /// Display row of the highlighted explorer entry.
    pub fn explorer_row(&self) -> usize {
        self.explorer
            .as_ref()
            .map_or(0, |e| self.sort.row_of(e.selected_idx(), e.files().len()))
    }

    /// Highlights the entry shown on `row`; out-of-range rows are ignored.
    pub fn select_explorer_row(&mut self, row: usize) {
        if let Some(explorer) = &mut self.explorer {
            let len = explorer.files().len();
            if row < len {
                explorer.set_selected_idx(self.sort.index_at(row, len));
            }
        }
    }

    /// Applies an explorer key. Moves follow the display order; entering or leaving a
    /// folder re-sorts the new listing.
    pub fn explorer_input(&mut self, input: Input) {
        let Some(explorer) = &mut self.explorer else {
            return;
        };
        let len = explorer.files().len();
        if len == 0 {
            return;
        }
        let row = self.sort.row_of(explorer.selected_idx(), len);
        let target = match input {
            Input::Up => Some(row.checked_sub(1).unwrap_or(len - 1)),
            Input::Down => Some((row + 1) % len),
            Input::Home => Some(0),
            Input::End => Some(len - 1),
            Input::PageUp => Some(row.saturating_sub(PAGE)),
            Input::PageDown => Some((row + PAGE).min(len - 1)),
            _ => None,
        };
        match target {
            Some(row) => explorer.set_selected_idx(self.sort.index_at(row, len)),
            None => {
                let _ = explorer.handle(input);
                self.sort.sync(explorer);
            }
        }
    }

    /// Sorts the explorer by `key`, keeping the highlighted entry.
    pub fn set_sort(&mut self, key: SortKey) {
        if let Some(explorer) = &self.explorer {
            self.sort.set_key(key, explorer);
        }
    }
}

/// Index of the first explorer row on screen for a viewport of `view_height` rows: the
/// widget scrolls just enough to keep the selection visible.
pub fn explorer_scroll_offset(selected: usize, view_height: usize) -> usize {
    (selected + 1).saturating_sub(view_height)
}
