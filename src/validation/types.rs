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
