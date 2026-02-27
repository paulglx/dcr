use crate::dicom::DicomTag;

use super::App;

impl App {
    pub(super) fn filter_tags(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_tags = None;
            self.rebuild_visible_tags();
        } else {
            let query = self.search_query.to_lowercase();
            let filtered: Vec<DicomTag> = self
                .all_tags
                .iter()
                .filter(|tag| {
                    tag.tag.to_lowercase().contains(&query)
                        || tag.name.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
            self.filtered_tags = Some(filtered);
            self.rebuild_visible_tags();
        }
        self.reset_selection();
    }

    pub(super) fn reset_selection(&mut self) {
        if self.tags.is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state.select(Some(0));
        }
    }
}
