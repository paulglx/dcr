use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use dcr::app::state::sort::{natural_cmp, SortKey, SortMenu};
use dcr::app::App;
use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

/// One test file: name on disk plus the attributes to write.
struct Instance<'a> {
    name: &'a str,
    uid: &'a str,
    instance_number: Option<i32>,
    content: Option<(&'a str, &'a str)>,
    acquisition_dt: Option<&'a str>,
}

fn write_instance(dir: &Path, inst: &Instance) {
    let mut obj = InMemDicomObject::new_empty();
    obj.put(DataElement::new(
        tags::SOP_CLASS_UID,
        VR::UI,
        PrimitiveValue::from(uids::CT_IMAGE_STORAGE),
    ));
    obj.put(DataElement::new(
        tags::SOP_INSTANCE_UID,
        VR::UI,
        PrimitiveValue::from(inst.uid),
    ));
    if let Some(n) = inst.instance_number {
        obj.put(DataElement::new(
            tags::INSTANCE_NUMBER,
            VR::IS,
            PrimitiveValue::from(n.to_string()),
        ));
    }
    if let Some((date, time)) = inst.content {
        obj.put(DataElement::new(
            tags::CONTENT_DATE,
            VR::DA,
            PrimitiveValue::from(date),
        ));
        obj.put(DataElement::new(
            tags::CONTENT_TIME,
            VR::TM,
            PrimitiveValue::from(time),
        ));
    }
    if let Some(dt) = inst.acquisition_dt {
        obj.put(DataElement::new(
            tags::ACQUISITION_DATE_TIME,
            VR::DT,
            PrimitiveValue::from(dt),
        ));
    }
    let file = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid(uids::CT_IMAGE_STORAGE)
                .media_storage_sop_instance_uid(inst.uid)
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN),
        )
        .expect("valid meta");
    file.write_to_file(dir.join(inst.name))
        .expect("write test file");
}

/// A fresh folder holding `instances`, a sub-folder and a non-DICOM file.
fn folder(test: &str, instances: &[Instance]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dcr-sort-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("notes.txt"), "not dicom").unwrap();
    for inst in instances {
        write_instance(&dir, inst);
    }
    dir
}

fn sample(test: &str) -> PathBuf {
    folder(
        test,
        &[
            Instance {
                name: "a.dcm",
                uid: "1.2.3.10",
                instance_number: Some(3),
                content: Some(("20240101", "120000.5")),
                acquisition_dt: Some("20240101115900"),
            },
            Instance {
                name: "b.dcm",
                uid: "1.2.3.9",
                instance_number: Some(10),
                content: Some(("20240101", "110000")),
                acquisition_dt: Some("20240101120100"),
            },
            Instance {
                name: "c.dcm",
                uid: "1.2.3.11",
                instance_number: Some(2),
                content: Some(("20231231", "235959")),
                acquisition_dt: None,
            },
            Instance {
                name: "d.dcm",
                uid: "1.2.3.1",
                instance_number: None,
                content: None,
                acquisition_dt: Some("20240101120000"),
            },
        ],
    )
}

fn explorer_in(dir: &Path) -> App {
    let mut app = App::new_explorer(None);
    app.layout.explorer.as_mut().unwrap().set_cwd(dir).unwrap();
    app
}

fn names(app: &App) -> Vec<String> {
    app.layout
        .explorer_rows()
        .iter()
        .map(|f| f.name.clone())
        .collect()
}

fn key(app: &mut App, code: KeyCode) {
    app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}

#[test]
fn filename_is_the_default_order() {
    let dir = sample("default");
    let app = explorer_in(&dir);
    assert!(!app.layout.sort.is_active());
    assert_eq!(
        names(&app),
        [
            "../",
            "sub/",
            "a.dcm",
            "b.dcm",
            "c.dcm",
            "d.dcm",
            "notes.txt"
        ]
    );
}

#[test]
fn sop_instance_uid_sorts_numerically_by_component() {
    let dir = sample("uid");
    let mut app = explorer_in(&dir);
    app.layout.set_sort(SortKey::SopInstanceUid);
    assert_eq!(
        names(&app),
        [
            "../",
            "sub/",
            "d.dcm",
            "b.dcm",
            "a.dcm",
            "c.dcm",
            "notes.txt"
        ],
        "folders stay on top, files without a value go last"
    );
}

#[test]
fn content_time_uses_date_then_time() {
    let dir = sample("content");
    let mut app = explorer_in(&dir);
    app.layout.set_sort(SortKey::ContentTime);
    assert_eq!(
        names(&app),
        [
            "../",
            "sub/",
            "c.dcm",
            "b.dcm",
            "a.dcm",
            "d.dcm",
            "notes.txt"
        ]
    );
}

#[test]
fn acquisition_time_falls_back_to_acquisition_datetime() {
    let dir = sample("acquisition");
    let mut app = explorer_in(&dir);
    app.layout.set_sort(SortKey::AcquisitionTime);
    assert_eq!(
        names(&app),
        [
            "../",
            "sub/",
            "a.dcm",
            "d.dcm",
            "b.dcm",
            "c.dcm",
            "notes.txt"
        ]
    );
}

