/// Status of a tag in diff mode
#[derive(Clone, Debug, PartialEq)]
pub enum DiffStatus {
    Unchanged,
    Added,
    Deleted,
    Changed,
}

/// Represents a single DICOM tag with its properties
#[derive(Clone, Debug)]
pub struct DicomTag {
    pub tag: String,
    pub name: String,
    pub vr: String,
    pub value: String,
    pub baseline_value: Option<String>,
    pub depth: usize,
    pub is_expandable: bool,
    pub is_expanded: bool,
    pub children: Vec<DicomTag>,
    pub diff_status: Option<DiffStatus>,
}

impl DicomTag {
    pub fn is_private(&self) -> bool {
        self.tag
            .get(1..5)
            .and_then(|s| u16::from_str_radix(s, 16).ok())
            .map(|group| group % 2 == 1)
            .unwrap_or(false)
    }
}
