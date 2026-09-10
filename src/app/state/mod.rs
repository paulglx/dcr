pub mod dirview;
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
use ratatui_explorer::{FileExplorerBuilder, Theme};
use ratatui_image::picker::Picker;
use std::path::{Path, PathBuf};

pub use self::layout::{AppMode, Focus};
pub use self::marks::DiffSide;

use self::dirview::DirView;
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
    /// Present when the loaded file is a DICOMDIR (see `dirview`).
    pub dirview: Option<DirView>,
    /// DICOMDIR to return to (with its view state) after drilling into a referenced file.
    pub dir_return: Option<(PathBuf, DirView)>,
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
            dirview: None,
            dir_return: None,
            should_quit: false,
        };
        app.preview
            .request(app.meta.path.as_deref(), app.meta.baseline_path.as_deref());
        app
    }

    pub fn new_explorer(picker: Option<Picker>) -> Self {
        let theme = Theme::default().with_block(Block::default());
        let explorer =
            FileExplorerBuilder::build_with_theme(theme).expect("failed to create file explorer");

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
            dirview: None,
            dir_return: None,
            should_quit: false,
        }
    }

    pub fn load_dicom_file(&mut self, path: &Path) {
        self.load_file_inner(path, None);
    }

    /// Loads `path`; `reuse` carries a previously built directory view for the same file
    /// so that returning to a DICOMDIR keeps its expansion and selection.
    fn load_file_inner(&mut self, path: &Path, reuse: Option<DirView>) {
        if self.meta.selected_path.as_deref() == Some(path) {
            return;
        }

        self.dirview = None;
        self.dir_return = None;
        self.meta.selected_path = Some(path.to_path_buf());
        self.meta.path = Some(path.to_path_buf());
        self.meta.baseline_path = None;

        let obj = match dicom_object::open_file(path) {
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

        match reuse {
            Some(view) => self.dirview = Some(view),
            None => self.attach_dicomdir(&obj),
        }

        self.preview.clear();
        self.preview.pending = self.preview.show;
    }

    /// Switches to the directory view when `obj` (the loaded file) is a DICOMDIR.
    pub fn attach_dicomdir(
        &mut self,
        obj: &dicom_object::FileDicomObject<dicom_object::InMemDicomObject>,
    ) {
        let Some(path) = self.meta.path.clone() else {
            return;
        };
        if self.meta.diff_mode || !crate::dicom::dicomdir::is_dicomdir(&path, obj) {
            return;
        }
        let dir = crate::dicom::DicomDir::from_object(&path, obj);
        self.dirview = Some(DirView::new(dir));
        // The DICOMDIR itself has no pixel data; the preview follows the selected record.
        self.preview.clear();
        self.preview.pending = self.preview.show;
    }

    /// True while the DICOMDIR tree (not the raw tag table) is displayed.
    pub fn dir_active(&self) -> bool {
        self.dirview.as_ref().is_some_and(|v| v.is_active())
    }

    /// Toggles between the DICOMDIR tree and the raw tag table. Clears any search filter,
    /// since the two views filter different things.
    pub fn toggle_dir_raw(&mut self) {
        let Some(view) = self.dirview.as_mut() else {
            return;
        };
        view.show_raw = !view.show_raw;
        self.search.query.clear();
        self.search.active = false;
        self.tags.clear_filter();
        if let Some(view) = self.dirview.as_mut() {
            view.clear_filter();
        }
        self.schedule_preview();
    }

    /// Opens the file referenced by the selected directory record, remembering the
    /// DICOMDIR so `Esc` can come back to it.
    pub fn open_selected_record(&mut self) {
        if !self.dir_active() {
            return;
        }
        let Some(target) = self.dirview.as_ref().and_then(|v| v.open_target()) else {
            return;
        };
        if !target.is_file() {
            self.marks.message = Some(format!(
                "Referenced file not found: {}",
                target
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            ));
            return;
        }
        let (Some(dir_path), Some(view)) = (self.meta.path.clone(), self.dirview.take()) else {
            return;
        };
        self.meta.selected_path = None;
        self.load_file_inner(&target, None);
        self.dir_return = Some((dir_path, view));
        if self.layout.mode == AppMode::Explorer {
            self.layout.focus = Focus::TagTable;
        }
    }

    /// Returns to the DICOMDIR a file was opened from. False if there is none.
    pub fn return_to_dicomdir(&mut self) -> bool {
        let Some((path, view)) = self.dir_return.take() else {
            return false;
        };
        self.meta.selected_path = None;
        self.load_file_inner(&path, Some(view));
        true
    }

    /// Path the preview pane should decode: the selected record's file in the directory
    /// view, otherwise the loaded file.
    pub fn preview_path(&self) -> Option<PathBuf> {
        if self.dir_active() {
            self.dirview.as_ref().and_then(|v| v.preview_target.clone())
        } else {
            self.meta.path.clone()
        }
    }

    /// Clears the preview and marks it for an immediate re-decode of `preview_path()`.
    fn schedule_preview(&mut self) {
        self.preview.clear();
        self.preview.pending = self.preview.show;
    }

    /// Runs `f` on the directory view and re-schedules the preview if the selection
    /// moved to a different file.
    fn with_dirview(&mut self, f: impl FnOnce(&mut DirView)) {
        let before = self.dirview.as_ref().and_then(|v| v.preview_target.clone());
        if let Some(view) = self.dirview.as_mut() {
            f(view);
        }
        let after = self.dirview.as_ref().and_then(|v| v.preview_target.clone());
        if before != after {
            self.schedule_preview();
        }
    }

    // Navigation that dispatches to whichever view is on screen.

    pub fn nav_down(&mut self, amount: usize) {
        if self.dir_active() {
            self.with_dirview(|v| v.scroll_down(amount));
        } else {
            self.tags.scroll_down(amount);
        }
    }

    pub fn nav_up(&mut self, amount: usize) {
        if self.dir_active() {
            self.with_dirview(|v| v.scroll_up(amount));
        } else {
            self.tags.scroll_up(amount);
        }
    }

    pub fn nav_expand(&mut self) {
        if self.dir_active() {
            self.with_dirview(|v| v.expand_selected());
        } else {
            self.tags.expand_selected();
        }
    }

    pub fn nav_collapse(&mut self) {
        if self.dir_active() {
            self.with_dirview(|v| v.collapse_parent());
        } else {
            self.tags.collapse_parent();
        }
    }

    pub fn nav_select(&mut self, index: usize) {
        if self.dir_active() {
            self.with_dirview(|v| v.select(index));
        } else if index < self.tags.visible.len() {
            self.tags.table_state.select(Some(index));
        }
    }

    pub fn apply_search(&mut self) {
        let query = self.search.query.clone();
        if self.dir_active() {
            self.with_dirview(|v| v.filter(&query));
        } else {
            self.tags.filter(&query);
        }
    }

    pub fn clear_dicom_state(&mut self) {
        if self.meta.selected_path.is_none() {
            return;
        }
        self.clear_dicom_display();
    }

    fn clear_dicom_display(&mut self) {
        self.dirview = None;
        self.dir_return = None;
        self.tags.clear();
        self.meta.clear();
        self.preview.clear();
        self.preview.pending = false;
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
            .map(|e| e.current().path.clone());
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
        self.preview.pending = self.preview.show;
    }

    /// Requests the preview scheduled by the last selection change, if any, and warms
    /// the cache with the files around it.
    pub fn request_pending_preview(&mut self) {
        if !std::mem::take(&mut self.preview.pending) {
            return;
        }
        let path = self.preview_path();
        self.preview
            .request(path.as_deref(), self.meta.baseline_path.as_deref());
        if !self.meta.diff_mode {
            let neighbours = self.preview_neighbours();
            self.preview.prefetch(neighbours);
        }
    }

    /// Files likely to be selected next: the rows around the selection in the directory
    /// view, otherwise the files around the highlighted explorer entry. Nearest first.
    fn preview_neighbours(&self) -> Vec<PathBuf> {
        const REACH: usize = 3;
        let mut out: Vec<PathBuf> = Vec::new();
        let mut push = |p: Option<PathBuf>| {
            if let Some(p) = p {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        };
        if self.dir_active() {
            if let Some(view) = &self.dirview {
                if let Some(sel) = view.table_state.selected() {
                    for i in around(sel, view.visible.len(), REACH) {
                        push(view.preview_target_at(i));
                    }
                }
            }
        } else if let Some(explorer) = &self.layout.explorer {
            let files = explorer.files();
            for i in around(explorer.selected_idx(), files.len(), REACH) {
                let file = &files[i];
                if !file.is_dir {
                    push(Some(file.path.clone()));
                }
            }
        }
        out
    }

    pub fn toggle_preview(&mut self) {
        let path = self.preview_path();
        self.preview
            .toggle(path.as_deref(), self.meta.baseline_path.as_deref());
    }
}

/// Indices within `reach` of `selected` (exclusive), nearest first, alternating below/above.
fn around(selected: usize, len: usize, reach: usize) -> Vec<usize> {
    let mut out = Vec::with_capacity(reach * 2);
    for d in 1..=reach {
        if selected + d < len {
            out.push(selected + d);
        }
        if let Some(i) = selected.checked_sub(d) {
            out.push(i);
        }
    }
    out
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}
