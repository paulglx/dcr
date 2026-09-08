use std::path::PathBuf;

use dcr::app::state::dirview::Node;
use dcr::app::App;
use dcr::dicom::dicomdir::{
    self, scan_item_offsets, DirIssue, INSTANCE_NUMBER, MODALITY, SERIES_NUMBER,
};
use dcr::dicom::DicomDir;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Synthetic file-set written by pydicom: 1 patient, 1 study, CT series #11 with
/// instances 1, 2, 4 (gap at 3) and MR series #5 with instances 1, 2. One referenced MR
/// file was deleted and a stray `STRAY.dcm` was added next to the DICOMDIR.
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dicomdir")
}

fn dicomdir_path() -> PathBuf {
    fixture_dir().join("DICOMDIR")
}

fn load() -> DicomDir {
    let path = dicomdir_path();
    let obj = dicom::object::open_file(&path).expect("fixture opens");
    DicomDir::from_object(&path, &obj)
}

fn record_types(dir: &DicomDir, indices: &[usize]) -> Vec<String> {
    indices
        .iter()
        .map(|&i| dir.records[i].record_type.clone())
        .collect()
}

// --- parsing -----------------------------------------------------------------------------

#[test]
fn detects_dicomdir_by_sop_class_and_by_name() {
    let path = dicomdir_path();
    let obj = dicom::object::open_file(&path).expect("fixture opens");
    assert!(dicomdir::is_dicomdir(&path, &obj));

    let ct = fixture_dir()
        .parent()
        .unwrap()
        .join("dicom")
        .join("ct-tap.dcm");
    let ct_obj = dicom::object::open_file(&ct).expect("ct fixture opens");
    assert!(!dicomdir::is_dicomdir(&ct, &ct_obj));
    assert!(
        dicomdir::is_dicomdir(&PathBuf::from("/x/dicomdir"), &ct_obj),
        "file name alone is enough"
    );
}

#[test]
fn item_offsets_match_the_root_offset_written_by_the_encoder() {
    let offsets = scan_item_offsets(&dicomdir_path()).expect("offsets scan");
    assert_eq!(offsets.len(), 9);
    // pydicom wrote the root record at byte 398 and links every record by offset.
    assert_eq!(offsets[0], 398);
    let dir = load();
    assert!(dir.linked_by_offsets);
    for (record, offset) in dir.records.iter().zip(&offsets) {
        assert_eq!(record.offset, Some(*offset));
    }
}

#[test]
fn rebuilds_the_hierarchy_from_offsets() {
    let dir = load();
    assert_eq!(dir.records.len(), 9);
    assert_eq!(dir.roots, vec![0]);
    assert_eq!(dir.records[0].record_type, "PATIENT");

    let study = dir.records[0].children.clone();
    assert_eq!(record_types(&dir, &study), vec!["STUDY"]);

    let series = dir.records[study[0]].children.clone();
    assert_eq!(record_types(&dir, &series), vec!["SERIES", "SERIES"]);
    assert_eq!(dir.records[series[0]].str(MODALITY), Some("CT".into()));
    assert_eq!(dir.records[series[0]].str(SERIES_NUMBER), Some("11".into()));
    assert_eq!(dir.records[series[0]].children.len(), 3);
    assert_eq!(dir.records[series[1]].str(MODALITY), Some("MR".into()));
    assert_eq!(dir.records[series[1]].children.len(), 2);

    for &s in &series {
        for &img in &dir.records[s].children {
            assert_eq!(dir.records[img].record_type, "IMAGE");
            assert_eq!(dir.records[img].parent, Some(s));
        }
    }
    assert_eq!(dir.records[0].leaf_count, 5);
    assert_eq!(dir.records[series[0]].leaf_count, 3);
}

