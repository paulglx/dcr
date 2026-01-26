use dicom::core::Tag;
use dicom::dictionary_std::tags;
use dicom::dictionary_std::uids::{CT_IMAGE_STORAGE, MR_IMAGE_STORAGE};
use dicom::object::{open_file, FileDicomObject, InMemDicomObject};
use std::path::Path;

/// Validation result for Type 1 fields
#[derive(Clone, Debug)]
pub enum ValidationResult {
    /// All required Type 1 fields are present
    Valid,
    /// Some required Type 1 fields are missing
    Invalid(Vec<String>),
    /// Modality is not CT or MRI, validation not applicable
    NotApplicable,
}

/// Common Type 1 tags required for both CT and MRI
const COMMON_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::PATIENT_ID, "PatientID"),
    (tags::STUDY_INSTANCE_UID, "StudyInstanceUID"),
    (tags::SERIES_INSTANCE_UID, "SeriesInstanceUID"),
    (tags::MODALITY, "Modality"),
    (tags::SOP_CLASS_UID, "SOPClassUID"),
    (tags::SOP_INSTANCE_UID, "SOPInstanceUID"),
];

/// Type 1 tags specific to CT Image IOD
const CT_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::IMAGE_TYPE, "ImageType"),
    (tags::SAMPLES_PER_PIXEL, "SamplesPerPixel"),
    (tags::PHOTOMETRIC_INTERPRETATION, "PhotometricInterpretation"),
    (tags::ROWS, "Rows"),
    (tags::COLUMNS, "Columns"),
    (tags::BITS_ALLOCATED, "BitsAllocated"),
    (tags::BITS_STORED, "BitsStored"),
    (tags::HIGH_BIT, "HighBit"),
    (tags::PIXEL_REPRESENTATION, "PixelRepresentation"),
    (tags::RESCALE_INTERCEPT, "RescaleIntercept"),
    (tags::RESCALE_SLOPE, "RescaleSlope"),
    (tags::KVP, "KVP"),
];

/// Type 1 tags specific to MR Image IOD
const MR_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::IMAGE_TYPE, "ImageType"),
    (tags::SAMPLES_PER_PIXEL, "SamplesPerPixel"),
    (tags::PHOTOMETRIC_INTERPRETATION, "PhotometricInterpretation"),
    (tags::ROWS, "Rows"),
    (tags::COLUMNS, "Columns"),
    (tags::BITS_ALLOCATED, "BitsAllocated"),
    (tags::BITS_STORED, "BitsStored"),
    (tags::HIGH_BIT, "HighBit"),
    (tags::PIXEL_REPRESENTATION, "PixelRepresentation"),
    (tags::SCANNING_SEQUENCE, "ScanningSequence"),
    (tags::SEQUENCE_VARIANT, "SequenceVariant"),
    (tags::SCAN_OPTIONS, "ScanOptions"),
    (tags::MR_ACQUISITION_TYPE, "MRAcquisitionType"),
];

/// Validate Type 1 fields in a DICOM file
pub fn validate_type1_fields<P: AsRef<Path>>(path: P) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    let obj = open_file(path)?;
    
    // Get the SOP Class UID to determine modality
    let sop_class_uid = obj
        .element(tags::SOP_CLASS_UID)
        .ok()
        .and_then(|e| e.to_str().ok())
        .map(|s| s.trim().to_string());
    
    let sop_class_uid = match sop_class_uid {
        Some(uid) => uid,
        None => return Ok(ValidationResult::NotApplicable),
    };
    
    // Determine which modality-specific tags to check
    let modality_tags: &[(Tag, &str)] = if sop_class_uid == CT_IMAGE_STORAGE {
        CT_TYPE1_TAGS
    } else if sop_class_uid == MR_IMAGE_STORAGE {
        MR_TYPE1_TAGS
    } else {
        return Ok(ValidationResult::NotApplicable);
    };
    
    // Collect missing tags
    let mut missing_tags = Vec::new();
    
    // Check common Type 1 tags
    for (tag, name) in COMMON_TYPE1_TAGS {
        if !is_tag_present(&obj, *tag) {
            missing_tags.push(name.to_string());
        }
    }
    
    // Check modality-specific Type 1 tags
    for (tag, name) in modality_tags {
        if !is_tag_present(&obj, *tag) {
            missing_tags.push(name.to_string());
        }
    }
    
    if missing_tags.is_empty() {
        Ok(ValidationResult::Valid)
    } else {
        Ok(ValidationResult::Invalid(missing_tags))
    }
}

/// Check if a tag is present and has a non-empty value
fn is_tag_present(obj: &FileDicomObject<InMemDicomObject>, tag: Tag) -> bool {
    obj.element(tag)
        .ok()
        .map(|e| {
            // Check that the value is not empty
            if let Ok(s) = e.to_str() {
                !s.trim().is_empty()
            } else {
                // For non-string values (like pixel data), just check presence
                true
            }
        })
        .unwrap_or(false)
}
