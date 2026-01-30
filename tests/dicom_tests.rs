use dcr::dicom::DicomTag;

fn create_test_tag(tag: &str, name: &str, vr: &str, value: &str, depth: usize) -> DicomTag {
    DicomTag {
        tag: tag.to_string(),
        name: name.to_string(),
        vr: vr.to_string(),
        value: value.to_string(),
        baseline_value: None,
        depth,
        is_expandable: false,
        is_expanded: false,
        children: Vec::new(),
        diff_status: None,
    }
}

#[test]
fn test_is_private_even_group() {
    let tag = create_test_tag("(0010,0010)", "PatientName", "PN", "Test^Patient", 0);
    assert!(
        !tag.is_private(),
        "Tag with even group (0010) should not be private"
    );
}

#[test]
fn test_is_private_odd_group() {
    let tag = create_test_tag("(0009,0010)", "PrivateTag", "LO", "Private Data", 0);
    assert!(
        tag.is_private(),
        "Tag with odd group (0009) should be private"
    );
}

#[test]
fn test_is_private_higher_odd_group() {
    let tag = create_test_tag("(0011,0010)", "PrivateTag", "LO", "Private Data", 0);
    assert!(
        tag.is_private(),
        "Tag with odd group (0011) should be private"
    );
}

#[test]
fn test_is_private_invalid_format() {
    let tag = DicomTag {
        tag: "Invalid".to_string(),
        name: "".to_string(),
        vr: "".to_string(),
        value: "".to_string(),
        baseline_value: None,
        depth: 0,
        is_expandable: false,
        is_expanded: false,
        children: Vec::new(),
        diff_status: None,
    };
    assert!(!tag.is_private(), "Invalid tag format should return false");
}

#[test]
fn test_is_private_item_header() {
    let tag = DicomTag {
        tag: "Item #1".to_string(),
        name: "".to_string(),
        vr: "".to_string(),
        value: "".to_string(),
        baseline_value: None,
        depth: 1,
        is_expandable: false,
        is_expanded: false,
        children: Vec::new(),
        diff_status: None,
    };
    assert!(!tag.is_private(), "Item header should return false");
}

// Note: Tests for truncate_value() have been removed since it's a private function.
// The function is tested indirectly through integration tests that verify the full
// DICOM file loading process.
