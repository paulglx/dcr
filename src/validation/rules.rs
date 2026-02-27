use dicom::core::Tag;
use dicom::dictionary_std::tags;

pub const SOP_COMMON_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::SOP_CLASS_UID, "SOPClassUID"),
    (tags::SOP_INSTANCE_UID, "SOPInstanceUID"),
];

pub const GENERAL_STUDY_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::STUDY_INSTANCE_UID, "StudyInstanceUID"),
];

pub const GENERAL_SERIES_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::MODALITY, "Modality"),
    (tags::SERIES_INSTANCE_UID, "SeriesInstanceUID"),
];

pub const FRAME_OF_REFERENCE_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::FRAME_OF_REFERENCE_UID, "FrameOfReferenceUID"),
];

pub const IMAGE_PLANE_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::IMAGE_POSITION_PATIENT, "ImagePositionPatient"),
    (tags::IMAGE_ORIENTATION_PATIENT, "ImageOrientationPatient"),
    (tags::PIXEL_SPACING, "PixelSpacing"),
];

pub const IMAGE_PIXEL_TYPE1_TAGS: &[(Tag, &str)] = &[
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

pub const CT_IMAGE_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::IMAGE_TYPE, "ImageType"),
    (tags::RESCALE_INTERCEPT, "RescaleIntercept"),
    (tags::RESCALE_SLOPE, "RescaleSlope"),
];

pub const MR_IMAGE_TYPE1_TAGS: &[(Tag, &str)] = &[
    (tags::IMAGE_TYPE, "ImageType"),
    (tags::SCANNING_SEQUENCE, "ScanningSequence"),
    (tags::SEQUENCE_VARIANT, "SequenceVariant"),
    (tags::MR_ACQUISITION_TYPE, "MRAcquisitionType"),
];
