use dcr::app::App;
use dcr::dicom::DicomTag;
use dcr::validation::{SopClass, ValidationResult};

fn create_test_tag(tag: &str, name: &str, depth: usize, expandable: bool, children: Vec<DicomTag>) -> DicomTag {
    DicomTag {
        tag: tag.to_string(),
        name: name.to_string(),
        vr: "LO".to_string(),
        value: "test value".to_string(),
        depth,
        is_expandable: expandable,
        is_expanded: false,
        children,
        diff_status: None,
    }
}

#[test]
fn test_build_visible_tags_from_empty() {
    let app = App::new(
        Vec::new(),
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    assert_eq!(app.tags.len(), 0, "Empty input should produce empty output");
}

#[test]
fn test_build_visible_tags_from_flat_list() {
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
        create_test_tag("(0010,0020)", "PatientID", 0, false, Vec::new()),
        create_test_tag("(0010,0030)", "PatientBirthDate", 0, false, Vec::new()),
    ];
    let app = App::new(
        tags,
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    assert_eq!(app.tags.len(), 3, "Flat list should have all 3 tags visible");
    assert_eq!(app.tags[0].tag, "(0010,0010)");
    assert_eq!(app.tags[1].tag, "(0010,0020)");
    assert_eq!(app.tags[2].tag, "(0010,0030)");
}

#[test]
fn test_build_visible_tags_from_collapsed_sequence() {
    let children = vec![
        create_test_tag("(0008,0100)", "CodeValue", 1, false, Vec::new()),
        create_test_tag("(0008,0102)", "CodingScheme", 1, false, Vec::new()),
    ];
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
        create_test_tag("(0008,1110)", "ReferencedStudy", 0, true, children),
    ];
    let app = App::new(
        tags,
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    assert_eq!(app.tags.len(), 2, "Collapsed sequence should only show parent");
    assert_eq!(app.tags[0].tag, "(0010,0010)");
    assert_eq!(app.tags[1].tag, "(0008,1110)");
}

#[test]
fn test_build_visible_tags_from_expanded_sequence() {
    let children = vec![
        create_test_tag("(0008,0100)", "CodeValue", 1, false, Vec::new()),
        create_test_tag("(0008,0102)", "CodingScheme", 1, false, Vec::new()),
    ];
    let mut parent = create_test_tag("(0008,1110)", "ReferencedStudy", 0, true, children.clone());
    parent.is_expanded = true;
    
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
        parent,
    ];
    let app = App::new(
        tags,
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    assert_eq!(app.tags.len(), 4, "Expanded sequence should show parent + 2 children");
    assert_eq!(app.tags[0].tag, "(0010,0010)");
    assert_eq!(app.tags[1].tag, "(0008,1110)");
    assert_eq!(app.tags[2].tag, "(0008,0100)");
    assert_eq!(app.tags[3].tag, "(0008,0102)");
}

#[test]
fn test_collect_visible_tags_nested() {
    let grandchildren = vec![
        create_test_tag("(0020,0032)", "Position", 2, false, Vec::new()),
    ];
    let mut child = create_test_tag("Item #1", "", 1, true, grandchildren);
    child.is_expanded = true;
    
    let mut parent = create_test_tag("(0008,1110)", "ReferencedStudy", 0, true, vec![child]);
    parent.is_expanded = true;
    
    let tags = vec![parent];
    let app = App::new(
        tags,
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    assert_eq!(app.tags.len(), 3, "Fully expanded nested structure should show all levels");
    assert_eq!(app.tags[0].tag, "(0008,1110)");
    assert_eq!(app.tags[1].tag, "Item #1");
    assert_eq!(app.tags[2].tag, "(0020,0032)");
}

#[test]
fn test_collect_visible_tags_partially_expanded() {
    let grandchildren = vec![
        create_test_tag("(0020,0032)", "Position", 2, false, Vec::new()),
    ];
    let child_expanded = create_test_tag("Item #1", "", 1, true, grandchildren);
    // child_expanded.is_expanded is false (collapsed)
    
    let mut parent = create_test_tag("(0008,1110)", "ReferencedStudy", 0, true, vec![child_expanded]);
    parent.is_expanded = true;
    
    let tags = vec![parent];
    let app = App::new(
        tags,
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    assert_eq!(app.tags.len(), 2, "Partially expanded should only show expanded levels");
    assert_eq!(app.tags[0].tag, "(0008,1110)");
    assert_eq!(app.tags[1].tag, "Item #1");
}

#[test]
fn test_app_new_initializes_correctly() {
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
        create_test_tag("(0010,0020)", "PatientID", 0, false, Vec::new()),
    ];
    let app = App::new(
        tags.clone(),
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    
    assert_eq!(app.file_name, "test.dcm");
    assert!(!app.should_quit);
    assert!(!app.search_mode);
    assert_eq!(app.search_query, "");
    assert_eq!(app.tags.len(), 2);
    assert_eq!(app.all_tags.len(), 2);
    assert!(app.table_state.selected().is_some());
    assert_eq!(app.table_state.selected().unwrap(), 0);
}

#[test]
fn test_app_new_with_empty_tags() {
    let app = App::new(
        Vec::new(),
        "empty.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Unknown,
    );
    
    assert_eq!(app.tags.len(), 0);
    assert!(app.table_state.selected().is_none());
}

#[test]
fn test_app_state_initialization() {
    let tags = vec![create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new())];
    let app = App::new(
        tags,
        "test.dcm".to_string(),
        ValidationResult::Invalid(vec!["Modality".to_string()]),
        SopClass::Mr,
    );
    
    match &app.validation_result {
        ValidationResult::Invalid(missing) => {
            assert_eq!(missing.len(), 1);
            assert_eq!(missing[0], "Modality");
        }
        _ => panic!("Expected Invalid validation result"),
    }
    
    match &app.sop_class {
        SopClass::Mr => assert!(true),
        _ => panic!("Expected Mr SOP class"),
    }
}

#[test]
fn test_collect_visible_tags_preserves_order() {
    let tags = vec![
        create_test_tag("(0008,0005)", "Tag1", 0, false, Vec::new()),
        create_test_tag("(0008,0008)", "Tag2", 0, false, Vec::new()),
        create_test_tag("(0010,0010)", "Tag3", 0, false, Vec::new()),
        create_test_tag("(0020,000D)", "Tag4", 0, false, Vec::new()),
    ];
    let app = App::new(
        tags,
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    
    assert_eq!(app.tags.len(), 4);
    for (i, tag) in app.tags.iter().enumerate() {
        assert_eq!(tag.name, format!("Tag{}", i + 1));
    }
}

#[test]
fn test_build_visible_tags_multiple_sequences() {
    let children1 = vec![create_test_tag("(0008,0100)", "Code1", 1, false, Vec::new())];
    let children2 = vec![create_test_tag("(0040,0008)", "Code2", 1, false, Vec::new())];
    
    let mut seq1 = create_test_tag("(0008,1110)", "Seq1", 0, true, children1);
    seq1.is_expanded = true;
    
    let seq2 = create_test_tag("(0040,0260)", "Seq2", 0, true, children2);
    // seq2 is collapsed
    
    let tags = vec![seq1, seq2];
    let app = App::new(
        tags,
        "test.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Ct,
    );
    
    assert_eq!(app.tags.len(), 3, "Should show seq1 + its child + seq2");
    assert_eq!(app.tags[0].tag, "(0008,1110)");
    assert_eq!(app.tags[1].tag, "(0008,0100)");
    assert_eq!(app.tags[2].tag, "(0040,0260)");
}
