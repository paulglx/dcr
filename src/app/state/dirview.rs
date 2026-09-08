//! State of the DICOMDIR directory view: a collapsible tree of records plus a synthetic
//! `LAYOUT` node describing how the referenced files are spread over folders.

use crate::dicom::dicomdir::DirFolder;
use crate::dicom::DicomDir;
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use std::collections::HashSet;
use std::path::PathBuf;

/// Maximum number of unreferenced files listed under the LAYOUT node.
pub const MAX_UNREFERENCED_ROWS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Node {
    /// A directory record, by index into `DicomDir::records`.
    Record(usize),
    /// Root of the folder summary.
    Layout,
    /// A referenced folder, by index into `DirView::folders`.
    Folder(usize),
    /// Header row of the files on disk that no record references.
    Unreferenced,
    /// One unreferenced file, by index into `DicomDir::unreferenced`.
    UnreferencedFile(usize),
}

#[derive(Clone, Debug)]
pub struct Row {
    pub node: Node,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
}

pub struct DirView {
    pub dir: DicomDir,
    pub folders: Vec<DirFolder>,
    expanded: HashSet<Node>,
    pub visible: Vec<Row>,
    pub table_state: TableState,
    /// Whole pane area (for focus clicks).
    pub area: Rect,
    /// Exact area of the scrollable rows (for row clicks).
    pub rows_area: Rect,
    /// When true the raw tag table is shown instead of the directory tree.
    pub show_raw: bool,
    /// Record indices to show while a search filter is active (matches plus ancestors).
    filter: Option<HashSet<usize>>,
    /// File the preview pane should currently decode for the selected row.
    pub preview_target: Option<PathBuf>,
}

