use std::path::PathBuf;

use dcr::app::{App, DiffSide};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dicom")
        .join(name)
}

fn baseline() -> PathBuf {
    fixture_path("ct-tap.dcm")
}

fn modified() -> PathBuf {
    fixture_path("ct-tap-with-missing-data.dcm")
}

#[test]
fn marking_a_then_b_opens_diff() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    assert!(!app.meta.diff_mode, "diff should wait for the second mark");
    assert_eq!(app.marks.baseline, Some(baseline()));

    app.mark_path_for_diff(DiffSide::Modified, &modified());
    assert!(app.meta.diff_mode);
    assert_eq!(app.meta.name, "ct-tap.dcm");
    assert_eq!(
        app.meta.modified_name,
        Some("ct-tap-with-missing-data.dcm".to_string())
    );
    assert_eq!(
        app.meta.path,
        Some(modified()),
        "validation follows modified"
    );
    assert_eq!(
        app.meta.baseline_path,
        Some(baseline()),
        "preview blends baseline against modified"
    );
    assert!(app.has_dicom_loaded());
    assert!(app.tags.all.iter().all(|t| t.diff_status.is_some()));
}

#[test]
fn marking_b_first_is_order_independent() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Modified, &modified());
    assert!(!app.meta.diff_mode);

    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    assert!(app.meta.diff_mode);
    assert_eq!(app.meta.name, "ct-tap.dcm");
}

#[test]
fn marking_same_file_on_both_sides_is_rejected() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &baseline());

    assert!(!app.meta.diff_mode);
    assert!(app.marks.modified.is_none());
    assert_eq!(app.marks.message, Some("Already marked as A".to_string()));
}

#[test]
fn remarking_same_side_moves_the_mark() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Baseline, &modified());

    assert!(!app.meta.diff_mode);
    assert_eq!(app.marks.baseline, Some(modified()));
}

#[test]
fn marking_a_directory_is_rejected() {
    let mut app = App::new_explorer(None);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    app.mark_path_for_diff(DiffSide::Baseline, &dir);

    assert!(app.marks.baseline.is_none());
    assert_eq!(
        app.marks.message,
        Some("Only files can be marked for diff".to_string())
    );
}

#[test]
fn swap_reverses_baseline_and_modified() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &modified());

    app.swap_diff_marks();

    assert!(app.meta.diff_mode);
    assert_eq!(app.marks.baseline, Some(modified()));
    assert_eq!(app.marks.modified, Some(baseline()));
    assert_eq!(app.meta.name, "ct-tap-with-missing-data.dcm");
    assert_eq!(app.meta.modified_name, Some("ct-tap.dcm".to_string()));
    assert_eq!(app.meta.path, Some(baseline()));
    assert_eq!(app.meta.baseline_path, Some(modified()));
}

#[test]
fn swap_without_open_diff_is_noop() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.swap_diff_marks();
    assert_eq!(app.marks.baseline, Some(baseline()));
    assert!(app.marks.modified.is_none());
}

#[test]
fn exit_diff_reopens_baseline_and_clears_marks() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &modified());
    assert!(app.meta.diff_mode);

    app.exit_diff();

    assert!(!app.meta.diff_mode);
    assert!(app.meta.modified_name.is_none());
    assert!(!app.marks.any());
    assert_eq!(app.meta.name, "ct-tap.dcm");
    assert_eq!(app.meta.path, Some(baseline()));
    assert!(app.meta.baseline_path.is_none());
    assert!(app.has_dicom_loaded());
    assert!(app.tags.all.iter().all(|t| t.diff_status.is_none()));
}

