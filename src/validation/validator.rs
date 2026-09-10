use super::rules::*;
use super::types::{SopClass, ValidationResult};
use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_dictionary_std::uids::{CT_IMAGE_STORAGE, MR_IMAGE_STORAGE};
use dicom_object::{open_file, FileDicomObject, InMemDicomObject};
use std::path::Path;

pub fn validate_type1_fields<P: AsRef<Path>>(
    path: P,
) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    let obj = open_file(path)?;
    Ok(validate_type1_fields_from_obj(&obj))
}

/// Checks the Type 1 attributes required by the object's SOP class. Only CT and MR image
/// storage have rules; anything else is `NotApplicable`.
pub fn validate_type1_fields_from_obj(obj: &FileDicomObject<InMemDicomObject>) -> ValidationResult {
    let modality_tags: &[(Tag, &str)] = match get_sop_class_from_obj(obj) {
        SopClass::Ct => CT_IMAGE_TYPE1_TAGS,
        SopClass::Mr => MR_IMAGE_TYPE1_TAGS,
        SopClass::Other(_) | SopClass::Unknown => return ValidationResult::NotApplicable,
    };

    let all_tag_groups: &[&[(Tag, &str)]] = &[
        SOP_COMMON_TYPE1_TAGS,
        GENERAL_STUDY_TYPE1_TAGS,
        GENERAL_SERIES_TYPE1_TAGS,
        FRAME_OF_REFERENCE_TYPE1_TAGS,
        IMAGE_PLANE_TYPE1_TAGS,
        IMAGE_PIXEL_TYPE1_TAGS,
        modality_tags,
    ];

    let missing_tags: Vec<String> = all_tag_groups
        .iter()
        .flat_map(|group| group.iter())
        .filter(|(tag, _)| !is_tag_present(obj, *tag))
        .map(|(_, name)| name.to_string())
        .collect();

    if missing_tags.is_empty() {
        ValidationResult::Valid
    } else {
        ValidationResult::Invalid(missing_tags)
    }
}

/// Present and, for string values, non-blank.
fn is_tag_present(obj: &FileDicomObject<InMemDicomObject>, tag: Tag) -> bool {
    obj.element(tag)
        .ok()
        .is_some_and(|e| e.to_str().map_or(true, |s| !s.trim().is_empty()))
}

pub fn get_sop_class<P: AsRef<Path>>(path: P) -> Result<SopClass, Box<dyn std::error::Error>> {
    let obj = open_file(path)?;
    Ok(get_sop_class_from_obj(&obj))
}

pub fn get_sop_class_from_obj(obj: &FileDicomObject<InMemDicomObject>) -> SopClass {
    let sop_class_uid = obj
        .element(tags::SOP_CLASS_UID)
        .ok()
        .and_then(|e| e.to_str().ok())
        .map(|s| s.trim().to_string());

    match sop_class_uid {
        Some(uid) if uid == CT_IMAGE_STORAGE => SopClass::Ct,
        Some(uid) if uid == MR_IMAGE_STORAGE => SopClass::Mr,
        Some(uid) => SopClass::Other(uid),
        None => SopClass::Unknown,
    }
}