#[test]
fn other_tag_sorts_numbers_by_value_then_by_name() {
    let dir = folder(
        "other",
        &[
            Instance {
                name: "x.dcm",
                uid: "1.1",
                instance_number: Some(10),
                content: None,
                acquisition_dt: None,
            },
            Instance {
                name: "b.dcm",
                uid: "1.2",
                instance_number: Some(2),
                content: None,
                acquisition_dt: None,
            },
            Instance {
                name: "a.dcm",
                uid: "1.3",
                instance_number: Some(2),
                content: None,
                acquisition_dt: None,
            },
        ],
    );
    let mut app = explorer_in(&dir);
    app.layout
        .set_sort(SortKey::parse_tag("InstanceNumber").unwrap());
    assert_eq!(
        names(&app),
        ["../", "sub/", "a.dcm", "b.dcm", "x.dcm", "notes.txt"]
    );
}

#[test]
fn navigation_follows_the_sorted_order_and_loads_files() {
    let dir = sample("nav");
    let mut app = explorer_in(&dir);
    app.layout.set_sort(SortKey::SopInstanceUid);
    key(&mut app, KeyCode::End);
    assert_eq!(app.layout.current_path(), Some(dir.join("notes.txt")));
    key(&mut app, KeyCode::Up);
    assert_eq!(app.layout.current_path(), Some(dir.join("c.dcm")));
    assert!(app.has_dicom_loaded());
    key(&mut app, KeyCode::Up);
    assert_eq!(app.layout.current_path(), Some(dir.join("a.dcm")));
}

#[test]
fn sort_stays_active_in_other_folders() {
    let dir = sample("persist");
    let mut app = explorer_in(&dir);
    app.layout.set_sort(SortKey::SopInstanceUid);
    key(&mut app, KeyCode::Down); // sub/
    key(&mut app, KeyCode::Enter);
    assert_eq!(names(&app), ["../"]);
    key(&mut app, KeyCode::Left);
    assert!(app.layout.sort.is_active());
    assert_eq!(names(&app)[2], "d.dcm");
}

#[test]
fn menu_picks_a_key_and_the_tag_prompt_validates() {
    let dir = sample("menu");
    let mut app = explorer_in(&dir);

    key(&mut app, KeyCode::Char('s'));
    assert_eq!(app.layout.sort_menu, Some(SortMenu::Pick(0)));
    key(&mut app, KeyCode::Char('3'));
    assert_eq!(app.layout.sort_menu, None);
    assert_eq!(app.layout.sort.key, SortKey::ContentTime);

    key(&mut app, KeyCode::Char('s'));
    assert_eq!(
        app.layout.sort_menu,
        Some(SortMenu::Pick(2)),
        "opens on the current key"
    );
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert!(matches!(app.layout.sort_menu, Some(SortMenu::Tag { .. })));
    for c in "Bogus".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.layout.sort_menu,
        Some(SortMenu::Tag {
            input: "Bogus".into(),
            error: true
        })
    );
    for _ in 0..5 {
        key(&mut app, KeyCode::Backspace);
    }
    for c in "0020,0013".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.layout.sort_menu, None);
    assert_eq!(app.layout.sort.key, SortKey::Tag(Tag(0x0020, 0x0013)));
    assert_eq!(app.layout.sort.key.label(), "InstanceNumber");

    key(&mut app, KeyCode::Char('s'));
    key(&mut app, KeyCode::Char('1'));
    assert!(!app.layout.sort.is_active());
    assert_eq!(names(&app)[2], "a.dcm");
}

#[test]
fn natural_order_compares_digit_runs_by_value() {
    assert_eq!(natural_cmp("1.2.9", "1.2.10"), Ordering::Less);
    assert_eq!(natural_cmp("1.2.010", "1.2.9"), Ordering::Greater);
    assert_eq!(natural_cmp("1.2", "1.2.1"), Ordering::Less);
    assert_eq!(natural_cmp("abc", "abd"), Ordering::Less);
}

#[test]
fn tag_parsing_accepts_keywords_and_numbers() {
    let instance_number = Some(SortKey::Tag(Tag(0x0020, 0x0013)));
    assert_eq!(SortKey::parse_tag("InstanceNumber"), instance_number);
    assert_eq!(SortKey::parse_tag(" (0020,0013) "), instance_number);
    assert_eq!(SortKey::parse_tag("0020,0013"), instance_number);
    assert_eq!(SortKey::parse_tag("00200013"), instance_number);
    assert_eq!(SortKey::parse_tag("NotATag"), None);
}

fn screen(app: &mut App) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 24)).unwrap();
    terminal.draw(|f| dcr::ui::render(f, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn active_sort_shows_a_badge_values_and_the_menu() {
    let dir = sample("render");
    let mut app = explorer_in(&dir);
    assert!(!screen(&mut app).contains('⇅'));

    app.layout.set_sort(SortKey::ContentTime);
    let out = screen(&mut app);
    assert!(out.contains("⇅ ContentTime"), "{out}");
    assert!(out.contains("2023-12-31 23:59:59"), "{out}");

    key(&mut app, KeyCode::Char('s'));
    let out = screen(&mut app);
    assert!(out.contains("Sort by"), "{out}");
    assert!(out.contains("● ContentTime"), "{out}");
}
