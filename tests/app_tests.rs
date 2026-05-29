use std::path::PathBuf;

use dcr::app::App;
use dcr::dicom::DicomTag;
use dcr::validation::{SopClass, ValidationResult};

fn create_test_tag(tag: &str, name: &str, depth: usize, expandable: bool, children: Vec<DicomTag>) -> DicomTag {
    DicomTag {
        tag: tag.to_string(),
        name: name.to_string(),
        vr: "LO".to_string(),
        value: "test value".to_string(),
        baseline_value: None,
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
    assert_eq!(app.tags.visible.len(), 0, "Empty input should produce empty output");
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
    assert_eq!(app.tags.visible.len(), 3, "Flat list should have all 3 tags visible");
    assert_eq!(app.tags.visible[0].tag, "(0010,0010)");
    assert_eq!(app.tags.visible[1].tag, "(0010,0020)");
    assert_eq!(app.tags.visible[2].tag, "(0010,0030)");
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
    assert_eq!(app.tags.visible.len(), 2, "Collapsed sequence should only show parent");
    assert_eq!(app.tags.visible[0].tag, "(0010,0010)");
    assert_eq!(app.tags.visible[1].tag, "(0008,1110)");
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
    assert_eq!(app.tags.visible.len(), 4, "Expanded sequence should show parent + 2 children");
    assert_eq!(app.tags.visible[0].tag, "(0010,0010)");
    assert_eq!(app.tags.visible[1].tag, "(0008,1110)");
    assert_eq!(app.tags.visible[2].tag, "(0008,0100)");
    assert_eq!(app.tags.visible[3].tag, "(0008,0102)");
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
    assert_eq!(app.tags.visible.len(), 3, "Fully expanded nested structure should show all levels");
    assert_eq!(app.tags.visible[0].tag, "(0008,1110)");
    assert_eq!(app.tags.visible[1].tag, "Item #1");
    assert_eq!(app.tags.visible[2].tag, "(0020,0032)");
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
    assert_eq!(app.tags.visible.len(), 2, "Partially expanded should only show expanded levels");
    assert_eq!(app.tags.visible[0].tag, "(0008,1110)");
    assert_eq!(app.tags.visible[1].tag, "Item #1");
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
    
    assert_eq!(app.meta.name, "test.dcm");
    assert!(!app.should_quit);
    assert!(!app.search.active);
    assert_eq!(app.search.query, "");
    assert_eq!(app.tags.visible.len(), 2);
    assert_eq!(app.tags.all.len(), 2);
    assert!(app.tags.table_state.selected().is_some());
    assert_eq!(app.tags.table_state.selected().unwrap(), 0);
}

#[test]
fn test_app_new_with_empty_tags() {
    let app = App::new(
        Vec::new(),
        "empty.dcm".to_string(),
        ValidationResult::Valid,
        SopClass::Unknown,
    );
    
    assert_eq!(app.tags.visible.len(), 0);
    assert!(app.tags.table_state.selected().is_none());
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
    
    match &app.meta.validation_result {
        ValidationResult::Invalid(missing) => {
            assert_eq!(missing.len(), 1);
            assert_eq!(missing[0], "Modality");
        }
        _ => panic!("Expected Invalid validation result"),
    }
    
    match &app.meta.sop_class {
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
    
    assert_eq!(app.tags.visible.len(), 4);
    for (i, tag) in app.tags.visible.iter().enumerate() {
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
    
    assert_eq!(app.tags.visible.len(), 3, "Should show seq1 + its child + seq2");
    assert_eq!(app.tags.visible[0].tag, "(0008,1110)");
    assert_eq!(app.tags.visible[1].tag, "(0008,0100)");
    assert_eq!(app.tags.visible[2].tag, "(0040,0260)");
}

// --- Scroll tests ---

fn create_app_with_n_tags(n: usize) -> App {
    let tags: Vec<DicomTag> = (0..n)
        .map(|i| create_test_tag(&format!("({:04X},0000)", i), &format!("Tag{i}"), 0, false, Vec::new()))
        .collect();
    App::new(tags, "test.dcm".to_string(), ValidationResult::Valid, SopClass::Ct)
}

#[test]
fn scroll_down_from_zero() {
    let mut app = create_app_with_n_tags(5);
    assert_eq!(app.tags.table_state.selected(), Some(0));
    app.tags.scroll_down(1);
    assert_eq!(app.tags.table_state.selected(), Some(1));
}

#[test]
fn scroll_down_clamps_to_last() {
    let mut app = create_app_with_n_tags(5);
    app.tags.scroll_down(100);
    assert_eq!(app.tags.table_state.selected(), Some(4));
}

#[test]
fn scroll_down_on_empty_is_noop() {
    let mut app = create_app_with_n_tags(0);
    app.tags.scroll_down(1);
    assert_eq!(app.tags.table_state.selected(), None);
}

#[test]
fn scroll_up_from_middle() {
    let mut app = create_app_with_n_tags(5);
    app.tags.table_state.select(Some(2));
    app.tags.scroll_up(1);
    assert_eq!(app.tags.table_state.selected(), Some(1));
}

#[test]
fn scroll_up_at_zero_stays_at_zero() {
    let mut app = create_app_with_n_tags(5);
    assert_eq!(app.tags.table_state.selected(), Some(0));
    app.tags.scroll_up(1);
    assert_eq!(app.tags.table_state.selected(), Some(0));
}

#[test]
fn scroll_up_on_empty_is_noop() {
    let mut app = create_app_with_n_tags(0);
    app.tags.scroll_up(1);
    assert_eq!(app.tags.table_state.selected(), None);
}

// --- Search tests ---

#[test]
fn filter_tags_empty_query_shows_all() {
    let mut app = create_app_with_n_tags(3);
    app.search.query = String::new();
    app.tags.filter(&app.search.query);
    assert_eq!(app.tags.visible.len(), 3);
    assert!(app.tags.filtered.is_none());
}

#[test]
fn filter_tags_matching_tag_string() {
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
        create_test_tag("(0010,0020)", "PatientID", 0, false, Vec::new()),
        create_test_tag("(0008,0060)", "Modality", 0, false, Vec::new()),
    ];
    let mut app = App::new(tags, "test.dcm".to_string(), ValidationResult::Valid, SopClass::Ct);
    app.search.query = "0010".to_string();
    app.tags.filter(&app.search.query);
    assert_eq!(app.tags.visible.len(), 2);
}

#[test]
fn filter_tags_matching_name_case_insensitive() {
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
        create_test_tag("(0008,0060)", "Modality", 0, false, Vec::new()),
    ];
    let mut app = App::new(tags, "test.dcm".to_string(), ValidationResult::Valid, SopClass::Ct);
    app.search.query = "modality".to_string();
    app.tags.filter(&app.search.query);
    assert_eq!(app.tags.visible.len(), 1);
    assert_eq!(app.tags.visible[0].name, "Modality");
}

#[test]
fn filter_tags_no_matches() {
    let mut app = create_app_with_n_tags(3);
    app.search.query = "zzzzzzz".to_string();
    app.tags.filter(&app.search.query);
    assert_eq!(app.tags.visible.len(), 0);
}

#[test]
fn reset_selection_with_tags_selects_zero() {
    let mut app = create_app_with_n_tags(3);
    app.tags.table_state.select(Some(2));
    app.tags.reset_selection();
    assert_eq!(app.tags.table_state.selected(), Some(0));
}

#[test]
fn reset_selection_empty_selects_none() {
    let mut app = create_app_with_n_tags(0);
    app.tags.reset_selection();
    assert_eq!(app.tags.table_state.selected(), None);
}

// --- Tree tests ---

#[test]
fn expand_selected_on_collapsed_expandable() {
    let children = vec![
        create_test_tag("(0008,0100)", "CodeValue", 1, false, Vec::new()),
    ];
    let tags = vec![
        create_test_tag("(0008,1110)", "ReferencedStudy", 0, true, children),
    ];
    let mut app = App::new(tags, "test.dcm".to_string(), ValidationResult::Valid, SopClass::Ct);
    assert_eq!(app.tags.visible.len(), 1);

    app.tags.expand_selected();

    assert_eq!(app.tags.visible.len(), 2);
    assert_eq!(app.tags.visible[1].tag, "(0008,0100)");
}

#[test]
fn expand_selected_on_non_expandable_is_noop() {
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
    ];
    let mut app = App::new(tags, "test.dcm".to_string(), ValidationResult::Valid, SopClass::Ct);
    app.tags.expand_selected();
    assert_eq!(app.tags.visible.len(), 1);
}