#[test]
fn stats_count_levels_modalities_and_files() {
    let dir = load();
    let s = &dir.stats;
    assert_eq!((s.patients, s.studies, s.series, s.leaves), (1, 1, 2, 5));
    assert_eq!(
        s.modalities,
        vec![("CT".to_string(), 1), ("MR".to_string(), 1)]
    );
    assert_eq!(s.in_use, 9);
    assert_eq!(s.referenced_files, 5);
    assert_eq!(s.files_found, 4, "one referenced file was deleted");
}

#[test]
fn reports_missing_and_unreferenced_files() {
    let dir = load();
    assert_eq!(
        dir.issues,
        vec![DirIssue::MissingFiles(1), DirIssue::UnreferencedFiles(1)]
    );
    assert!(!dir.is_healthy());
    assert_eq!(dir.unreferenced, vec![PathBuf::from("STRAY.dcm")]);

    let missing: Vec<&dcr::dicom::dicomdir::DirRecord> = dir
        .records
        .iter()
        .filter(|r| r.file_exists == Some(false))
        .collect();
    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0].str(MODALITY).or(None),
        None,
        "image records carry no modality"
    );
    assert_eq!(
        missing[0].file_name(),
        Some("IM000001"),
        "the last MR instance was the one deleted"
    );
}

#[test]
fn instance_range_reports_gaps() {
    let dir = load();
    let study = dir.records[0].children[0];
    let (ct, mr) = (
        dir.records[study].children[0],
        dir.records[study].children[1],
    );
    let ct_range = dir.instance_range(ct).expect("CT has instance numbers");
    assert_eq!(
        (
            ct_range.min,
            ct_range.max,
            ct_range.gaps,
            ct_range.duplicates
        ),
        (1, 4, 1, 0)
    );
    assert_eq!(ct_range.to_string(), "1–4 (1 gap)");
    let mr_range = dir.instance_range(mr).unwrap();
    assert_eq!(mr_range.to_string(), "1–2");
    assert!(
        dir.instance_range(study).is_none(),
        "series carry no instance numbers"
    );
}

#[test]
fn folders_group_files_by_referenced_directory() {
    let dir = load();
    let folders = dir.folders();
    assert_eq!(folders.len(), 2);
    assert_eq!(folders[0].name, "PT000000/ST000000/SE000000");
    assert_eq!(folders[0].file_count, 3);
    assert_eq!(record_types(&dir, &folders[0].parents), vec!["SERIES"]);
    assert_eq!(folders[1].name, "PT000000/ST000000/SE000001");
    assert_eq!(folders[1].file_count, 2);

    let study = dir.records[0].children[0];
    let ct = dir.records[study].children[0];
    assert_eq!(
        dir.child_folders(ct),
        vec!["PT000000/ST000000/SE000000".to_string()]
    );
    assert_eq!(
        dir.file_path(dir.records[ct].children[0]),
        Some(fixture_dir().join("PT000000/ST000000/SE000000/IM000000"))
    );
}

#[test]
fn record_summaries_are_short_and_readable() {
    let dir = load();
    assert_eq!(dir.records[0].summary(), "PATIENT · Test Patient");
    let study = dir.records[0].children[0];
    assert_eq!(
        dir.records[study].summary(),
        "STUDY · 2024-03-12 · CT ABDOMEN"
    );
    let ct = dir.records[study].children[0];
    assert_eq!(dir.records[ct].summary(), "SERIES · #11 CT");
    let img = dir.records[ct].children[0];
    assert_eq!(dir.records[img].summary(), "IMAGE · #1 IM000000");
    assert_eq!(dir.records[img].int(INSTANCE_NUMBER), Some(1));
}

#[test]
fn names_for_common_uids() {
    assert_eq!(
        dicomdir::sop_class_name("1.2.840.10008.5.1.4.1.1.2"),
        "CT Image"
    );
    assert_eq!(
        dicomdir::sop_class_name("1.2.840.10008.5.1.4.1.1.999"),
        "SOP …999"
    );
    assert_eq!(
        dicomdir::transfer_syntax_name("1.2.840.10008.1.2.4.90"),
        "JPEG 2000 Lossless"
    );
    assert_eq!(dicomdir::transfer_syntax_name("9.9"), "9.9");
    assert_eq!(dicomdir::format_da("20240312"), "2024-03-12");
    assert_eq!(dicomdir::format_da("2024"), "2024");
}

