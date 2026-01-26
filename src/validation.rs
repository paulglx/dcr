use dicom::core::Tag;
use dicom::dictionary_std::tags;
use dicom::dictionary_std::uids::{CT_IMAGE_STORAGE, MR_IMAGE_STORAGE};
use dicom::object::{open_file, FileDicomObject, InMemDicomObject};
use std::path::Path;

/// Interpreted SOP Class information
#[derive(Clone, Debug)]
pub enum SopClass {
    /// CT Image Storage
    Ct,
    /// MR Image Storage
    Mr,
    /// Other/unknown SOP Class with raw UID
    Other(String),
    /// No SOP Class UID found
    Unknown,
}

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

// =============================================================================
// Type 1 Tags organized by DICOM Module (per DICOM Part 3)
// =============================================================================

// -----------------------------------------------------------------------------
// SOP Common Module (M) - Type 1 tags
// -----------------------------------------------------------------------------
const SOP_COMMON_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::SOP_CLASS_UID, "SOPClassUID"),
    (tags::SOP_INSTANCE_UID, "SOPInstanceUID"),
];

// -----------------------------------------------------------------------------
// General Study Module (M) - Type 1 tags
// -----------------------------------------------------------------------------
const GENERAL_STUDY_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::STUDY_INSTANCE_UID, "StudyInstanceUID"),
];

// -----------------------------------------------------------------------------
// General Series Module (M) - Type 1 tags
// -----------------------------------------------------------------------------
const GENERAL_SERIES_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::MODALITY, "Modality"),
    (tags::SERIES_INSTANCE_UID, "SeriesInstanceUID"),
];

// -----------------------------------------------------------------------------
// Frame of Reference Module (M) - Type 1 tags
// -----------------------------------------------------------------------------
const FRAME_OF_REFERENCE_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::FRAME_OF_REFERENCE_UID, "FrameOfReferenceUID"),
];

// -----------------------------------------------------------------------------
// Image Plane Module (M) - Type 1 tags
// -----------------------------------------------------------------------------
const IMAGE_PLANE_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::IMAGE_POSITION_PATIENT, "ImagePositionPatient"),
    (tags::IMAGE_ORIENTATION_PATIENT, "ImageOrientationPatient"),
    (tags::PIXEL_SPACING, "PixelSpacing"),
];

// -----------------------------------------------------------------------------
// Image Pixel Module (M) - Type 1 tags
// -----------------------------------------------------------------------------
const IMAGE_PIXEL_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::SAMPLES_PER_PIXEL, "SamplesPerPixel"),
    (tags::PHOTOMETRIC_INTERPRETATION, "PhotometricInterpretation"),
    (tags::ROWS, "Rows"),
    (tags::COLUMNS, "Columns"),
    (tags::BITS_ALLOCATED, "BitsAllocated"),
    (tags::BITS_STORED, "BitsStored"),
    (tags::HIGH_BIT, "HighBit"),
    (tags::PIXEL_REPRESENTATION, "PixelRepresentation"),
    (tags::PIXEL_DATA, "PixelData"),
];

// -----------------------------------------------------------------------------
// CT Image Module (M) - Type 1 tags (CT only)
// Note: KVP (0018,0060) is Type 2, not Type 1
// -----------------------------------------------------------------------------
const CT_IMAGE_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::IMAGE_TYPE, "ImageType"),
    (tags::RESCALE_INTERCEPT, "RescaleIntercept"),
    (tags::RESCALE_SLOPE, "RescaleSlope"),
];

// -----------------------------------------------------------------------------
// MR Image Module (M) - Type 1 tags (MR only)
// Note: ScanOptions (0018,0022) is Type 2, not Type 1
// Note: RepetitionTime and EchoTime are Type 1C (conditional), skipped
// -----------------------------------------------------------------------------
const MR_IMAGE_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::IMAGE_TYPE, "ImageType"),
    (tags::SCANNING_SEQUENCE, "ScanningSequence"),
    (tags::SEQUENCE_VARIANT, "SequenceVariant"),
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
        CT_IMAGE_TYPE1_TAGS
    } else if sop_class_uid == MR_IMAGE_STORAGE {
        MR_IMAGE_TYPE1_TAGS
    } else {
        return Ok(ValidationResult::NotApplicable);
    };
    
    // Collect missing tags
    let mut missing_tags = Vec::new();
    
    // Check SOP Common Module Type 1 tags
    for (tag, name) in SOP_COMMON_TYPE1_TAGS {
        if !is_tag_present(&obj, *tag) {
            missing_tags.push(name.to_string());
        }
    }
    
    // Check General Study Module Type 1 tags
    for (tag, name) in GENERAL_STUDY_TYPE1_TAGS {
        if !is_tag_present(&obj, *tag) {
            missing_tags.push(name.to_string());
        }
    }
    
    // Check General Series Module Type 1 tags
    for (tag, name) in GENERAL_SERIES_TYPE1_TAGS {
        if !is_tag_present(&obj, *tag) {
            missing_tags.push(name.to_string());
        }
    }
    
    // Check Frame of Reference Module Type 1 tags
    for (tag, name) in FRAME_OF_REFERENCE_TYPE1_TAGS {
        if !is_tag_present(&obj, *tag) {
            missing_tags.push(name.to_string());
        }
    }
    
    // Check Image Plane Module Type 1 tags
    for (tag, name) in IMAGE_PLANE_TYPE1_TAGS {
        if !is_tag_present(&obj, *tag) {
            missing_tags.push(name.to_string());
        }
    }
    
    // Check Image Pixel Module Type 1 tags
    for (tag, name) in IMAGE_PIXEL_TYPE1_TAGS {
        if !is_tag_present(&obj, *tag) {
            missing_tags.push(name.to_string());
        }
    }
    
    // Check modality-specific Type 1 tags (CT or MR Image Module)
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

/// Get the SOP Class from a DICOM file
pub fn get_sop_class<P: AsRef<Path>>(path: P) -> Result<SopClass, Box<dyn std::error::Error>> {
    let obj = open_file(path)?;
    
    let sop_class_uid = obj
        .element(tags::SOP_CLASS_UID)
        .ok()
        .and_then(|e| e.to_str().ok())
        .map(|s| s.trim().to_string());
    
    Ok(match sop_class_uid {
        Some(uid) if uid == CT_IMAGE_STORAGE => SopClass::Ct,
        Some(uid) if uid == MR_IMAGE_STORAGE => SopClass::Mr,
        Some(uid) => SopClass::Other(uid),
        None => SopClass::Unknown,
    })
}