#[test]
fn collapse_parent_on_child_selects_parent() {
    let children = vec![
        create_test_tag("(0008,0100)", "CodeValue", 1, false, Vec::new()),
    ];
    let mut parent = create_test_tag("(0008,1110)", "ReferencedStudy", 0, true, children);
    parent.is_expanded = true;

    let tags = vec![parent];
    let mut app = App::new(tags, "test.dcm".to_string(), ValidationResult::Valid, SopClass::Ct);
    assert_eq!(app.tags.visible.len(), 2);

    app.tags.table_state.select(Some(1));
    app.tags.collapse_parent();

    assert_eq!(app.tags.visible.len(), 1);
    assert_eq!(app.tags.table_state.selected(), Some(0));
}

#[test]
fn collapse_parent_on_root_is_noop() {
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
    ];
    let mut app = App::new(tags, "test.dcm".to_string(), ValidationResult::Valid, SopClass::Ct);
    app.tags.collapse_parent();
    assert_eq!(app.tags.visible.len(), 1);
    assert_eq!(app.tags.table_state.selected(), Some(0));
}

// --- new_with_diff constructor tests ---

#[test]
fn new_with_diff_stores_fields_correctly() {
    let tags = vec![
        create_test_tag("(0010,0010)", "PatientName", 0, false, Vec::new()),
    ];
    let app = App::new_with_diff(
        tags,
        "baseline.dcm".to_string(),
        Some("modified.dcm".to_string()),
        ValidationResult::Valid,
        SopClass::Ct,
        true,
        Some(PathBuf::from("/tmp/test.dcm")),
        None,
    );

    assert!(app.meta.diff_mode);
    assert_eq!(app.meta.modified_name, Some("modified.dcm".to_string()));
    assert_eq!(app.meta.path, Some(PathBuf::from("/tmp/test.dcm")));
    assert_eq!(app.meta.name, "baseline.dcm");
    assert_eq!(app.tags.visible.len(), 1);
    assert_eq!(app.tags.table_state.selected(), Some(0));
}
