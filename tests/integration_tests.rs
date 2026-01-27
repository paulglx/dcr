use dcr::dicom::{load_dicom_file, DicomTag};
use dcr::validation::{get_sop_class, validate_type1_fields, SopClass, ValidationResult};
use std::path::PathBuf;

fn fixture_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dicom")
        .join(filename)
}

#[test]
fn test_load_valid_ct_file() {
    let path = fixture_path("ct-tap.dcm");
    let result = load_dicom_file(&path);

    assert!(result.is_ok(), "Should successfully load valid CT file");
    let tags = result.unwrap();
    assert!(!tags.is_empty(), "Should return non-empty tag vector");
}

#[test]
fn test_load_ct_file_with_missing_data() {
    let path = fixture_path("ct-tap-with-missing-data.dcm");
    let result = load_dicom_file(&path);

    assert!(
        result.is_ok(),
        "Should successfully load CT file with missing data"
    );
    let tags = result.unwrap();
    assert!(!tags.is_empty(), "Should return non-empty tag vector");
}

#[test]
fn test_loaded_files_return_nonempty_tags() {
    let path1 = fixture_path("ct-tap.dcm");
    let path2 = fixture_path("ct-tap-with-missing-data.dcm");

    let tags1 = load_dicom_file(&path1).expect("Failed to load ct-tap.dcm");
    let tags2 = load_dicom_file(&path2).expect("Failed to load ct-tap-with-missing-data.dcm");

    assert!(tags1.len() > 10, "Complete CT file should have many tags");
    assert!(
        tags2.len() > 10,
        "Incomplete CT file should still have many tags"
    );
}

#[test]
fn test_load_nonexistent_file_error() {
    let path = fixture_path("nonexistent-file.dcm");
    let result = load_dicom_file(&path);

    assert!(result.is_err(), "Should return error for non-existent file");
}

#[test]
fn test_essential_dicom_tags_present() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    let has_tag = |tag_str: &str| tags.iter().any(|t| t.tag == tag_str);

    assert!(
        has_tag("(0008,0016)"),
        "Should have SOPClassUID (0008,0016)"
    );
    assert!(
        has_tag("(0008,0018)"),
        "Should have SOPInstanceUID (0008,0018)"
    );

    assert!(
        has_tag("(0020,000D)"),
        "Should have StudyInstanceUID (0020,000D)"
    );

    assert!(has_tag("(0008,0060)"), "Should have Modality (0008,0060)");
    assert!(
        has_tag("(0020,000E)"),
        "Should have SeriesInstanceUID (0020,000E)"
    );

    assert!(
        has_tag("(0010,0010)"),
        "Should have PatientName (0010,0010)"
    );
}

#[test]
fn test_tag_format_correct() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    // All tags should match the (GGGG,EEEE) format or be Item headers
    for tag in &tags {
        let is_valid_format =
            tag.tag.starts_with('(') && tag.tag.contains(',') && tag.tag.ends_with(')')
                || tag.tag.starts_with("Item #");
        assert!(
            is_valid_format,
            "Tag should have format (GGGG,EEEE) or 'Item #N', got: {}",
            tag.tag
        );
    }
}

#[test]
fn test_tag_names_populated_from_dictionary() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    // Find specific tags and verify their names
    let patient_name_tag = tags.iter().find(|t| t.tag == "(0010,0010)");
    assert!(patient_name_tag.is_some(), "Should find PatientName tag");
    assert_eq!(
        patient_name_tag.unwrap().name,
        "PatientName",
        "Tag name should be from dictionary"
    );

    let modality_tag = tags.iter().find(|t| t.tag == "(0008,0060)");
    assert!(modality_tag.is_some(), "Should find Modality tag");
    assert_eq!(
        modality_tag.unwrap().name,
        "Modality",
        "Tag name should be from dictionary"
    );
}

#[test]
fn test_vr_fields_populated() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    // Find specific tags and verify their VRs
    let patient_name_tag = tags.iter().find(|t| t.tag == "(0010,0010)");
    assert!(patient_name_tag.is_some(), "Should find PatientName tag");
    assert_eq!(
        patient_name_tag.unwrap().vr,
        "PN",
        "PatientName should have VR of PN"
    );

    let modality_tag = tags.iter().find(|t| t.tag == "(0008,0060)");
    assert!(modality_tag.is_some(), "Should find Modality tag");
    assert_eq!(
        modality_tag.unwrap().vr,
        "CS",
        "Modality should have VR of CS"
    );
}

#[test]
fn test_values_extracted_correctly() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    // Find modality tag and verify its value
    let modality_tag = tags.iter().find(|t| t.tag == "(0008,0060)");
    assert!(modality_tag.is_some(), "Should find Modality tag");

    let modality_value = &modality_tag.unwrap().value;
    assert!(
        !modality_value.is_empty(),
        "Modality value should not be empty"
    );
    assert_eq!(modality_value.trim(), "CT", "Modality value should be CT");
}

#[test]
fn test_sequences_marked_as_expandable() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    // Find sequence tags
    let sequences: Vec<&DicomTag> = tags
        .iter()
        .filter(|t| t.value.contains("<Sequence with"))
        .collect();

    if !sequences.is_empty() {
        for seq in sequences {
            if !seq.children.is_empty() {
                assert!(
                    seq.is_expandable,
                    "Sequence {} with children should be expandable",
                    seq.tag
                );
            }
        }
    }
}

