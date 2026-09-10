use crate::validation::{SopClass, ValidationResult};
use std::path::PathBuf;

#[derive(Default)]
pub struct FileMeta {
    pub path: Option<PathBuf>,
    pub baseline_path: Option<PathBuf>,
    pub name: String,
    pub selected_path: Option<PathBuf>,
    pub validation_result: ValidationResult,
    pub sop_class: SopClass,
    pub diff_mode: bool,
    pub modified_name: Option<String>,
}

impl FileMeta {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
