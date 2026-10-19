//! Display order of the explorer's entries. The explorer lists `../`, then folders, then
//! files, each by name; sorting reorders only the files, by a DICOM attribute read from
//! each file's header, with the file name breaking ties. Files without the attribute (or
//! that are not DICOM) go last.

use dicom_core::dictionary::DataDictionary;
use dicom_core::{Tag, VR};
use dicom_dictionary_std::{tags, StandardDataDictionary};
use dicom_object::{DefaultDicomObject, OpenFileOptions};
use ratatui_explorer::FileExplorer;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SortKey {
    #[default]
    FileName,
    SopInstanceUid,
    /// ContentDate + ContentTime.
    ContentTime,
    /// AcquisitionDate + AcquisitionTime, else AcquisitionDateTime.
    AcquisitionTime,
    /// Any attribute of the root data set.
    Tag(Tag),
}

/// Rows of the sort menu; the last one opens the tag prompt.
pub const MENU: [&str; 5] = [
    "Filename",
    "SOPInstanceUID",
    "ContentTime",
    "AcquisitionTime",
    "Other tag…",
];
pub const MENU_OTHER: usize = MENU.len() - 1;

impl SortKey {
    /// Key of a menu row, `None` for the tag prompt row.
    pub fn from_menu(row: usize) -> Option<Self> {
        match row {
            0 => Some(Self::FileName),
            1 => Some(Self::SopInstanceUid),
            2 => Some(Self::ContentTime),
            3 => Some(Self::AcquisitionTime),
            _ => None,
        }
    }

    /// Menu row showing this key as current.
    pub fn menu_row(&self) -> usize {
        match self {
            Self::FileName => 0,
            Self::SopInstanceUid => 1,
            Self::ContentTime => 2,
            Self::AcquisitionTime => 3,
            Self::Tag(_) => MENU_OTHER,
        }
    }

    /// Accepts a keyword (`InstanceNumber`), `(gggg,eeee)`, `gggg,eeee` or `ggggeeee`.
    pub fn parse_tag(input: &str) -> Option<Self> {
        StandardDataDictionary
            .parse_tag(input.trim())
            .map(Self::Tag)
    }

    pub fn label(&self) -> String {
        match self {
            Self::FileName => MENU[0].to_string(),
            Self::SopInstanceUid => MENU[1].to_string(),
            Self::ContentTime => MENU[2].to_string(),
            Self::AcquisitionTime => MENU[3].to_string(),
            Self::Tag(tag) => StandardDataDictionary
                .by_tag(*tag)
                .map(|e| e.alias.to_string())
                .unwrap_or_else(|| format!("({:04X},{:04X})", tag.group(), tag.element())),
        }
    }

    /// Text that brings this key back when typed in the tag prompt.
    pub fn prompt_text(&self) -> String {
        match self {
            Self::Tag(_) => self.label(),
            _ => String::new(),
        }
    }

