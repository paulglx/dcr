use dcr::dicom::{compare_dicom_files, DiffStatus};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dicom")
        .join(name)
}

fn non_dicom_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn compare_returns_ok_with_non_empty_results() {
    let result = compare_dicom_files(fixture_path("ct-tap.dcm"), fixture_path("ct-tap-with-missing-data.dcm"));
    let tags = result.expect("compare_dicom_files should return Ok");
    assert!(!tags.is_empty());
}

#[test]
fn result_contains_expected_diff_statuses() {
    let tags = compare_dicom_files(
        fixture_path("ct-tap.dcm"),
        fixture_path("ct-tap-with-missing-data.dcm"),
    )
    .unwrap();

    let has_unchanged = tags.iter().any(|t| t.diff_status == Some(DiffStatus::Unchanged));
    let has_added_or_deleted = tags
        .iter()
        .any(|t| t.diff_status == Some(DiffStatus::Added) || t.diff_status == Some(DiffStatus::Deleted));

    assert!(has_unchanged, "Should contain at least one Unchanged tag");
    assert!(has_added_or_deleted, "Should contain at least one Added or Deleted tag");
}

#[test]
fn all_result_tags_have_diff_status() {
    let tags = compare_dicom_files(
        fixture_path("ct-tap.dcm"),
        fixture_path("ct-tap-with-missing-data.dcm"),
    )
    .unwrap();

    for tag in &tags {
        assert!(tag.diff_status.is_some(), "Tag {} should have diff_status set", tag.tag);
    }
}

#[test]
fn results_are_sorted_by_tag_string() {
    let tags = compare_dicom_files(
        fixture_path("ct-tap.dcm"),
        fixture_path("ct-tap-with-missing-data.dcm"),
    )
    .unwrap();

    for window in tags.windows(2) {
        assert!(
            window[0].tag <= window[1].tag,
            "Tags should be sorted: {} should come before {}",
            window[0].tag,
            window[1].tag
        );
    }
}

#[test]
fn changed_tags_have_baseline_value() {
    let tags = compare_dicom_files(
        fixture_path("ct-tap.dcm"),
        fixture_path("ct-tap-with-missing-data.dcm"),
    )
    .unwrap();

    for tag in &tags {
        if tag.diff_status == Some(DiffStatus::Changed) {
            assert!(
                tag.baseline_value.is_some(),
                "Changed tag {} should have baseline_value",
                tag.tag
            );
        }
    }
}

#[test]
fn nonexistent_file_returns_error() {
    let result = compare_dicom_files(
        fixture_path("nonexistent.dcm"),
        fixture_path("ct-tap.dcm"),
    );
    assert!(result.is_err());
}

#[test]
fn test_diff_with_non_dicom_file_returns_error() {
    let result = compare_dicom_files(
        non_dicom_fixture_path("not-a-dicom.txt"),
        fixture_path("ct-tap.dcm"),
    );
    assert!(result.is_err(), "Should return error when comparing a non-DICOM file");
}
