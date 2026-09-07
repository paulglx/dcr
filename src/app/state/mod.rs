pub mod layout;
pub mod marks;
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
pub use self::marks::DiffSide;

use self::layout::Layout;
use self::marks::Marks;
use self::meta::FileMeta;
use self::preview::Preview;
use self::search::Search;
use self::tags::Tags;

pub struct App {
    pub tags: Tags,
    pub search: Search,
    pub preview: Preview,
    pub meta: FileMeta,
    pub marks: Marks,
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
        Self::new_with_diff(
            tags,
            file_name,
            None,
            validation_result,
            sop_class,
            false,
            None,
            None,
            None,
        )
    }

    pub fn new_with_diff(
        tags: Vec<DicomTag>,
        file_name: String,
        modified_name: Option<String>,
        validation_result: ValidationResult,
        sop_class: SopClass,
        diff_mode: bool,
        dicom_file_path: Option<PathBuf>,
        baseline_path: Option<PathBuf>,
        picker: Option<Picker>,
    ) -> Self {
        let mut app = Self {
            tags: Tags::from_tags(tags),
            search: Search::default(),
            preview: Preview::new(picker),
            marks: Marks::default(),
            meta: FileMeta {
                path: dicom_file_path,
                baseline_path,
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
        app.preview
            .decode(app.meta.path.as_deref(), app.meta.baseline_path.as_deref());
        app
    }

    pub fn new_explorer(picker: Option<Picker>) -> Self {
        let theme = Theme::default().with_block(Block::default());
        let explorer = FileExplorer::with_theme(theme).expect("failed to create file explorer");

        Self {
            tags: Tags::default(),
            search: Search::default(),
            preview: Preview::new(picker),
            marks: Marks::default(),
            meta: FileMeta {
                path: None,
                baseline_path: None,
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
        self.meta.baseline_path = None;

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

        self.preview.clear();
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
        self.preview.clear();
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

    pub fn mark_for_diff(&mut self, side: DiffSide) {
        let path = self
            .layout
            .explorer
            .as_ref()
            .map(|e| e.current().path().to_path_buf());
        if let Some(path) = path {
            self.mark_path_for_diff(side, &path);
        }
    }

    pub fn mark_path_for_diff(&mut self, side: DiffSide, path: &Path) {
        if !path.is_file() {
            self.marks.message = Some("Only files can be marked for diff".into());
            return;
        }

        let other = match side {
            DiffSide::Baseline => (&self.marks.modified, "B"),
            DiffSide::Modified => (&self.marks.baseline, "A"),
        };
        if other.0.as_deref() == Some(path) {
            self.marks.message = Some(format!("Already marked as {}", other.1));
            return;
        }

        match side {
            DiffSide::Baseline => self.marks.baseline = Some(path.to_path_buf()),
            DiffSide::Modified => self.marks.modified = Some(path.to_path_buf()),
        }
        self.open_diff_if_ready();
    }

    pub fn swap_diff_marks(&mut self) {
        if self.meta.diff_mode {
            std::mem::swap(&mut self.marks.baseline, &mut self.marks.modified);
            self.open_diff_if_ready();
        }
    }

    pub fn exit_diff(&mut self) {
        if !self.meta.diff_mode {
            return;
        }
        let baseline = self.marks.baseline.take();
        self.marks.clear();
        self.meta.diff_mode = false;
        self.meta.modified_name = None;
        self.meta.selected_path = None;
        if let Some(path) = baseline {
            self.load_dicom_file(&path);
        } else {
            self.clear_dicom_display();
        }
    }

    fn open_diff_if_ready(&mut self) {
        let (Some(baseline), Some(modified)) =
            (self.marks.baseline.clone(), self.marks.modified.clone())
        else {
            return;
        };

        let tags = match crate::dicom::compare_dicom_files(&baseline, &modified) {
            Ok(tags) => tags,
            Err(e) => {
                self.marks.message = Some(format!("Diff failed: {e}"));
                return;
            }
        };

        self.meta.diff_mode = true;
        self.meta.name = display_file_name(&baseline);
        self.meta.modified_name = Some(display_file_name(&modified));
        self.meta.selected_path = None;
        // Validation follows the modified file (B); the preview blends A and B.
        self.meta.path = Some(modified.clone());
        self.meta.baseline_path = Some(baseline.clone());
        self.meta.sop_class =
            crate::validation::get_sop_class(&modified).unwrap_or(SopClass::Unknown);
        self.meta.validation_result = crate::validation::validate_type1_fields(&modified)
            .unwrap_or(ValidationResult::NotApplicable);

        self.tags.all = tags;
        self.tags.filter(&self.search.query);

        self.preview.clear();
        if self.preview.show {
            self.preview.pending_since = Some(Instant::now());
        }
    }

    pub fn tick_preview_debounce(&mut self) {
        self.preview.tick_debounce(
            self.meta.path.as_deref(),
            self.meta.baseline_path.as_deref(),
        );
    }
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}
