use crate::dicom::DicomTag;
use crate::validation::{SopClass, ValidationResult};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use ratatui::widgets::Block;
use ratatui_explorer::{FileExplorer, Theme};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::path::{Path, PathBuf};
use std::time::Instant;

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

pub struct App {
    pub tags: Vec<DicomTag>,
    pub all_tags: Vec<DicomTag>,
    pub filtered_tags: Option<Vec<DicomTag>>,
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
    pub mode: AppMode,
    pub focus: Focus,
    pub explorer: Option<FileExplorer>,
    pub selected_path: Option<PathBuf>,
    pub preview_pending_since: Option<Instant>,
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
            mode: AppMode::Direct,
            focus: Focus::TagTable,
            explorer: None,
            selected_path: None,
            preview_pending_since: None,
        };
        app.decode_preview();
        app
    }

    pub fn new_explorer(picker: Option<Picker>) -> Self {
        let theme = Theme::default().with_block(Block::default());
        let explorer = FileExplorer::with_theme(theme).expect("failed to create file explorer");

        Self {
            tags: Vec::new(),
            all_tags: Vec::new(),
            filtered_tags: None,
            table_state: TableState::default(),
            should_quit: false,
            file_name: String::new(),
            diff_mode: false,
            modified_name: None,
            search_mode: false,
            search_query: String::new(),
            validation_result: ValidationResult::NotApplicable,
            sop_class: SopClass::Unknown,
            table_area: Rect::default(),
            show_preview: true,
            preview_image: None,
            preview_error: None,
            dicom_file_path: None,
            picker,
            mode: AppMode::Explorer,
            focus: Focus::Explorer,
            explorer: Some(explorer),
            selected_path: None,
            preview_pending_since: None,
        }
    }

    pub fn load_dicom_file(&mut self, path: &Path) {
        if self.selected_path.as_deref() == Some(path) {
            return;
        }

        self.selected_path = Some(path.to_path_buf());
        self.dicom_file_path = Some(path.to_path_buf());

        let obj = match dicom::object::open_file(path) {
            Ok(obj) => obj,
            Err(_) => {
                self.clear_dicom_display();
                return;
            }
        };

        let tags = crate::dicom::extract_tags(&obj);
        self.sop_class = crate::validation::get_sop_class_from_obj(&obj);
        self.validation_result = crate::validation::validate_type1_fields_from_obj(&obj);
        self.file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        self.all_tags = tags;
        self.filter_tags();

        self.preview_image = None;
        self.preview_error = None;
        if self.show_preview {
            self.preview_pending_since = Some(Instant::now());
        }
    }

    pub fn clear_dicom_state(&mut self) {
        if self.selected_path.is_none() {
            return;
        }
        self.clear_dicom_display();
    }

    fn clear_dicom_display(&mut self) {
        self.selected_path = None;
        self.tags.clear();
        self.all_tags.clear();
        self.filtered_tags = None;
        self.table_state.select(None);
        self.file_name = String::new();
        self.validation_result = ValidationResult::NotApplicable;
        self.sop_class = SopClass::Unknown;
        self.preview_image = None;
        self.preview_error = None;
        self.preview_pending_since = None;
        self.dicom_file_path = None;
        self.search_query.clear();
        self.search_mode = false;
        if self.focus == Focus::TagTable {
            self.focus = Focus::Explorer;
        }
    }

    pub fn has_dicom_loaded(&self) -> bool {
        !self.all_tags.is_empty()
    }

    pub fn tick_preview_debounce(&mut self) {
        if let Some(since) = self.preview_pending_since {
            if since.elapsed() >= std::time::Duration::from_millis(100) {
                self.preview_pending_since = None;
                self.decode_preview();
            }
        }
    }
}
