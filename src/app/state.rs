use crate::dicom::DicomTag;
use crate::validation::{SopClass, ValidationResult};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::path::PathBuf;

pub struct App {
    pub tags: Vec<DicomTag>,
    pub all_tags: Vec<DicomTag>,
    pub(super) filtered_tags: Option<Vec<DicomTag>>,
    pub table_state: TableState,
    pub should_quit: bool,
    pub file_name: String,
    pub diff_mode: bool,
    pub modified_name: Option<String>,
    pub search_mode: bool,
    pub search_query: String,
    pub validation_result: ValidationResult,
    pub sop_class: SopClass,
    pub table_area: Rect,
    pub show_preview: bool,
    pub preview_image: Option<StatefulProtocol>,
    pub preview_error: Option<String>,
    pub dicom_file_path: Option<PathBuf>,
    pub picker: Option<Picker>,
}

impl App {
    pub fn new(
        tags: Vec<DicomTag>,
        file_name: String,
        validation_result: ValidationResult,
        sop_class: SopClass,
    ) -> Self {
        Self::new_with_diff(tags, file_name, None, validation_result, sop_class, false, None, None)
    }

    pub fn new_with_diff(
        tags: Vec<DicomTag>,
        file_name: String,
        modified_name: Option<String>,
        validation_result: ValidationResult,
        sop_class: SopClass,
        diff_mode: bool,
        dicom_file_path: Option<PathBuf>,
        picker: Option<Picker>,
    ) -> Self {
        let mut table_state = TableState::default();
        let visible_tags = Self::build_visible_tags_from(&tags);
        if !visible_tags.is_empty() {
            table_state.select(Some(0));
        }

        let mut app = Self {
            tags: visible_tags,
            all_tags: tags,
            filtered_tags: None,
            table_state,
            should_quit: false,
            file_name,
            diff_mode,
            modified_name,
            search_mode: false,
            search_query: String::new(),
            validation_result,
            sop_class,
            table_area: Rect::default(),
            show_preview: true,
            preview_image: None,
            preview_error: None,
            dicom_file_path,
            picker,
        };
        app.decode_preview();
        app
    }
}
