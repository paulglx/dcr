use super::loader::extract_tags;
use super::tag::{DicomTag, DiffStatus};
use std::collections::HashMap;
use std::path::Path;

pub fn compare_dicom_files<P: AsRef<Path>>(
    baseline_path: P,
    modified_path: P,
) -> Result<Vec<DicomTag>, Box<dyn std::error::Error>> {
    let baseline = dicom_object::open_file(baseline_path)?;
    let modified = dicom_object::open_file(modified_path)?;
    Ok(diff_tags(extract_tags(&baseline), extract_tags(&modified)))
}

/// Merges two tag lists into one annotated with each tag's [`DiffStatus`], sorted by tag id.
/// Top-level tags are matched by id; a changed tag keeps the modified value and records the
/// baseline one.
pub fn diff_tags(baseline: Vec<DicomTag>, modified: Vec<DicomTag>) -> Vec<DicomTag> {
    let mut baseline: HashMap<String, DicomTag> =
        baseline.into_iter().map(|t| (t.tag.clone(), t)).collect();

    let mut result: Vec<DicomTag> = modified
        .into_iter()
        .map(|mut tag| {
            (tag.diff_status, tag.baseline_value) = match baseline.remove(&tag.tag) {
                None => (Some(DiffStatus::Added), None),
                Some(base) if base.value == tag.value => (Some(DiffStatus::Unchanged), None),
                Some(base) => (Some(DiffStatus::Changed), Some(base.value)),
            };
            tag
        })
        .collect();

    result.extend(baseline.into_values().map(|mut tag| {
        tag.diff_status = Some(DiffStatus::Deleted);
        tag
    }));
    result.sort_by(|a, b| a.tag.cmp(&b.tag));
    result
}