// --- raw tag table enrichment ------------------------------------------------------------

#[test]
fn raw_tag_table_labels_directory_record_items() {
    let obj = dicom::object::open_file(dicomdir_path()).unwrap();
    let tags = dcr::dicom::extract_tags(&obj);
    let seq = tags
        .iter()
        .find(|t| t.tag == "(0004,1220)")
        .expect("directory record sequence");
    assert_eq!(seq.children.len(), 9);
    assert_eq!(seq.children[0].tag, "Item #1");
    assert_eq!(seq.children[0].name, "PATIENT · Test Patient");
    assert_eq!(seq.children[2].name, "SERIES · #11 CT");
    assert_eq!(seq.children[3].name, "IMAGE · #1 IM000000");
}

// --- app integration ---------------------------------------------------------------------

#[test]
fn loading_a_dicomdir_activates_the_directory_view() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    assert!(
        app.has_dicom_loaded(),
        "raw tags are still loaded as a fallback"
    );
    assert!(app.dir_active());

    let view = app.dirview.as_ref().unwrap();
    // Patient and study open by default, series folded, layout folded:
    // PATIENT, STUDY, SERIES CT, SERIES MR, LAYOUT.
    assert_eq!(view.visible.len(), 5);
    assert_eq!(view.visible[0].node, Node::Record(0));
    assert!(view.visible[0].expanded);
    assert!(view.visible[2].expandable && !view.visible[2].expanded);
    assert_eq!(view.visible[4].node, Node::Layout);
    assert_eq!(view.table_state.selected(), Some(0));
}

#[test]
fn preview_follows_the_selection_to_the_middle_instance() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    // Patient selected: middle child chain → study → MR series (index 1 of 2) → IM000001.
    assert_eq!(
        app.preview_path(),
        Some(fixture_dir().join("PT000000/ST000000/SE000001/IM000001"))
    );
    app.nav_down(2); // CT series
    assert_eq!(
        app.preview_path(),
        Some(fixture_dir().join("PT000000/ST000000/SE000000/IM000001")),
        "middle of instances 1, 2, 4"
    );
    app.nav_expand();
    app.nav_down(1); // first CT image
    assert_eq!(
        app.preview_path(),
        Some(fixture_dir().join("PT000000/ST000000/SE000000/IM000000"))
    );
}

#[test]
fn expand_collapse_and_layout_rows() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.nav_down(2);
    app.nav_expand();
    let view = app.dirview.as_ref().unwrap();
    assert_eq!(view.visible.len(), 8, "3 CT images appear");
    assert_eq!(view.visible[3].node, Node::Record(3));
    assert_eq!(view.visible[3].depth, 3);

    app.nav_down(1);
    app.nav_collapse();
    let view = app.dirview.as_ref().unwrap();
    assert_eq!(view.visible.len(), 5);
    assert_eq!(
        view.table_state.selected(),
        Some(2),
        "selection moves to the collapsed series"
    );

    app.nav_down(10); // clamps to LAYOUT
    app.nav_expand();
    let view = app.dirview.as_ref().unwrap();
    let nodes: Vec<Node> = view.visible.iter().map(|r| r.node.clone()).collect();
    assert_eq!(
        &nodes[4..],
        &[
            Node::Layout,
            Node::Folder(0),
            Node::Folder(1),
            Node::Unreferenced
        ]
    );
    app.nav_down(3);
    app.nav_expand();
    let view = app.dirview.as_ref().unwrap();
    assert_eq!(view.visible.last().unwrap().node, Node::UnreferencedFile(0));
    assert_eq!(
        view.open_target(),
        None,
        "the header row itself opens nothing"
    );
}