    fn value(&self, obj: &DefaultDicomObject) -> Option<SortValue> {
        match self {
            Self::FileName => None,
            Self::SopInstanceUid => text(obj, tags::SOP_INSTANCE_UID).map(SortValue::Text),
            Self::ContentTime => date_time(obj, tags::CONTENT_DATE, tags::CONTENT_TIME),
            Self::AcquisitionTime => date_time(obj, tags::ACQUISITION_DATE, tags::ACQUISITION_TIME)
                .or_else(|| {
                    text(obj, tags::ACQUISITION_DATE_TIME).map(|dt| {
                        let (date, time) = dt.split_at_checked(8).unwrap_or((&dt, ""));
                        SortValue::DateTime(date.to_string(), time.to_string())
                    })
                }),
            Self::Tag(tag) => tag_value(obj, *tag),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SortValue {
    /// Numeric VRs, one number per value.
    Numbers(Vec<f64>),
    /// DICOM date (`YYYYMMDD`) and time (`HHMMSS.f`), compared as text.
    DateTime(String, String),
    /// Anything else, compared with digit runs as numbers (so UIDs order naturally).
    Text(String),
}

impl SortValue {
    /// Short rendering for the explorer row.
    fn label(&self) -> String {
        match self {
            Self::Numbers(n) => n.iter().map(f64::to_string).collect::<Vec<_>>().join("\\"),
            Self::DateTime(date, time) => {
                let date = match (date.get(..4), date.get(4..6), date.get(6..8)) {
                    (Some(y), Some(m), Some(d)) => format!("{y}-{m}-{d}"),
                    _ => date.clone(),
                };
                let time = match (time.get(..2), time.get(2..4), time.get(4..)) {
                    (Some(h), Some(m), Some(s)) if !s.is_empty() => format!("{h}:{m}:{s}"),
                    (Some(h), Some(m), _) => format!("{h}:{m}"),
                    _ => time.clone(),
                };
                format!("{date} {time}").trim().to_string()
            }
            Self::Text(s) => s.clone(),
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Numbers(_) => 0,
            Self::DateTime(..) => 1,
            Self::Text(_) => 2,
        }
    }

    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Numbers(a), Self::Numbers(b)) => {
                for (x, y) in a.iter().zip(b) {
                    match x.total_cmp(y) {
                        Ordering::Equal => {}
                        o => return o,
                    }
                }
                a.len().cmp(&b.len())
            }
            (Self::DateTime(da, ta), Self::DateTime(db, tb)) => (da, ta).cmp(&(db, tb)),
            (Self::Text(a), Self::Text(b)) => natural_cmp(a, b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

fn text(obj: &DefaultDicomObject, tag: Tag) -> Option<String> {
    let value = obj.element(tag).ok()?.to_str().ok()?;
    let first = value.split('\\').next().unwrap_or("").trim();
    (!first.is_empty()).then(|| first.to_string())
}

/// Legacy (ACR-NEMA) dates and times may carry `.` / `:` separators.
fn date_time(obj: &DefaultDicomObject, date: Tag, time: Tag) -> Option<SortValue> {
    let date = text(obj, date).map(|d| d.replace('.', ""));
    let time = text(obj, time).map(|t| t.replace(':', ""));
    if date.is_none() && time.is_none() {
        return None;
    }
    Some(SortValue::DateTime(
        date.unwrap_or_default(),
        time.unwrap_or_default(),
    ))
}

fn tag_value(obj: &DefaultDicomObject, tag: Tag) -> Option<SortValue> {
    let elem = obj.element(tag).ok()?;
    let numeric = matches!(
        elem.vr(),
        VR::IS | VR::DS | VR::US | VR::SS | VR::UL | VR::SL | VR::UV | VR::SV | VR::FL | VR::FD
    );
    if numeric {
        if let Ok(numbers) = elem.value().to_multi_float64() {
            if !numbers.is_empty() {
                return Some(SortValue::Numbers(numbers));
            }
        }
    }
    if matches!(elem.vr(), VR::DA | VR::TM | VR::DT) {
        let s = elem.to_str().ok()?.trim().to_string();
        return (!s.is_empty()).then(|| match elem.vr() {
            VR::DA => SortValue::DateTime(s, String::new()),
            VR::TM => SortValue::DateTime(String::new(), s),
            _ => {
                let (date, time) = s.split_at_checked(8).unwrap_or((&s, ""));
                SortValue::DateTime(date.to_string(), time.to_string())
            }
        });
    }
    let s = elem.to_str().ok()?.trim().to_string();
    (!s.is_empty()).then_some(SortValue::Text(s))
}

/// Compares `a` and `b` with runs of ASCII digits compared by value: `1.2.9` < `1.2.10`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let da = a.iter().take_while(|c| c.is_ascii_digit()).count();
                let db = b.iter().take_while(|c| c.is_ascii_digit()).count();
                let trim = |s: &[u8]| -> usize { s.iter().take_while(|&&c| c == b'0').count() };
                let (na, nb) = (&a[trim(&a[..da])..da], &b[trim(&b[..db])..db]);
                let o = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
                if o != Ordering::Equal {
                    return o;
                }
                a = &a[da..];
                b = &b[db..];
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(y);
                }
                a = &a[1..];
                b = &b[1..];
            }
        }
    }
}

