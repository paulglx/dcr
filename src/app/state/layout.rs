use ratatui::layout::Rect;
use ratatui_explorer::FileExplorer;

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
