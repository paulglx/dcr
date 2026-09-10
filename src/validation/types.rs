/// Interpreted SOP Class information
#[derive(Clone, Debug, Default)]
pub enum SopClass {
    Ct,
    Mr,
    Other(String),
    #[default]
    Unknown,
}

/// Validation result for Type 1 fields
#[derive(Clone, Debug, Default)]
pub enum ValidationResult {
    Valid,
    Invalid(Vec<String>),
    #[default]
    NotApplicable,
}

impl SopClass {
    /// Human name shown in the validation pane.
    pub fn name(&self) -> &str {
        match self {
            SopClass::Ct => "CT Image Storage",
            SopClass::Mr => "MR Image Storage",
            SopClass::Other(_) => "Other",
            SopClass::Unknown => "Unknown",
        }
    }

    /// The SOP Class UID, or `N/A` when none was found.
    pub fn uid(&self) -> &str {
        match self {
            SopClass::Ct => dicom_dictionary_std::uids::CT_IMAGE_STORAGE,
            SopClass::Mr => dicom_dictionary_std::uids::MR_IMAGE_STORAGE,
            SopClass::Other(uid) => uid,
            SopClass::Unknown => "N/A",
        }
    }
}
