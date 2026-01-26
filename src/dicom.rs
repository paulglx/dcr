use dicom::core::dictionary::DataDictionary;
use dicom::core::header::Header;
use dicom::core::header::HasLength;
use dicom::dictionary_std::StandardDataDictionary;
use dicom::object::{open_file, FileDicomObject, InMemDicomObject};
use std::path::Path;

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
}

impl DicomTag {
    /// Returns true if this is a private tag (odd group number)
    pub fn is_private(&self) -> bool {
        // Parse group from "(GGGG,EEEE)" format
        self.tag
            .get(1..5)
            .and_then(|s| u16::from_str_radix(s, 16).ok())
            .map(|group| group % 2 == 1)
            .unwrap_or(false)
    }
}

/// Load a DICOM file and extract all tags
pub fn load_dicom_file<P: AsRef<Path>>(path: P) -> Result<Vec<DicomTag>, Box<dyn std::error::Error>> {
    let obj = open_file(path)?;
    Ok(extract_tags(&obj))
}

/// Extract all tags from a DICOM object
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

        // Check if this is a sequence and extract children
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
        });
    }

    tags
}

/// Extract tags from sequence items recursively
fn extract_sequence_items(items: &[InMemDicomObject], depth: usize) -> Vec<DicomTag> {
    let mut children = Vec::new();

    for (item_idx, item) in items.iter().enumerate() {
        let item_children = extract_tags_from_inmem_object(item, depth + 1);
        // Create a header tag for the sequence item
        let item_header = DicomTag {
            tag: format!("Item #{}", item_idx + 1),
            name: String::new(),
            vr: String::new(),
            value: format!("<{} element(s)>", item.into_iter().count()),
            depth,
            is_expandable: !item_children.is_empty(),
            is_expanded: false,
            children: item_children,
        };
        children.push(item_header);
    }

    children
}

/// Extract tags from an InMemDicomObject (for nested sequences)
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

        // Check if this is a sequence and extract children
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
        });
    }

    tags
}

/// Format a DICOM element value as a string, truncating if necessary
fn format_value<I: HasLength, P>(value: &dicom::core::value::Value<I, P>) -> String {
    let value_str = if value.primitive().is_some() {
        value.to_str().map(|c| c.into_owned()).unwrap_or_else(|_| "<Error>".to_string())
    } else if let Some(seq) = value.items() {
        format!("<Sequence with {} item(s)>", seq.len())
    } else if value.fragments().is_some() {
        "<Pixel Data Sequence>".to_string()
    } else {
        "<Unknown>".to_string()
    };

    truncate_value(&value_str, 256)
}

/// Truncate a string to max_len characters, appending "..." if truncated
fn truncate_value(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}