#[test]
fn remarking_while_diff_open_recomputes() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &modified());
    assert!(app.meta.diff_mode);

    // Retargeting onto the file carrying the other mark is rejected.
    app.mark_path_for_diff(DiffSide::Baseline, &modified());
    assert_eq!(app.marks.message, Some("Already marked as B".to_string()));
    assert_eq!(app.marks.baseline, Some(baseline()));

    // Retargeting onto a third file recomputes the diff with the new pair.
    let third = std::env::temp_dir().join("dcr-ui-diff-third.dcm");
    std::fs::copy(baseline(), &third).expect("failed to copy fixture");
    app.mark_path_for_diff(DiffSide::Modified, &third);

    assert!(app.meta.diff_mode);
    assert_eq!(app.marks.modified, Some(third.clone()));
    assert_eq!(app.meta.name, "ct-tap.dcm");
    assert_eq!(
        app.meta.modified_name,
        Some("dcr-ui-diff-third.dcm".to_string())
    );
    assert_eq!(app.meta.path, Some(third));
}

#[test]
fn diff_failure_keeps_view_and_reports_message() {
    let mut app = App::new_explorer(None);
    let not_dicom = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("not-a-dicom.txt");

    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &not_dicom);

    assert!(!app.meta.diff_mode);
    assert!(app
        .marks
        .message
        .as_deref()
        .is_some_and(|m| m.starts_with("Diff failed:")));
    assert_eq!(app.marks.modified, Some(not_dicom), "marks are kept");
}

#[test]
fn changes_only_hides_unchanged_rows() {
    use dcr::dicom::DiffStatus;

    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &modified());
    assert!(!app.tags.changes_only);
    let full_len = app.tags.visible.len();
    assert!(app
        .tags
        .visible
        .iter()
        .any(|t| t.diff_status == Some(DiffStatus::Unchanged)));

    app.toggle_changes_only();

    assert!(app.tags.changes_only);
    assert!(!app.tags.visible.is_empty());
    assert!(app.tags.visible.len() < full_len);
    assert!(
        app.tags
            .visible
            .iter()
            .all(|t| t.diff_status != Some(DiffStatus::Unchanged)),
        "no unchanged row may remain"
    );
    assert_eq!(app.tags.table_state.selected(), Some(0));

    app.toggle_changes_only();
    assert!(!app.tags.changes_only);
    assert_eq!(app.tags.visible.len(), full_len);
}

#[test]
fn changes_only_composes_with_search_filter() {
    use dcr::dicom::DiffStatus;

    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &modified());
    app.toggle_changes_only();
    let changes_len = app.tags.visible.len();

    app.search.query = "(0008".to_string();
    app.apply_search();

    assert!(app.tags.visible.len() < changes_len);
    assert!(app
        .tags
        .visible
        .iter()
        .all(|t| { t.tag.starts_with("(0008") && t.diff_status != Some(DiffStatus::Unchanged) }));

    app.search.query.clear();
    app.apply_search();
    assert_eq!(
        app.tags.visible.len(),
        changes_len,
        "filter cleared, changes_only kept"
    );
}

#[test]
fn changes_only_survives_remark_and_swap() {
    use dcr::dicom::DiffStatus;

    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &modified());
    app.toggle_changes_only();

    app.swap_diff_marks();
    assert!(app.tags.changes_only);
    assert!(app
        .tags
        .visible
        .iter()
        .all(|t| t.diff_status != Some(DiffStatus::Unchanged)));

    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    assert!(app.tags.changes_only);
    assert!(app
        .tags
        .visible
        .iter()
        .all(|t| t.diff_status != Some(DiffStatus::Unchanged)));
}

#[test]
fn exit_diff_resets_changes_only() {
    let mut app = App::new_explorer(None);
    app.mark_path_for_diff(DiffSide::Baseline, &baseline());
    app.mark_path_for_diff(DiffSide::Modified, &modified());
    app.toggle_changes_only();

    app.exit_diff();

    assert!(!app.tags.changes_only);
    assert!(app.tags.filtered.is_none());
    assert!(app.has_dicom_loaded());
}

#[test]
fn changes_only_is_noop_outside_diff_mode() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&baseline());
    let full_len = app.tags.visible.len();

    app.toggle_changes_only();

    assert!(!app.tags.changes_only);
    assert_eq!(app.tags.visible.len(), full_len);
}
