/// Interpreted SOP Class information
#[derive(Clone, Debug)]
pub enum SopClass {
    Ct,
    Mr,
    Other(String),
    Unknown,
}

/// Validation result for Type 1 fields
#[derive(Clone, Debug)]
pub enum ValidationResult {
    Valid,
    Invalid(Vec<String>),
    NotApplicable,
}
