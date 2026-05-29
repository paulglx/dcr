use crate::dicom::DicomTag;
use ratatui::layout::Rect;
use ratatui::widgets::TableState;

#[derive(Default)]
pub struct Tags {
    pub visible: Vec<DicomTag>,
    pub all: Vec<DicomTag>,
    pub filtered: Option<Vec<DicomTag>>,
    pub table_state: TableState,
    pub area: Rect,
}

impl Tags {
    pub fn from_tags(all: Vec<DicomTag>) -> Self {
        let visible = Self::build_visible_tags_from(&all);
        let mut table_state = TableState::default();
        if !visible.is_empty() {
            table_state.select(Some(0));
        }
        Self {
            visible,
            all,
            filtered: None,
            table_state,
            area: Rect::default(),
        }
    }

    pub fn has_loaded(&self) -> bool {
        !self.all.is_empty()
    }

    pub fn clear(&mut self) {
        self.visible.clear();
        self.all.clear();
        self.filtered = None;
        self.table_state.select(None);
    }

    pub fn filter(&mut self, query: &str) {
        if query.is_empty() {
            self.filtered = None;
            self.rebuild_visible_tags();
        } else {
            let query = query.to_lowercase();
            let filtered: Vec<DicomTag> = self
                .all
                .iter()
                .filter(|tag| {
                    tag.tag.to_lowercase().contains(&query)
                        || tag.name.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
            self.filtered = Some(filtered);
            self.rebuild_visible_tags();
        }
        self.reset_selection();
    }

    pub fn clear_filter(&mut self) {
        self.filtered = None;
        self.rebuild_visible_tags();
        self.reset_selection();
    }

    pub fn reset_selection(&mut self) {
        if self.visible.is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state.select(Some(0));
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        if self.visible.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0);
        let max_index = self.visible.len().saturating_sub(1);
        let new_index = (current + amount).min(max_index);
        self.table_state.select(Some(new_index));
    }

    pub fn scroll_up(&mut self, amount: usize) {
        if self.visible.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0);
        let new_index = current.saturating_sub(amount);
        self.table_state.select(Some(new_index));
    }

    pub fn expand_selected(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if selected_idx < self.visible.len() {
                let selected_tag = &self.visible[selected_idx];
                if selected_tag.is_expandable && !selected_tag.is_expanded {
                    let path = self.build_path_to_tag(selected_idx);
                    Self::set_expanded_in_tree(self.active_tags_mut(), &path, true);
                    self.rebuild_visible_tags();
                }
            }
        }
    }

    pub fn collapse_parent(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if selected_idx < self.visible.len() {
                let current_depth = self.visible[selected_idx].depth;

                if current_depth > 0 {
                    for i in (0..selected_idx).rev() {
                        if self.visible[i].depth < current_depth && self.visible[i].is_expanded {
                            let path = self.build_path_to_tag(i);
                            Self::set_expanded_in_tree(self.active_tags_mut(), &path, false);
                            self.rebuild_visible_tags();
                            self.table_state.select(Some(i));
                            break;
                        }
                    }
                }
            }
        }
    }

    fn build_visible_tags_from(tags: &[DicomTag]) -> Vec<DicomTag> {
        let mut visible = Vec::new();
        Self::collect_visible_tags(tags, &mut visible);
        visible
    }

    fn collect_visible_tags(tags: &[DicomTag], visible: &mut Vec<DicomTag>) {
        for tag in tags {
            visible.push(tag.clone());
            if tag.is_expanded && !tag.children.is_empty() {
                Self::collect_visible_tags(&tag.children, visible);
            }
        }
    }

    fn rebuild_visible_tags(&mut self) {
        let source = self.filtered.as_ref().unwrap_or(&self.all);
        self.visible = Self::build_visible_tags_from(source);
    }

    fn active_tags(&self) -> &Vec<DicomTag> {
        self.filtered.as_ref().unwrap_or(&self.all)
    }

    fn active_tags_mut(&mut self) -> &mut Vec<DicomTag> {
        if self.filtered.is_some() {
            self.filtered.as_mut().unwrap()
        } else {
            &mut self.all
        }
    }

    fn build_path_to_tag(&self, visible_idx: usize) -> Vec<usize> {
        let mut path = Vec::new();
        let mut current_idx = 0;
        Self::find_path_to_index(self.active_tags(), visible_idx, &mut current_idx, &mut path);
        path
    }

    fn find_path_to_index(
        tags: &[DicomTag],
        target_idx: usize,
        current_idx: &mut usize,
        path: &mut Vec<usize>,
    ) -> bool {
        for (i, tag) in tags.iter().enumerate() {
            if *current_idx == target_idx {
                path.push(i);
                return true;
            }
            *current_idx += 1;

            if tag.is_expanded && !tag.children.is_empty() {
                path.push(i);
                if Self::find_path_to_index(&tag.children, target_idx, current_idx, path) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }

    fn set_expanded_in_tree(tags: &mut [DicomTag], path: &[usize], expanded: bool) {
        if path.is_empty() {
            return;
        }

        let idx = path[0];
        if idx >= tags.len() {
            return;
        }

        if path.len() == 1 {
            tags[idx].is_expanded = expanded;
        } else {
            Self::set_expanded_in_tree(&mut tags[idx].children, &path[1..], expanded);
        }
    }
}
