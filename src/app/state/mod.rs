pub mod layout;
pub mod meta;
pub mod preview;
pub mod search;
pub mod tags;

use crate::dicom::DicomTag;
use crate::validation::{SopClass, ValidationResult};
use ratatui::layout::Rect;
use ratatui::widgets::Block;
use ratatui_explorer::{FileExplorer, Theme};
use ratatui_image::picker::Picker;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub use self::layout::{AppMode, Focus};

use self::layout::Layout;
use self::meta::FileMeta;
use self::preview::Preview;
use self::search::Search;
use self::tags::Tags;

pub struct App {
    pub tags: Tags,
    pub search: Search,
    pub preview: Preview,
    pub meta: FileMeta,
    pub layout: Layout,
    pub should_quit: bool,
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
        let mut app = Self {
            tags: Tags::from_tags(tags),
            search: Search::default(),
            preview: Preview::new(picker),
            meta: FileMeta {
                path: dicom_file_path,
                name: file_name,
                selected_path: None,
                validation_result,
                sop_class,
                diff_mode,
                modified_name,
            },
            layout: Layout {
                mode: AppMode::Direct,
                focus: Focus::TagTable,
                explorer: None,
                explorer_area: Rect::default(),
            },
            should_quit: false,
        };
        app.preview.decode(app.meta.path.as_deref());
        app
    }

    pub fn new_explorer(picker: Option<Picker>) -> Self {
        let theme = Theme::default().with_block(Block::default());
        let explorer = FileExplorer::with_theme(theme).expect("failed to create file explorer");

        Self {
            tags: Tags::default(),
            search: Search::default(),
            preview: Preview::new(picker),
            meta: FileMeta {
                path: None,
                name: String::new(),
                selected_path: None,
                validation_result: ValidationResult::NotApplicable,
                sop_class: SopClass::Unknown,
                diff_mode: false,
                modified_name: None,
            },
            layout: Layout {
                mode: AppMode::Explorer,
                focus: Focus::Explorer,
                explorer: Some(explorer),
                explorer_area: Rect::default(),
            },
            should_quit: false,
        }
    }

    pub fn load_dicom_file(&mut self, path: &Path) {
        if self.meta.selected_path.as_deref() == Some(path) {
            return;
        }

        self.meta.selected_path = Some(path.to_path_buf());
        self.meta.path = Some(path.to_path_buf());

        let obj = match dicom::object::open_file(path) {
            Ok(obj) => obj,
            Err(_) => {
                self.clear_dicom_display();
                return;
            }
        };

        let tags = crate::dicom::extract_tags(&obj);
        self.meta.sop_class = crate::validation::get_sop_class_from_obj(&obj);
        self.meta.validation_result = crate::validation::validate_type1_fields_from_obj(&obj);
        self.meta.name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        self.tags.all = tags;
        self.tags.filter(&self.search.query);

        self.preview.image = None;
        self.preview.error = None;
        if self.preview.show {
            self.preview.pending_since = Some(Instant::now());
        }
    }

    pub fn clear_dicom_state(&mut self) {
        if self.meta.selected_path.is_none() {
            return;
        }
        self.clear_dicom_display();
    }

    fn clear_dicom_display(&mut self) {
        self.tags.clear();
        self.meta.clear();
        self.preview.image = None;
        self.preview.error = None;
        self.preview.pending_since = None;
        self.search.query.clear();
        self.search.active = false;
        if self.layout.focus == Focus::TagTable {
            self.layout.focus = Focus::Explorer;
        }
    }

    pub fn has_dicom_loaded(&self) -> bool {
        self.tags.has_loaded()
    }

    pub fn tick_preview_debounce(&mut self) {
        self.preview.tick_debounce(self.meta.path.as_deref());
    }
}
