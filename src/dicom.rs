use dicom::core::dictionary::DataDictionary;
use dicom::core::header::HasLength;
use dicom::core::header::Header;
use dicom::dictionary_std::StandardDataDictionary;
use dicom::object::{open_file, FileDicomObject, InMemDicomObject};
use std::collections::HashMap;
use std::path::Path;

/// Status of a tag in diff mode
#[derive(Clone, Debug, PartialEq)]
pub enum DiffStatus {
    Unchanged,
    Added,   // In modified but not baseline
    Deleted, // In baseline but not modified
    Changed, // Same tag, different value
}

/// Represents a single DICOM tag with its properties
#[derive(Clone, Debug)]
pub struct DicomTag {
    /// Tag in (GGGG,EEEE) hex format
    pub tag: String,
    /// Human-readable tag name
    pub name: String,
    /// Value Representation (e.g., "PN", "CS", "LO")
    pub vr: String,
    /// The tag value, truncated if longer than 256 characters
    pub value: String,
    /// Nesting level (0 = root)
    pub depth: usize,
    /// True if this tag has children (is a sequence)
    pub is_expandable: bool,
    /// Current expansion state
    pub is_expanded: bool,
    /// Nested sequence items
    pub children: Vec<DicomTag>,
    /// Diff status (None in normal mode, Some(status) in diff mode)
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

pub fn load_dicom_file<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<DicomTag>, Box<dyn std::error::Error>> {
    let obj = open_file(path)?;
    Ok(extract_tags(&obj))
}

/// Compare two DICOM files and return tags with diff status
pub fn compare_dicom_files<P: AsRef<Path>>(
    baseline_path: P,
    modified_path: P,
) -> Result<Vec<DicomTag>, Box<dyn std::error::Error>> {
    let baseline_tags = load_dicom_file(baseline_path)?;
    let modified_tags = load_dicom_file(modified_path)?;

    // Build a map of baseline tags by tag ID (only root-level tags, sequences treated as units)
    let mut baseline_map: HashMap<String, &DicomTag> = HashMap::new();
    for tag in &baseline_tags {
        baseline_map.insert(tag.tag.clone(), tag);
    }

    // Track which tags from baseline we've seen
    let mut baseline_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Process modified tags and compare with baseline
    let mut result_tags: Vec<DicomTag> = Vec::new();

    for modified_tag in &modified_tags {
        let tag_id = &modified_tag.tag;
        let mut diff_status = DiffStatus::Added;

        if let Some(baseline_tag) = baseline_map.get(tag_id) {
            baseline_seen.insert(tag_id.clone());
            // Compare values (for sequences, compare the sequence representation)
            if baseline_tag.value == modified_tag.value {
                diff_status = DiffStatus::Unchanged;
            } else {
                diff_status = DiffStatus::Changed;
            }
        }

        // Clone modified_tag and set diff_status, preserving children
        let mut result_tag = modified_tag.clone();
        result_tag.diff_status = Some(diff_status);
        result_tags.push(result_tag);
    }

    // Add remaining baseline tags as Deleted
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

fn extract_tags(obj: &FileDicomObject<InMemDicomObject>) -> Vec<DicomTag> {
    let mut tags = Vec::new();

    for element in obj {
        let tag = element.tag();
        let tag_str = format!("({:04X},{:04X})", tag.group(), tag.element());

        let name = StandardDataDictionary
            .by_tag(tag)
            .map(|entry| entry.alias.to_string())
            .unwrap_or_default();

        let vr = element.vr().to_string();

        let (value, children, is_expandable) = if let Some(items) = element.value().items() {
            let children = extract_sequence_items(items, 1);
            let is_expandable = !children.is_empty();
            let value = format!("<Sequence with {} item(s)>", items.len());
            (value, children, is_expandable)
        } else {
            (format_value(element.value()), Vec::new(), false)
        };

        tags.push(DicomTag {
            tag: tag_str,
            name,
            vr: vr.to_string(),
            value,
            depth: 0,
            is_expandable,
            is_expanded: false,
            children,
            diff_status: None,
        });
    }

    tags
}

fn extract_sequence_items(items: &[InMemDicomObject], depth: usize) -> Vec<DicomTag> {
    let mut children = Vec::new();

    for (item_idx, item) in items.iter().enumerate() {
        let item_children = extract_tags_from_inmem_object(item, depth + 1);
        let item_header = DicomTag {
            tag: format!("Item #{}", item_idx + 1),
            name: String::new(),
            vr: String::new(),
            value: format!("<{} element(s)>", item.into_iter().count()),
            depth,
            is_expandable: !item_children.is_empty(),
            is_expanded: false,
            children: item_children,
            diff_status: None,
        };
        children.push(item_header);
    }

    children
}

fn extract_tags_from_inmem_object(obj: &InMemDicomObject, depth: usize) -> Vec<DicomTag> {
    let mut tags = Vec::new();

    for element in obj {
        let tag = element.tag();
        let tag_str = format!("({:04X},{:04X})", tag.group(), tag.element());

        let name = StandardDataDictionary
            .by_tag(tag)
            .map(|entry| entry.alias.to_string())
            .unwrap_or_default();

        let vr = element.vr().to_string();

        let (value, children, is_expandable) = if let Some(items) = element.value().items() {
            let children = extract_sequence_items(items, depth + 1);
            let is_expandable = !children.is_empty();
            let value = format!("<Sequence with {} item(s)>", items.len());
            (value, children, is_expandable)
        } else {
            (format_value(element.value()), Vec::new(), false)
        };

        tags.push(DicomTag {
            tag: tag_str,
            name,
            vr: vr.to_string(),
            value,
            depth,
            is_expandable,
            is_expanded: false,
            children,
            diff_status: None,
        });
    }

    tags
}

fn format_value<I: HasLength, P>(value: &dicom::core::value::Value<I, P>) -> String {
    let value_str = if value.primitive().is_some() {
        value
            .to_str()
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| "<Error>".to_string())
    } else if let Some(seq) = value.items() {
        format!("<Sequence with {} item(s)>", seq.len())
    } else if value.fragments().is_some() {
        "<Pixel Data Sequence>".to_string()
    } else {
        "<Unknown>".to_string()
    };

    truncate_value(&value_str, 256)
}

fn truncate_value(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}