/// Sort value of one file, as read from its header.
#[derive(Clone, Debug)]
struct Entry {
    value: Option<SortValue>,
    label: Option<String>,
}

fn read_entry(key: &SortKey, path: &Path) -> Entry {
    let value = OpenFileOptions::new()
        .read_until(tags::PIXEL_DATA)
        .open_file(path)
        .ok()
        .and_then(|obj| key.value(&obj));
    let label = value.as_ref().map(SortValue::label);
    Entry { value, label }
}

/// Explorer display order. Indices are into `FileExplorer::files()`; rows are positions on
/// screen.
#[derive(Default)]
pub struct ExplorerSort {
    pub key: SortKey,
    /// Explorer index of each row; empty while sorting by file name.
    order: Vec<usize>,
    /// Value shown beside each file, by explorer index.
    labels: Vec<Option<String>>,
    /// Folder and entry count `order` was built for.
    built_for: Option<(PathBuf, usize)>,
    /// Values already read with the current key, so revisiting a folder is instant.
    cache: HashMap<PathBuf, Entry>,
}

impl ExplorerSort {
    pub fn is_active(&self) -> bool {
        self.key != SortKey::FileName
    }

    /// Switches to `key` and re-reads every file (choosing a key again refreshes it).
    pub fn set_key(&mut self, key: SortKey, explorer: &FileExplorer) {
        self.key = key;
        self.cache.clear();
        self.built_for = None;
        self.sync(explorer);
    }

    /// Rebuilds the order if the explorer moved to another folder (or its listing changed).
    pub fn sync(&mut self, explorer: &FileExplorer) {
        let current = (explorer.cwd().clone(), explorer.files().len());
        if self.built_for.as_ref() == Some(&current) {
            return;
        }
        self.built_for = Some(current);
        self.order.clear();
        self.labels.clear();
        if !self.is_active() {
            return;
        }

        let files = explorer.files();
        let mut entries: Vec<Option<Entry>> = Vec::with_capacity(files.len());
        for file in files {
            if file.is_dir {
                entries.push(None);
                continue;
            }
            let entry = self
                .cache
                .entry(file.path.clone())
                .or_insert_with(|| read_entry(&self.key, &file.path));
            entries.push(Some(entry.clone()));
        }

        // Directories (and `../`) come first in the explorer and keep their place.
        let first_file = files.iter().position(|f| !f.is_dir).unwrap_or(files.len());
        let mut sorted: Vec<usize> = (first_file..files.len()).collect();
        sorted.sort_by(|&i, &j| {
            let a = entries[i].as_ref().and_then(|e| e.value.as_ref());
            let b = entries[j].as_ref().and_then(|e| e.value.as_ref());
            let by_value = match (a, b) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            by_value.then_with(|| files[i].name.cmp(&files[j].name))
        });
        self.order = (0..first_file).chain(sorted).collect();
        self.labels = entries
            .into_iter()
            .map(|e| e.and_then(|e| e.label))
            .collect();
    }

    /// True when `order` matches the explorer's listing; otherwise rows are explorer indices.
    fn ordered(&self, len: usize) -> bool {
        self.order.len() == len
    }

    /// Explorer index of the entry on `row`.
    pub fn index_at(&self, row: usize, len: usize) -> usize {
        if self.ordered(len) {
            self.order[row]
        } else {
            row
        }
    }

    /// Row showing the entry at explorer index `index`.
    pub fn row_of(&self, index: usize, len: usize) -> usize {
        if self.ordered(len) {
            self.order.iter().position(|&i| i == index).unwrap_or(index)
        } else {
            index
        }
    }

    /// Sort value shown beside the entry at explorer index `index`.
    pub fn label(&self, index: usize) -> Option<&str> {
        self.labels.get(index)?.as_deref()
    }
}

/// The sort picker opened with `s` from the explorer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortMenu {
    /// Choosing among `MENU`; the highlighted row.
    Pick(usize),
    /// Typing a tag for "Other"; `error` is set after an unknown tag was submitted.
    Tag { input: String, error: bool },
}
