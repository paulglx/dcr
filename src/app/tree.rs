use crate::dicom::DicomTag;

use super::App;

impl App {
    pub(super) fn build_visible_tags_from(tags: &[DicomTag]) -> Vec<DicomTag> {
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

    pub(super) fn rebuild_visible_tags(&mut self) {
        let source = self.filtered_tags.as_ref().unwrap_or(&self.all_tags);
        self.tags = Self::build_visible_tags_from(source);
    }

    pub(super) fn active_tags(&self) -> &Vec<DicomTag> {
        self.filtered_tags.as_ref().unwrap_or(&self.all_tags)
    }

    pub(super) fn active_tags_mut(&mut self) -> &mut Vec<DicomTag> {
        if self.filtered_tags.is_some() {
            self.filtered_tags.as_mut().unwrap()
        } else {
            &mut self.all_tags
        }
    }

    pub(super) fn expand_selected(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if selected_idx < self.tags.len() {
                let selected_tag = &self.tags[selected_idx];
                if selected_tag.is_expandable && !selected_tag.is_expanded {
                    let path = self.build_path_to_tag(selected_idx);
                    Self::set_expanded_in_tree(self.active_tags_mut(), &path, true);
                    self.rebuild_visible_tags();
                }
            }
        }
    }

    pub(super) fn collapse_parent(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if selected_idx < self.tags.len() {
                let current_depth = self.tags[selected_idx].depth;

                if current_depth > 0 {
                    for i in (0..selected_idx).rev() {
                        if self.tags[i].depth < current_depth && self.tags[i].is_expanded {
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