#[test]
fn test_sequences_have_children() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    // Find sequence tags with items
    let sequences_with_items: Vec<&DicomTag> = tags
        .iter()
        .filter(|t| t.value.contains("<Sequence with") && !t.value.contains("0 item(s)>"))
        .collect();

    if !sequences_with_items.is_empty() {
        for seq in sequences_with_items {
            assert!(
                !seq.children.is_empty(),
                "Sequence {} with items should have children",
                seq.tag
            );
        }
    }
}

#[test]
fn test_validate_complete_ct_file_valid() {
    let path = fixture_path("ct-tap.dcm");
    let result = validate_type1_fields(&path);

    assert!(result.is_ok(), "Validation should succeed");
    match result.unwrap() {
        ValidationResult::Valid => {
            // Expected outcome
        }
        ValidationResult::Invalid(missing) => {
            panic!(
                "Complete CT file should be valid, but found missing tags: {:?}",
                missing
            );
        }
        ValidationResult::NotApplicable => {
            panic!("CT file validation should be applicable");
        }
    }
}

#[test]
fn test_validate_incomplete_ct_file_invalid() {
    let path = fixture_path("ct-tap-with-missing-data.dcm");
    let result = validate_type1_fields(&path);

    assert!(result.is_ok(), "Validation should succeed");
    match result.unwrap() {
        ValidationResult::Valid => {
            panic!("Incomplete CT file should be invalid");
        }
        ValidationResult::Invalid(missing) => {
            assert!(!missing.is_empty(), "Should have at least one missing tag");
        }
        ValidationResult::NotApplicable => {
            panic!("CT file validation should be applicable");
        }
    }
}

#[test]
fn test_get_sop_class_returns_ct_for_complete_file() {
    let path = fixture_path("ct-tap.dcm");
    let result = get_sop_class(&path);

    assert!(result.is_ok(), "Should successfully get SOP class");
    match result.unwrap() {
        SopClass::Ct => {
            // Expected outcome
        }
        other => panic!("Expected SopClass::Ct, got {:?}", other),
    }
}

#[test]
fn test_get_sop_class_returns_ct_for_incomplete_file() {
    let path = fixture_path("ct-tap-with-missing-data.dcm");
    let result = get_sop_class(&path);

    assert!(result.is_ok(), "Should successfully get SOP class");
    match result.unwrap() {
        SopClass::Ct => {
            // Expected outcome
        }
        other => panic!("Expected SopClass::Ct, got {:?}", other),
    }
}

#[test]
fn test_incomplete_file_reports_specific_missing_fields() {
    let path = fixture_path("ct-tap-with-missing-data.dcm");
    let result = validate_type1_fields(&path);

    assert!(result.is_ok(), "Validation should succeed");
    if let ValidationResult::Invalid(missing) = result.unwrap() {
        // Verify that missing field names are meaningful (not empty)
        for tag_name in &missing {
            assert!(
                !tag_name.is_empty(),
                "Missing tag names should not be empty"
            );
            assert!(
                tag_name.chars().all(|c| c.is_alphanumeric()),
                "Tag names should be alphanumeric: {}",
                tag_name
            );
        }

        // Print missing tags for informational purposes
        println!(
            "Missing Type 1 fields in ct-tap-with-missing-data.dcm: {:?}",
            missing
        );
    } else {
        panic!("Expected ValidationResult::Invalid for incomplete file");
    }
}

#[test]
fn test_is_private_on_real_file_standard_tags() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    // Check that standard tags (even group numbers) are not marked as private
    let patient_tags: Vec<&DicomTag> = tags
        .iter()
        .filter(|t| t.tag.starts_with("(0010,"))
        .collect();

    assert!(!patient_tags.is_empty(), "Should have patient tags");
    for tag in patient_tags {
        assert!(
            !tag.is_private(),
            "Patient tags (0010,xxxx) should not be private"
        );
    }

    let study_tags: Vec<&DicomTag> = tags
        .iter()
        .filter(|t| t.tag.starts_with("(0020,"))
        .collect();

    if !study_tags.is_empty() {
        for tag in study_tags {
            assert!(
                !tag.is_private(),
                "Study tags (0020,xxxx) should not be private"
            );
        }
    }
}

#[test]
fn test_is_private_on_real_file_private_tags() {
    let path = fixture_path("ct-tap.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    let private_tags: Vec<&DicomTag> = tags
        .iter()
        .filter(|t| {
            if let Some(group_str) = t.tag.get(1..5) {
                if let Ok(group) = u16::from_str_radix(group_str, 16) {
                    return group % 2 == 1;
                }
            }
            false
        })
        .collect();

    for tag in private_tags {
        assert!(
            tag.is_private(),
            "Tags with odd group numbers should be marked as private: {}",
            tag.tag
        );
    }
}

#[test]
fn test_is_private_on_incomplete_file() {
    let path = fixture_path("ct-tap-with-missing-data.dcm");
    let tags = load_dicom_file(&path).expect("Failed to load CT file");

    let standard_tags: Vec<&DicomTag> = tags
        .iter()
        .filter(|t| t.tag.starts_with("(00") && t.tag.contains(','))
        .collect();

    if !standard_tags.is_empty() {
        for tag in standard_tags {
            if let Some(group_str) = tag.tag.get(1..5) {
                if let Ok(group) = u16::from_str_radix(group_str, 16) {
                    if group % 2 == 0 {
                        assert!(
                            !tag.is_private(),
                            "Standard tag {} should not be private",
                            tag.tag
                        );
                    }
                }
            }
        }
    }
}