#[test]
fn search_filters_records_and_shows_ancestors_expanded() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.search.query = "IM000002".into();
    app.apply_search();
    let view = app.dirview.as_ref().unwrap();
    let nodes: Vec<Node> = view.visible.iter().map(|r| r.node.clone()).collect();
    assert_eq!(
        nodes,
        vec![
            Node::Record(0),
            Node::Record(1),
            Node::Record(2),
            Node::Record(5)
        ]
    );
    assert!(view.visible.iter().take(3).all(|r| r.expanded));

    app.search.query = "mr".into();
    app.apply_search();
    let view = app.dirview.as_ref().unwrap();
    assert_eq!(
        view.visible.len(),
        5,
        "patient, study, MR series and its 2 images"
    );

    app.search.query.clear();
    app.apply_search();
    assert_eq!(app.dirview.as_ref().unwrap().visible.len(), 5);
}

#[test]
fn enter_opens_the_referenced_file_and_esc_returns() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.nav_down(2);
    app.nav_expand();
    app.nav_down(1); // IM000000 of the CT series

    app.open_selected_record();
    assert!(!app.dir_active());
    assert!(app.dirview.is_none());
    assert!(app.dir_return.is_some());
    assert_eq!(app.meta.name, "IM000000");
    assert_eq!(
        app.meta.path,
        Some(fixture_dir().join("PT000000/ST000000/SE000000/IM000000"))
    );
    assert!(
        app.tags.all.iter().any(|t| t.tag == "(7FE0,0010)"),
        "image tags loaded"
    );

    assert!(app.return_to_dicomdir());
    assert!(app.dir_active());
    assert!(app.dir_return.is_none());
    assert_eq!(app.meta.name, "DICOMDIR");
    let view = app.dirview.as_ref().unwrap();
    assert_eq!(
        view.visible.len(),
        8,
        "expansion state survives the round trip"
    );
    assert_eq!(
        view.table_state.selected(),
        Some(3),
        "selection survives too"
    );

    assert!(!app.return_to_dicomdir(), "nothing to return to now");
}

#[test]
fn enter_on_a_missing_file_shows_a_message_instead_of_opening() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.nav_down(3); // MR series
    app.nav_expand();
    app.nav_down(2); // IM000001, deleted from disk
    let view = app.dirview.as_ref().unwrap();
    let Node::Record(idx) = view.visible[5].node.clone() else {
        panic!("expected a record row");
    };
    assert_eq!(view.dir.records[idx].file_exists, Some(false));

    app.open_selected_record();
    assert!(app.dir_active(), "still on the directory view");
    assert_eq!(
        app.marks.message.as_deref(),
        Some("Referenced file not found: IM000001")
    );
}

#[test]
fn toggle_raw_shows_tag_table_and_clears_search() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.search.query = "mr".into();
    app.apply_search();

    app.toggle_dir_raw();
    assert!(!app.dir_active());
    assert!(app.dirview.is_some());
    assert!(app.search.query.is_empty());
    assert_eq!(
        app.preview_path(),
        Some(dicomdir_path()),
        "raw view previews the file itself"
    );
    assert!(app.tags.visible.iter().any(|t| t.tag == "(0004,1220)"));

    app.toggle_dir_raw();
    assert!(app.dir_active());
    assert_eq!(
        app.dirview.as_ref().unwrap().visible.len(),
        5,
        "filter was cleared"
    );
}

#[test]
fn loading_another_file_drops_the_directory_state() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.nav_down(2);
    app.nav_expand();
    app.nav_down(1);
    app.open_selected_record();
    assert!(app.dir_return.is_some());

    let ct = fixture_dir()
        .parent()
        .unwrap()
        .join("dicom")
        .join("ct-tap.dcm");
    app.load_dicom_file(&ct);
    assert!(app.dirview.is_none());
    assert!(app.dir_return.is_none());
    assert!(!app.return_to_dicomdir());

    app.clear_dicom_state();
    assert!(app.dirview.is_none());
}

