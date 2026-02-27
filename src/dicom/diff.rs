use super::loader::load_dicom_file;
use super::tag::{DiffStatus, DicomTag};
use std::collections::HashMap;
use std::path::Path;

pub fn compare_dicom_files<P: AsRef<Path>>(
    baseline_path: P,
    modified_path: P,
) -> Result<Vec<DicomTag>, Box<dyn std::error::Error>> {
    let baseline_tags = load_dicom_file(baseline_path)?;
    let modified_tags = load_dicom_file(modified_path)?;

    let mut baseline_map: HashMap<String, &DicomTag> = HashMap::new();
    for tag in &baseline_tags {
        baseline_map.insert(tag.tag.clone(), tag);
    }

    let mut baseline_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut result_tags: Vec<DicomTag> = Vec::new();

    for modified_tag in &modified_tags {
        let tag_id = &modified_tag.tag;
        let mut diff_status = DiffStatus::Added;
        let mut baseline_value = None;

        if let Some(baseline_tag) = baseline_map.get(tag_id) {
            baseline_seen.insert(tag_id.clone());
            if baseline_tag.value == modified_tag.value {
                diff_status = DiffStatus::Unchanged;
            } else {
                diff_status = DiffStatus::Changed;
                baseline_value = Some(baseline_tag.value.clone());
            }
        }

        let mut result_tag = modified_tag.clone();
        result_tag.diff_status = Some(diff_status);
        result_tag.baseline_value = baseline_value;
        result_tags.push(result_tag);
    }

    for baseline_tag in &baseline_tags {
        if !baseline_seen.contains(&baseline_tag.tag) {
            let mut deleted_tag = baseline_tag.clone();
            deleted_tag.diff_status = Some(DiffStatus::Deleted);
            result_tags.push(deleted_tag);
        }
    }

    result_tags.sort_by(|a, b| a.tag.cmp(&b.tag));

    Ok(result_tags)
}
