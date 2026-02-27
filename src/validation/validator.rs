use super::rules::*;
use super::types::{SopClass, ValidationResult};
use dicom::core::Tag;
use dicom::dictionary_std::tags;
use dicom::dictionary_std::uids::{CT_IMAGE_STORAGE, MR_IMAGE_STORAGE};
use dicom::object::{open_file, FileDicomObject, InMemDicomObject};
use std::path::Path;

pub fn validate_type1_fields<P: AsRef<Path>>(
    path: P,
) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    let obj = open_file(path)?;
    Ok(validate_type1_fields_from_obj(&obj))
}

pub fn validate_type1_fields_from_obj(obj: &FileDicomObject<InMemDicomObject>) -> ValidationResult {
    let sop_class_uid = obj
        .element(tags::SOP_CLASS_UID)
        .ok()
        .and_then(|e| e.to_str().ok())
        .map(|s| s.trim().to_string());

    let sop_class_uid = match sop_class_uid {
        Some(uid) => uid,
        None => return ValidationResult::NotApplicable,
    };

    let modality_tags: &[(Tag, &str)] = if sop_class_uid == CT_IMAGE_STORAGE {
        CT_IMAGE_TYPE1_TAGS
    } else if sop_class_uid == MR_IMAGE_STORAGE {
        MR_IMAGE_TYPE1_TAGS
    } else {
        return ValidationResult::NotApplicable;
    };

    let mut missing_tags = Vec::new();

    let all_tag_groups: &[&[(Tag, &str)]] = &[
        SOP_COMMON_TYPE1_TAGS,
        GENERAL_STUDY_TYPE1_TAGS,
        GENERAL_SERIES_TYPE1_TAGS,
        FRAME_OF_REFERENCE_TYPE1_TAGS,
        IMAGE_PLANE_TYPE1_TAGS,
        IMAGE_PIXEL_TYPE1_TAGS,
        modality_tags,
    ];

    for group in all_tag_groups {
        for (tag, name) in *group {
            if !is_tag_present(obj, *tag) {
                missing_tags.push(name.to_string());
            }
        }
    }

    if missing_tags.is_empty() {
        ValidationResult::Valid
    } else {
        ValidationResult::Invalid(missing_tags)
    }
}

fn is_tag_present(obj: &FileDicomObject<InMemDicomObject>, tag: Tag) -> bool {
    obj.element(tag)
        .ok()
        .map(|e| {
            if let Ok(s) = e.to_str() {
                !s.trim().is_empty()
            } else {
                true
            }
        })
        .unwrap_or(false)
}

pub fn get_sop_class<P: AsRef<Path>>(
    path: P,
) -> Result<SopClass, Box<dyn std::error::Error>> {
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