#[test]
fn diff_mode_never_uses_the_directory_view() {
    let mut app = App::new_explorer(None);
    let ct = fixture_dir()
        .parent()
        .unwrap()
        .join("dicom")
        .join("ct-tap.dcm");
    app.mark_path_for_diff(dcr::app::DiffSide::Baseline, &dicomdir_path());
    app.mark_path_for_diff(dcr::app::DiffSide::Modified, &ct);
    assert!(app.meta.diff_mode);
    assert!(app.dirview.is_none());
}

// --- rendering ---------------------------------------------------------------------------

fn render_to_text(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| dcr::ui::render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[test]
fn directory_view_renders_summary_tree_and_health() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.layout.focus = dcr::app::Focus::TagTable;
    app.preview.show = false;
    app.nav_down(2);
    app.nav_expand();
    let text = render_to_text(&mut app, 180, 24);

    assert!(
        text.contains("DICOMDIR · dicomdir"),
        "title names the folder\n{text}"
    );
    assert!(
        text.contains("1 patient · 1 study · 2 series · 5 images"),
        "{text}"
    );
    assert!(text.contains("CT ×1  MR ×1"), "{text}");
    assert!(text.contains("on disk: 4/5"), "{text}");
    assert!(text.contains("PATIENT Test Patient"), "{text}");
    assert!(text.contains("STUDY   2024-03-12  CT ABDOMEN"), "{text}");
    assert!(text.contains("acc ACC123"), "{text}");
    // pydicom's default SERIES record carries no description.
    assert!(
        text.contains("SERIES  #11   CT  (no description)"),
        "{text}"
    );
    assert!(text.contains("1–4 (1 gap)"), "{text}");
    assert!(text.contains("PT000000/ST000000/SE000000/"), "{text}");
    assert!(text.contains("IMAGE   #1     IM000000"), "{text}");
    assert!(text.contains("CT Image"), "{text}");
    assert!(text.contains("Explicit LE"), "{text}");
    assert!(text.contains("✓"), "{text}");
    assert!(
        text.contains("LAYOUT  2 folders · 5 files referenced · 1 unreferenced on disk"),
        "{text}"
    );
    assert!(text.contains("✗ DICOMDIR: 2 issues"), "{text}");
    assert!(
        text.contains("1 missing file, 1 unreferenced file"),
        "{text}"
    );
    assert!(text.contains("Links: byte offsets"), "{text}");
    assert!(text.contains("t: Raw tags"), "{text}");
}

#[test]
fn missing_files_are_flagged_on_their_row() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.preview.show = false;
    app.nav_down(3);
    app.nav_expand();
    let text = render_to_text(&mut app, 180, 24);
    assert!(text.contains("✗ missing"), "{text}");
    assert!(text.contains("MR Image"), "{text}");
}

#[test]
fn raw_toggle_renders_the_tag_table_with_labelled_items() {
    let mut app = App::new_explorer(None);
    app.load_dicom_file(&dicomdir_path());
    app.layout.focus = dcr::app::Focus::TagTable;
    app.preview.show = false;
    app.toggle_dir_raw();
    app.tags.table_state.select(Some(
        app.tags
            .visible
            .iter()
            .position(|t| t.tag == "(0004,1220)")
            .unwrap(),
    ));
    app.nav_expand();
    let text = render_to_text(&mut app, 160, 30);
    assert!(text.contains("DICOMDIR (raw tags): DICOMDIR"), "{text}");
    assert!(text.contains("Item #1"), "{text}");
    assert!(text.contains("PATIENT · Test Patient"), "{text}");
    assert!(text.contains("SERIES · #11 CT"), "{text}");
    assert!(text.contains("t: Directory"), "{text}");
    assert!(
        text.contains("✗ DICOMDIR: 2 issues"),
        "health pane stays in raw view\n{text}"
    );
}
