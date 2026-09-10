use ratatui::layout::Rect;
use ratatui_explorer::FileExplorer;
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
}

impl Layout {
    /// Path highlighted in the file explorer, if there is one.
    pub fn current_path(&self) -> Option<PathBuf> {
        self.explorer.as_ref().map(|e| e.current().path.clone())
    }
}

/// Index of the first explorer row on screen for a viewport of `view_height` rows: the
/// widget scrolls just enough to keep the selection visible.
pub fn explorer_scroll_offset(selected: usize, view_height: usize) -> usize {
    (selected + 1).saturating_sub(view_height)
}
