use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub enum DiffSide {
    Baseline,
    Modified,
}

#[derive(Default)]
pub struct Marks {
    pub baseline: Option<PathBuf>,
    pub modified: Option<PathBuf>,
    pub message: Option<String>,
}

impl Marks {
    pub fn any(&self) -> bool {
        self.baseline.is_some() || self.modified.is_some()
    }

    pub fn clear(&mut self) {
        self.baseline = None;
        self.modified = None;
        self.message = None;
    }
}