impl DirView {
    pub fn new(dir: DicomDir) -> Self {
        let folders = dir.folders();
        // Patients and studies open by default; series stay folded because they can hold
        // hundreds of instances. The layout summary starts folded too.
        let expanded: HashSet<Node> = dir
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.children.is_empty() && r.record_type != "SERIES")
            .map(|(i, _)| Node::Record(i))
            .collect();
        let mut view = Self {
            dir,
            folders,
            expanded,
            visible: Vec::new(),
            table_state: TableState::default(),
            area: Rect::default(),
            rows_area: Rect::default(),
            show_raw: false,
            filter: None,
            preview_target: None,
        };
        view.rebuild();
        view.reset_selection();
        view
    }

    /// True when the tree (not the raw tag table) is what the user sees.
    pub fn is_active(&self) -> bool {
        !self.show_raw
    }

    pub fn selected_row(&self) -> Option<&Row> {
        self.table_state
            .selected()
            .and_then(|i| self.visible.get(i))
    }

    pub fn reset_selection(&mut self) {
        if self.visible.is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state.select(Some(0));
        }
        self.update_preview_target();
    }

    pub fn scroll_down(&mut self, amount: usize) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0);
        let max_index = self.visible.len().saturating_sub(1);
        self.table_state
            .select(Some((current + amount).min(max_index)));
        self.update_preview_target();
    }

    pub fn scroll_up(&mut self, amount: usize) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0);
        self.table_state
            .select(Some(current.saturating_sub(amount)));
        self.update_preview_target();
    }

    pub fn select(&mut self, index: usize) {
        if index < self.visible.len() {
            self.table_state.select(Some(index));
            self.update_preview_target();
        }
    }

    pub fn expand_selected(&mut self) {
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        if row.expandable && !row.expanded {
            self.expanded.insert(row.node);
            self.rebuild();
        }
    }

    /// Collapses the selected row if it is open, otherwise its nearest expanded ancestor,
    /// and moves the selection to the collapsed row.
    pub fn collapse_parent(&mut self) {
        let Some(selected) = self.table_state.selected() else {
            return;
        };
        let Some(row) = self.visible.get(selected).cloned() else {
            return;
        };
        if row.expandable && row.expanded {
            self.expanded.remove(&row.node);
            self.rebuild();
            return;
        }
        for i in (0..selected).rev() {
            if self.visible[i].depth < row.depth && self.visible[i].expanded {
                let node = self.visible[i].node.clone();
                self.expanded.remove(&node);
                self.rebuild();
                self.table_state.select(Some(i));
                self.update_preview_target();
                return;
            }
        }
    }

    /// Filters records whose type, attributes or file path contain `query`
    /// (case-insensitive). Matching records are shown with their ancestors, fully expanded.
    pub fn filter(&mut self, query: &str) {
        if query.is_empty() {
            self.filter = None;
        } else {
            let query = query.to_lowercase();
            let mut keep = HashSet::new();
            for (i, r) in self.dir.records.iter().enumerate() {
                if !r.search_text().to_lowercase().contains(&query) {
                    continue;
                }
                // A match brings its ancestors (for context) and its descendants (a matching
                // series shows its instances) along.
                let mut parent = r.parent;
                while let Some(p) = parent {
                    if !keep.insert(p) {
                        break;
                    }
                    parent = self.dir.records[p].parent;
                }
                let mut stack = vec![i];
                while let Some(idx) = stack.pop() {
                    if keep.insert(idx) {
                        stack.extend(self.dir.records[idx].children.iter().copied());
                    }
                }
            }
            self.filter = Some(keep);
        }
        self.rebuild();
        self.reset_selection();
    }

    pub fn clear_filter(&mut self) {
        self.filter(String::new().as_str());
    }

    /// The file to open when the user presses Enter on the selected row.
    pub fn open_target(&self) -> Option<PathBuf> {
        match &self.selected_row()?.node {
            Node::Record(idx) => self.dir.file_path(*idx),
            Node::UnreferencedFile(i) => self
                .dir
                .unreferenced
                .get(*i)
                .map(|rel| self.dir.base_dir.join(rel)),
            _ => None,
        }
    }

    /// Recomputes the preview target for the current selection. A leaf previews its own
    /// file; a branch previews the middle instance of its middle child, recursively
    /// (the first instance of a series is often a localizer).
    fn update_preview_target(&mut self) {
        self.preview_target = match self.selected_row().map(|r| r.node.clone()) {
            Some(Node::Record(idx)) => self.representative_file(idx, 0),
            Some(Node::UnreferencedFile(i)) => self
                .dir
                .unreferenced
                .get(i)
                .map(|rel| self.dir.base_dir.join(rel)),
            _ => None,
        };
    }

    fn representative_file(&self, idx: usize, depth: usize) -> Option<PathBuf> {
        let record = &self.dir.records[idx];
        if record.children.is_empty() || depth > 8 {
            return self.dir.file_path(idx);
        }
        let mid = record.children[record.children.len() / 2];
        self.representative_file(mid, depth + 1)
    }

    fn rebuild(&mut self) {
        let mut rows = Vec::new();
        let roots = self.dir.roots.clone();
        for root in roots {
            self.collect_record_rows(root, 0, &mut rows);
        }
        if self.filter.is_none() && (!self.folders.is_empty() || !self.dir.unreferenced.is_empty())
        {
            let layout_expanded = self.expanded.contains(&Node::Layout);
            rows.push(Row {
                node: Node::Layout,
                depth: 0,
                expandable: true,
                expanded: layout_expanded,
            });
            if layout_expanded {
                for i in 0..self.folders.len() {
                    rows.push(Row {
                        node: Node::Folder(i),
                        depth: 1,
                        expandable: false,
                        expanded: false,
                    });
                }
                if !self.dir.unreferenced.is_empty() {
                    let expanded = self.expanded.contains(&Node::Unreferenced);
                    rows.push(Row {
                        node: Node::Unreferenced,
                        depth: 1,
                        expandable: true,
                        expanded,
                    });
                    if expanded {
                        for i in 0..self.dir.unreferenced.len().min(MAX_UNREFERENCED_ROWS) {
                            rows.push(Row {
                                node: Node::UnreferencedFile(i),
                                depth: 2,
                                expandable: false,
                                expanded: false,
                            });
                        }
                    }
                }
            }
        }
        self.visible = rows;
        if let Some(selected) = self.table_state.selected() {
            if selected >= self.visible.len() {
                self.table_state.select(self.visible.len().checked_sub(1));
            }
        }
    }

    fn collect_record_rows(&self, idx: usize, depth: usize, rows: &mut Vec<Row>) {
        if let Some(keep) = &self.filter {
            if !keep.contains(&idx) {
                return;
            }
        }
        let record = &self.dir.records[idx];
        let expandable = !record.children.is_empty();
        let expanded =
            expandable && (self.filter.is_some() || self.expanded.contains(&Node::Record(idx)));
        rows.push(Row {
            node: Node::Record(idx),
            depth,
            expandable,
            expanded,
        });
        if expanded {
            for &child in &record.children {
                self.collect_record_rows(child, depth + 1, rows);
            }
        }
    }
}
