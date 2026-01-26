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

        let value = format_value(element.value());

        tags.push(DicomTag {
            tag: tag_str,
            name,
            vr: vr.to_string(),
            value,
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
