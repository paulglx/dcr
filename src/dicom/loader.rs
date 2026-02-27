use super::tag::DicomTag;
use dicom::core::dictionary::DataDictionary;
use dicom::core::header::HasLength;
use dicom::core::header::Header;
use dicom::dictionary_std::StandardDataDictionary;
use dicom::object::{open_file, FileDicomObject, InMemDicomObject};
use std::path::Path;

pub fn load_dicom_file<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<DicomTag>, Box<dyn std::error::Error>> {
    let obj = open_file(path)?;
    Ok(extract_tags(&obj))
}

pub fn extract_tags(obj: &FileDicomObject<InMemDicomObject>) -> Vec<DicomTag> {
    let mut tags = Vec::new();

    for element in obj.meta().to_element_iter() {
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
            baseline_value: None,
            depth: 0,
            is_expandable: false,
            is_expanded: false,
            children: Vec::new(),
            diff_status: None,
        });
    }

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
            baseline_value: None,
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
            baseline_value: None,
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
            baseline_value: None,
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
