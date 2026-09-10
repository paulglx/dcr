use super::tag::DicomTag;
use dicom_core::dictionary::DataDictionary;
use dicom_core::header::{HasLength, Header};
use dicom_core::DataElement;
use dicom_dictionary_std::StandardDataDictionary;
use dicom_object::mem::InMemElement;
use dicom_object::{FileDicomObject, InMemDicomObject};
use std::path::Path;

pub fn load_dicom_file<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<DicomTag>, Box<dyn std::error::Error>> {
    Ok(extract_tags(&dicom_object::open_file(path)?))
}

/// Flattens the file meta group and the data set of `obj` into a tag tree.
pub fn extract_tags(obj: &FileDicomObject<InMemDicomObject>) -> Vec<DicomTag> {
    obj.meta()
        .to_element_iter()
        .map(|element| element_to_tag(&element, Vec::new(), 0))
        .chain(obj.iter().map(|element| inmem_element_to_tag(element, 0)))
        .collect()
}

/// Converts a data set element at `depth`; a sequence's items become its children.
fn inmem_element_to_tag(element: &InMemElement, depth: usize) -> DicomTag {
    let children = element
        .value()
        .items()
        .map(|items| extract_sequence_items(items, depth + 1))
        .unwrap_or_default();
    element_to_tag(element, children, depth)
}

fn element_to_tag<I: HasLength, P>(
    element: &DataElement<I, P>,
    children: Vec<DicomTag>,
    depth: usize,
) -> DicomTag {
    let tag = element.tag();
    DicomTag {
        tag: format!("({:04X},{:04X})", tag.group(), tag.element()),
        name: StandardDataDictionary
            .by_tag(tag)
            .map(|entry| entry.alias.to_string())
            .unwrap_or_default(),
        // `VR::to_string` is inherent and yields `&str`.
        vr: element.vr().to_string().to_owned(),
        value: format_value(element.value()),
        baseline_value: None,
        depth,
        is_expandable: !children.is_empty(),
        is_expanded: false,
        children,
        diff_status: None,
    }
}

fn extract_sequence_items(items: &[InMemDicomObject], depth: usize) -> Vec<DicomTag> {
    items
        .iter()
        .enumerate()
        .map(|(item_idx, item)| {
            let children: Vec<DicomTag> = item
                .iter()
                .map(|element| inmem_element_to_tag(element, depth + 1))
                .collect();
            DicomTag {
                tag: format!("Item #{}", item_idx + 1),
                // DICOMDIR records get a readable label (e.g. `SERIES · #3 CT`) instead of nothing.
                name: super::dicomdir::record_summary(item).unwrap_or_default(),
                vr: String::new(),
                value: format!("<{} element(s)>", item.iter().count()),
                baseline_value: None,
                depth,
                is_expandable: !children.is_empty(),
                is_expanded: false,
                children,
                diff_status: None,
            }
        })
        .collect()
}

fn format_value<I: HasLength, P>(value: &dicom_core::value::Value<I, P>) -> String {
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
