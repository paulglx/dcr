use dcr::validation::{SopClass, ValidationResult};

#[test]
fn test_sop_class_ct_variant() {
    let sop = SopClass::Ct;
    match sop {
        SopClass::Ct => assert!(true),
        _ => panic!("Expected SopClass::Ct"),
    }
}

#[test]
fn test_sop_class_mr_variant() {
    let sop = SopClass::Mr;
    match sop {
        SopClass::Mr => assert!(true),
        _ => panic!("Expected SopClass::Mr"),
    }
}

#[test]
fn test_sop_class_other_variant() {
    let uid = "1.2.840.10008.5.1.4.1.1.7".to_string();
    let sop = SopClass::Other(uid.clone());
    match sop {
        SopClass::Other(stored_uid) => assert_eq!(stored_uid, uid),
        _ => panic!("Expected SopClass::Other"),
    }
}

#[test]
fn test_sop_class_unknown_variant() {
    let sop = SopClass::Unknown;
    match sop {
        SopClass::Unknown => assert!(true),
        _ => panic!("Expected SopClass::Unknown"),
    }
}

#[test]
fn test_sop_class_clone() {
    let sop1 = SopClass::Ct;
    let sop2 = sop1.clone();
    match (sop1, sop2) {
        (SopClass::Ct, SopClass::Ct) => assert!(true),
        _ => panic!("Clone should preserve variant"),
    }
}

#[test]
fn test_validation_result_valid() {
    let result = ValidationResult::Valid;
    match result {
        ValidationResult::Valid => assert!(true),
        _ => panic!("Expected ValidationResult::Valid"),
    }
}

#[test]
fn test_validation_result_invalid() {
    let missing = vec!["SOPClassUID".to_string(), "Modality".to_string()];
    let result = ValidationResult::Invalid(missing.clone());
    match result {
        ValidationResult::Invalid(tags) => {
            assert_eq!(tags.len(), 2);
            assert_eq!(tags[0], "SOPClassUID");
            assert_eq!(tags[1], "Modality");
        }
        _ => panic!("Expected ValidationResult::Invalid"),
    }
}

#[test]
fn test_validation_result_not_applicable() {
    let result = ValidationResult::NotApplicable;
    match result {
        ValidationResult::NotApplicable => assert!(true),
        _ => panic!("Expected ValidationResult::NotApplicable"),
    }
}

#[test]
fn test_validation_result_clone() {
    let result1 = ValidationResult::Valid;
    let result2 = result1.clone();
    match (result1, result2) {
        (ValidationResult::Valid, ValidationResult::Valid) => assert!(true),
        _ => panic!("Clone should preserve variant"),
    }
}
