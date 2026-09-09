//! DICOMDIR (Media Storage Directory) parsing.
//!
//! A DICOMDIR stores its records as one flat `Directory Record Sequence` whose items are
//! linked to each other by **byte offsets** into the file (next sibling / first child).
//! This module rebuilds the patient → study → series → instance hierarchy from those
//! offsets, checks the referenced files against the disk and collects consistency issues.

use dicom_core::Tag;
use dicom_object::{FileDicomObject, InMemDicomObject};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const MEDIA_STORAGE_DIRECTORY_STORAGE: &str = "1.2.840.10008.1.3.10";

pub const FILE_SET_ID: Tag = Tag(0x0004, 0x1130);
pub const ROOT_FIRST_RECORD_OFFSET: Tag = Tag(0x0004, 0x1200);
pub const FILE_SET_CONSISTENCY_FLAG: Tag = Tag(0x0004, 0x1212);
pub const DIRECTORY_RECORD_SEQUENCE: Tag = Tag(0x0004, 0x1220);
pub const NEXT_RECORD_OFFSET: Tag = Tag(0x0004, 0x1400);
pub const RECORD_IN_USE_FLAG: Tag = Tag(0x0004, 0x1410);
pub const LOWER_LEVEL_OFFSET: Tag = Tag(0x0004, 0x1420);
pub const DIRECTORY_RECORD_TYPE: Tag = Tag(0x0004, 0x1430);
pub const REFERENCED_FILE_ID: Tag = Tag(0x0004, 0x1500);
pub const REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0004, 0x1510);
pub const REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0004, 0x1511);
pub const REFERENCED_TRANSFER_SYNTAX_UID: Tag = Tag(0x0004, 0x1512);

pub const PATIENT_NAME: Tag = Tag(0x0010, 0x0010);
pub const PATIENT_ID: Tag = Tag(0x0010, 0x0020);
pub const STUDY_DATE: Tag = Tag(0x0008, 0x0020);
pub const ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
pub const MODALITY: Tag = Tag(0x0008, 0x0060);
pub const STUDY_DESCRIPTION: Tag = Tag(0x0008, 0x1030);
pub const SERIES_DESCRIPTION: Tag = Tag(0x0008, 0x103E);
pub const SERIES_NUMBER: Tag = Tag(0x0020, 0x0011);
pub const INSTANCE_NUMBER: Tag = Tag(0x0020, 0x0013);

/// Upper bound on the number of directory entries walked when looking for files on disk
/// that the DICOMDIR does not reference. Protects against pointing dcr at a huge volume.
const MAX_DISK_WALK: usize = 50_000;

/// Returns true when `obj` (opened from `path`) is a DICOMDIR: either its media storage
/// SOP class says so, or the file is literally named `DICOMDIR`.
pub fn is_dicomdir(path: &Path, obj: &FileDicomObject<InMemDicomObject>) -> bool {
    if obj.meta().media_storage_sop_class_uid() == MEDIA_STORAGE_DIRECTORY_STORAGE {
        return true;
    }
    path.file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("DICOMDIR"))
        .unwrap_or(false)
}

/// Hierarchy rank of a record type: patient < study < series < everything else (leaves).
pub fn record_rank(record_type: &str) -> u8 {
    match record_type {
        "PATIENT" => 0,
        "STUDY" => 1,
        "SERIES" => 2,
        _ => 3,
    }
}

#[derive(Clone, Debug)]
pub struct DirRecord {
    /// Byte offset of the item in the file, when it could be determined.
    pub offset: Option<u64>,
    pub record_type: String,
    pub in_use: bool,
    pub next_offset: Option<u32>,
    pub lower_offset: Option<u32>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Number of leaf (file-referencing) records below this one, or 1 for a leaf itself.
    pub leaf_count: usize,
    /// Referenced File ID components (path relative to the DICOMDIR's folder).
    pub file_id: Option<Vec<String>>,
    /// Whether the referenced file exists on disk (None when the record references nothing).
    pub file_exists: Option<bool>,
    pub item: InMemDicomObject,
}

impl DirRecord {
    fn from_item(item: &InMemDicomObject) -> Self {
        let record_type = str_attr(item, DIRECTORY_RECORD_TYPE).unwrap_or_default();
        let in_use = int_attr(item, RECORD_IN_USE_FLAG)
            .map(|v| v != 0)
            .unwrap_or(true);
        let file_id = item
            .get(REFERENCED_FILE_ID)
            .and_then(|e| e.value().to_multi_str().ok())
            .map(|parts| {
                parts
                    .iter()
                    .map(|p| p.trim_end_matches(['\0', ' ']).to_string())
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|parts| !parts.is_empty());
        Self {
            offset: None,
            record_type,
            in_use,
            next_offset: int_attr(item, NEXT_RECORD_OFFSET).map(|v| v as u32),
            lower_offset: int_attr(item, LOWER_LEVEL_OFFSET).map(|v| v as u32),
            parent: None,
            children: Vec::new(),
            leaf_count: 0,
            file_id,
            file_exists: None,
            item: item.clone(),
        }
    }

    /// Trimmed string value of an attribute of the record, `None` if absent or empty.
    pub fn str(&self, tag: Tag) -> Option<String> {
        str_attr(&self.item, tag)
    }

    pub fn int(&self, tag: Tag) -> Option<i64> {
        int_attr(&self.item, tag)
    }

    pub fn rank(&self) -> u8 {
        record_rank(&self.record_type)
    }

    /// Relative path of the referenced file, using the platform separator.
    pub fn relative_path(&self) -> Option<PathBuf> {
        self.file_id.as_ref().map(|parts| parts.iter().collect())
    }

    /// Folder part of the referenced file id, joined with `/` (empty for root files).
    pub fn folder(&self) -> Option<String> {
        self.file_id
            .as_ref()
            .map(|parts| parts[..parts.len().saturating_sub(1)].join("/"))
    }

    pub fn file_name(&self) -> Option<&str> {
        self.file_id
            .as_ref()
            .and_then(|parts| parts.last())
            .map(|s| s.as_str())
    }

    /// Short, human key of the record used in labels: who/what it is in a few words.
    pub fn summary(&self) -> String {
        record_summary(&self.item).unwrap_or_default()
    }

    /// Text used for searching: type, every primitive attribute value and the file path.
    pub fn search_text(&self) -> String {
        let mut text = self.record_type.clone();
        for element in &self.item {
            if let Ok(v) = element.value().to_str() {
                text.push(' ');
                text.push_str(&v);
            }
        }
        if let Some(parts) = &self.file_id {
            text.push(' ');
            text.push_str(&parts.join("/"));
        }
        text
    }
}

/// Human label for a directory record item (used both by the directory view and to
/// annotate `Item #k` rows of the raw tag table). `None` if the item is not a record.
pub fn record_summary(item: &InMemDicomObject) -> Option<String> {
    let record_type = str_attr(item, DIRECTORY_RECORD_TYPE)?;
    let key = match record_type.as_str() {
        "PATIENT" => str_attr(item, PATIENT_NAME)
            .map(|n| n.replace('^', " "))
            .or_else(|| str_attr(item, PATIENT_ID)),
        "STUDY" => {
            let date = str_attr(item, STUDY_DATE).map(|d| format_da(&d));
            let desc = str_attr(item, STUDY_DESCRIPTION);
            match (date, desc) {
                (Some(d), Some(s)) => Some(format!("{d} · {s}")),
                (Some(d), None) => Some(d),
                (None, s) => s,
            }
        }
        "SERIES" => {
            let num = str_attr(item, SERIES_NUMBER).map(|n| format!("#{n}"));
            let modality = str_attr(item, MODALITY);
            let parts: Vec<String> = [num, modality].into_iter().flatten().collect();
            (!parts.is_empty()).then(|| parts.join(" "))
        }
        _ => {
            let num = str_attr(item, INSTANCE_NUMBER).map(|n| format!("#{n}"));
            let file = item
                .get(REFERENCED_FILE_ID)
                .and_then(|e| e.value().to_multi_str().ok())
                .and_then(|p| p.last().map(|s| s.trim().to_string()));
            let parts: Vec<String> = [num, file].into_iter().flatten().collect();
            (!parts.is_empty()).then(|| parts.join(" "))
        }
    };
    Some(match key {
        Some(k) => format!("{record_type} · {k}"),
        None => record_type,
    })
}

/// Formats a DICOM `DA` value (`YYYYMMDD`) as `YYYY-MM-DD`; other strings pass through.
pub fn format_da(da: &str) -> String {
    let d = da.trim();
    if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..])
    } else {
        d.to_string()
    }
}

fn str_attr(item: &InMemDicomObject, tag: Tag) -> Option<String> {
    let s = item.get(tag)?.value().to_str().ok()?;
    let s = s.trim_end_matches(['\0', ' ']).trim_start();
    (!s.is_empty()).then(|| s.to_string())
}

fn int_attr(item: &InMemDicomObject, tag: Tag) -> Option<i64> {
    let element = item.get(tag)?;
    element
        .value()
        .to_int::<i64>()
        .ok()
        .or_else(|| element.value().to_str().ok()?.trim().parse().ok())
}

#[derive(Clone, Debug, PartialEq)]
pub enum DirIssue {
    /// File-set Consistency Flag is non-zero.
    Inconsistent(u16),
    /// Records flagged as not in use (deleted but still present).
    InactiveRecords(usize),
    /// Referenced files that do not exist on disk.
    MissingFiles(usize),
    /// Files on disk that no record references.
    UnreferencedFiles(usize),
    /// Offsets pointing outside the record set (including the root offset).
    DanglingOffsets(usize),
    /// Records not reachable from the root through the offset links.
    UnreachableRecords(usize),
    /// A child record whose type does not fit under its parent (e.g. IMAGE under PATIENT).
    HierarchyViolations(usize),
    /// Referenced SOP Instance UIDs occurring more than once.
    DuplicateInstances(usize),
    /// Series whose instances use more than one transfer syntax.
    MixedTransferSyntax(usize),
    /// Item offsets could not be read; the tree was rebuilt from record order instead.
    OffsetsUnavailable,
}

impl std::fmt::Display for DirIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn plural(n: usize, one: &str, many: &str) -> String {
            if n == 1 {
                format!("{n} {one}")
            } else {
                format!("{n} {many}")
            }
        }
        match self {
            DirIssue::Inconsistent(flag) => write!(f, "consistency flag {flag}"),
            DirIssue::InactiveRecords(n) => {
                write!(f, "{}", plural(*n, "inactive record", "inactive records"))
            }
            DirIssue::MissingFiles(n) => {
                write!(f, "{}", plural(*n, "missing file", "missing files"))
            }
            DirIssue::UnreferencedFiles(n) => write!(
                f,
                "{}",
                plural(*n, "unreferenced file", "unreferenced files")
            ),
            DirIssue::DanglingOffsets(n) => {
                write!(f, "{}", plural(*n, "dangling offset", "dangling offsets"))
            }
            DirIssue::UnreachableRecords(n) => write!(
                f,
                "{}",
                plural(*n, "unreachable record", "unreachable records")
            ),
            DirIssue::HierarchyViolations(n) => write!(
                f,
                "{}",
                plural(*n, "hierarchy violation", "hierarchy violations")
            ),
            DirIssue::DuplicateInstances(n) => write!(
                f,
                "{}",
                plural(*n, "duplicate instance UID", "duplicate instance UIDs")
            ),
            DirIssue::MixedTransferSyntax(n) => write!(
                f,
                "{}",
                plural(
                    *n,
                    "series with mixed transfer syntax",
                    "series with mixed transfer syntaxes"
                )
            ),
            DirIssue::OffsetsUnavailable => {
                write!(f, "offsets unreadable, tree rebuilt from record order")
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DirStats {
    pub patients: usize,
    pub studies: usize,
    pub series: usize,
    pub leaves: usize,
    /// Leaf record types other than IMAGE, with counts (e.g. `SR DOCUMENT`).
    pub other_leaf_types: Vec<(String, usize)>,
    /// Modalities of series records with counts, most frequent first.
    pub modalities: Vec<(String, usize)>,
    pub in_use: usize,
    pub referenced_files: usize,
    pub files_found: usize,
}

#[derive(Clone, Debug)]
pub struct DicomDir {
    pub base_dir: PathBuf,
    pub file_set_id: Option<String>,
    pub consistency_flag: Option<u16>,
    /// All records in sequence order.
    pub records: Vec<DirRecord>,
    /// Indices of the top-level records, in link order.
    pub roots: Vec<usize>,
    /// True when the hierarchy was rebuilt from the byte offsets (the ground truth),
    /// false when it fell back to record order.
    pub linked_by_offsets: bool,
    pub issues: Vec<DirIssue>,
    /// Files under `base_dir` that no record references (relative paths, sorted).
    pub unreferenced: Vec<PathBuf>,
    pub stats: DirStats,
}

impl DicomDir {
    /// Parses `obj` (already opened from `path`) as a DICOMDIR.
    pub fn from_object(path: &Path, obj: &FileDicomObject<InMemDicomObject>) -> Self {
        let base_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let file_set_id = str_attr(obj, FILE_SET_ID);
        let consistency_flag = int_attr(obj, FILE_SET_CONSISTENCY_FLAG).map(|v| v as u16);
        let root_offset = int_attr(obj, ROOT_FIRST_RECORD_OFFSET).map(|v| v as u32);

        let mut records: Vec<DirRecord> = obj
            .get(DIRECTORY_RECORD_SEQUENCE)
            .and_then(|e| e.value().items())
            .map(|items| items.iter().map(DirRecord::from_item).collect())
            .unwrap_or_default();

        let mut issues = Vec::new();

        let offsets = scan_item_offsets(path)
            .ok()
            .filter(|o| o.len() == records.len());
        let (roots, linked_by_offsets) = match offsets {
            Some(offsets) => {
                for (record, offset) in records.iter_mut().zip(&offsets) {
                    record.offset = Some(*offset);
                }
                link_by_offsets(&mut records, root_offset, &mut issues)
            }
            None => {
                issues.push(DirIssue::OffsetsUnavailable);
                (link_by_order(&mut records), false)
            }
        };

        compute_leaf_counts(&mut records, &roots);
        check_files_on_disk(&mut records, &base_dir);
        let unreferenced = find_unreferenced(&records, &base_dir, path);
        let stats = compute_stats(&records);
        collect_issues(
            &records,
            &roots,
            &unreferenced,
            consistency_flag,
            &stats,
            &mut issues,
        );

        Self {
            base_dir,
            file_set_id,
            consistency_flag,
            records,
            roots,
            linked_by_offsets,
            issues,
            unreferenced,
            stats,
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.issues.is_empty()
    }

    /// Absolute path of the file referenced by record `idx`, if it references one.
    pub fn file_path(&self, idx: usize) -> Option<PathBuf> {
        self.records
            .get(idx)?
            .relative_path()
            .map(|rel| self.base_dir.join(rel))
    }

    /// Instance-number coverage of the children of `idx`: `(min, max, gaps, duplicates)`.
    /// `None` when no child carries an instance number.
    pub fn instance_range(&self, idx: usize) -> Option<InstanceRange> {
        let numbers: Vec<i64> = self.records[idx]
            .children
            .iter()
            .filter_map(|&c| self.records[c].int(INSTANCE_NUMBER))
            .collect();
        if numbers.is_empty() {
            return None;
        }
        let min = *numbers.iter().min().unwrap();
        let max = *numbers.iter().max().unwrap();
        let distinct: HashSet<i64> = numbers.iter().copied().collect();
        let span = (max - min + 1).max(0) as usize;
        Some(InstanceRange {
            min,
            max,
            gaps: span.saturating_sub(distinct.len()),
            duplicates: numbers.len() - distinct.len(),
        })
    }

    /// Distinct folders of the files referenced under `idx` (direct children only).
    pub fn child_folders(&self, idx: usize) -> Vec<String> {
        let mut folders: Vec<String> = self.records[idx]
            .children
            .iter()
            .filter_map(|&c| self.records[c].folder())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        folders.sort();
        folders
    }

    /// The folders referenced by the whole file-set, each with the leaf records it holds.
    pub fn folders(&self) -> Vec<DirFolder> {
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, r) in self.records.iter().enumerate() {
            if let Some(folder) = r.folder() {
                map.entry(folder).or_default().push(i);
            }
        }
        let mut folders: Vec<DirFolder> = map
            .into_iter()
            .map(|(name, leaves)| {
                let mut parents: Vec<usize> = leaves
                    .iter()
                    .filter_map(|&l| self.records[l].parent)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                parents.sort();
                DirFolder {
                    name,
                    file_count: leaves.len(),
                    parents,
                }
            })
            .collect();
        folders.sort_by(|a, b| a.name.cmp(&b.name));
        folders
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstanceRange {
    pub min: i64,
    pub max: i64,
    pub gaps: usize,
    pub duplicates: usize,
}

impl std::fmt::Display for InstanceRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.min == self.max {
            write!(f, "#{}", self.min)?;
        } else {
            write!(f, "{}–{}", self.min, self.max)?;
        }
        let mut notes = Vec::new();
        if self.gaps > 0 {
            notes.push(format!(
                "{} gap{}",
                self.gaps,
                if self.gaps == 1 { "" } else { "s" }
            ));
        }
        if self.duplicates > 0 {
            notes.push(format!("{} dup", self.duplicates));
        }
        if !notes.is_empty() {
            write!(f, " ({})", notes.join(", "))?;
        }
        Ok(())
    }
}

/// A folder referenced by the file-set.
#[derive(Clone, Debug)]
pub struct DirFolder {
    /// Relative folder path joined with `/`; empty for files next to the DICOMDIR.
    pub name: String,
    pub file_count: usize,
    /// Parent records (usually series) of the leaves stored in this folder.
    pub parents: Vec<usize>,
}

// ---------------------------------------------------------------------------------------
// Offsets
// ---------------------------------------------------------------------------------------

/// Counts the bytes pulled through it so item offsets can be recovered from the token
/// stream (the data set reader does not expose its position).
struct CountingReader<R> {
    inner: R,
    count: Rc<Cell<u64>>,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.set(self.count.get() + n as u64);
        Ok(n)
    }
}

/// Returns the byte offset (from the start of the file) of every item of the
/// Directory Record Sequence, in sequence order.
pub fn scan_item_offsets(path: &Path) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
    use dicom_encoding::TransferSyntaxIndex;
    use dicom_object::meta::FileMetaTable;
    use dicom_parser::dataset::read::DataSetReader;
    use dicom_parser::dataset::DataToken;
    use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

    let mut file = BufReader::new(std::fs::File::open(path)?);
    let mut head = [0u8; 132];
    let n = file.read(&mut head)?;
    if n >= 132 && &head[128..132] == b"DICM" {
        file.seek(SeekFrom::Start(128))?;
    } else if n >= 4 && &head[0..4] == b"DICM" {
        file.seek(SeekFrom::Start(0))?;
    } else {
        return Err("no DICM magic".into());
    }

    let meta = FileMetaTable::from_reader(&mut file)?;
    let base = file.stream_position()?;
    let ts = TransferSyntaxRegistry
        .get(meta.transfer_syntax())
        .ok_or("unsupported transfer syntax")?;

    let count = Rc::new(Cell::new(0u64));
    let reader = CountingReader {
        inner: file,
        count: count.clone(),
    };
    let tokens = DataSetReader::new_with_ts(reader, ts)?;

    let mut offsets = Vec::new();
    let mut depth = 0usize;
    let mut record_depth: Option<usize> = None;
    for token in tokens {
        match token? {
            DataToken::SequenceStart { tag, .. } => {
                if depth == 0 && tag == DIRECTORY_RECORD_SEQUENCE {
                    record_depth = Some(1);
                }
                depth += 1;
            }
            DataToken::PixelSequenceStart => depth += 1,
            DataToken::SequenceEnd => {
                depth = depth.saturating_sub(1);
                if record_depth.is_some() && depth == 0 {
                    break;
                }
            }
            DataToken::ItemStart { .. } if record_depth == Some(depth) => {
                // The item header (tag + length) is 8 bytes and has just been consumed.
                offsets.push(base + count.get() - 8);
            }
            _ => {}
        }
    }
    Ok(offsets)
}

fn link_by_offsets(
    records: &mut [DirRecord],
    root_offset: Option<u32>,
    issues: &mut Vec<DirIssue>,
) -> (Vec<usize>, bool) {
    let by_offset: HashMap<u64, usize> = records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.offset.map(|o| (o, i)))
        .collect();

    let resolve = |offset: Option<u32>| -> Option<usize> {
        offset
            .filter(|&o| o != 0)
            .and_then(|o| by_offset.get(&(o as u64)).copied())
    };

    let mut dangling = 0usize;
    let mut visited = vec![false; records.len()];
    let mut roots = Vec::new();

    // Iterative walk: (record index, parent) following lower-level then next links.
    let mut stack: Vec<(usize, Option<usize>)> = Vec::new();
    match root_offset {
        Some(o) if o != 0 => match resolve(Some(o)) {
            Some(idx) => stack.push((idx, None)),
            None => dangling += 1,
        },
        _ => {}
    }

    while let Some((idx, parent)) = stack.pop() {
        if visited[idx] {
            // A cycle or a record referenced twice: count it as a bad link and stop here.
            dangling += 1;
            continue;
        }
        visited[idx] = true;
        records[idx].parent = parent;
        match parent {
            Some(p) => records[p].children.push(idx),
            None => roots.push(idx),
        }

        // Push the sibling first so the child is processed (and ordered) before it.
        let next = records[idx].next_offset;
        if next.is_some_and(|o| o != 0) {
            match resolve(next) {
                Some(n) => stack.push((n, parent)),
                None => dangling += 1,
            }
        }
        let lower = records[idx].lower_offset;
        if lower.is_some_and(|o| o != 0) {
            match resolve(lower) {
                Some(c) => stack.push((c, Some(idx))),
                None => dangling += 1,
            }
        }
    }

    let unreachable: Vec<usize> = (0..records.len()).filter(|&i| !visited[i]).collect();
    if !unreachable.is_empty() {
        issues.push(DirIssue::UnreachableRecords(unreachable.len()));
        // Keep them visible as extra roots so nothing silently disappears.
        roots.extend(unreachable);
    }
    if dangling > 0 {
        issues.push(DirIssue::DanglingOffsets(dangling));
    }

    (roots, true)
}

/// Fallback when offsets are unavailable: records are assumed to be written depth-first,
/// so each record hangs under the closest preceding record of a lower rank.
fn link_by_order(records: &mut [DirRecord]) -> Vec<usize> {
    let mut roots = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for idx in 0..records.len() {
        let rank = records[idx].rank();
        while let Some(&top) = stack.last() {
            if records[top].rank() < rank {
                break;
            }
            stack.pop();
        }
        match stack.last().copied() {
            Some(parent) => {
                records[idx].parent = Some(parent);
                records[parent].children.push(idx);
            }
            None => roots.push(idx),
        }
        if rank < 3 {
            stack.push(idx);
        }
    }
    roots
}

fn compute_leaf_counts(records: &mut [DirRecord], roots: &[usize]) {
    fn visit(records: &mut [DirRecord], idx: usize) -> usize {
        let children = records[idx].children.clone();
        let count = if children.is_empty() {
            usize::from(records[idx].file_id.is_some())
        } else {
            children.iter().map(|&c| visit(records, c)).sum()
        };
        records[idx].leaf_count = count;
        count
    }
    for &root in roots {
        visit(records, root);
    }
}

fn check_files_on_disk(records: &mut [DirRecord], base_dir: &Path) {
    for record in records.iter_mut() {
        if let Some(rel) = record.relative_path() {
            record.file_exists = Some(base_dir.join(rel).is_file());
        }
    }
}

fn find_unreferenced(records: &[DirRecord], base_dir: &Path, dicomdir: &Path) -> Vec<PathBuf> {
    let referenced: HashSet<String> = records
        .iter()
        .filter_map(|r| r.file_id.as_ref())
        .map(|parts| normalize_rel(&parts.iter().collect::<PathBuf>()))
        .collect();

    let mut found = Vec::new();
    let mut pending = vec![base_dir.to_path_buf()];
    let mut walked = 0usize;
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            walked += 1;
            if walked > MAX_DISK_WALK {
                found.sort();
                return found;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                if path == dicomdir {
                    continue;
                }
                if let Ok(rel) = path.strip_prefix(base_dir) {
                    if !referenced.contains(&normalize_rel(rel)) {
                        found.push(rel.to_path_buf());
                    }
                }
            }
        }
    }
    found.sort();
    found
}

/// Case-insensitive, separator-normalized key for comparing relative paths.
fn normalize_rel(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().to_uppercase())
        .collect::<Vec<_>>()
        .join("/")
}

fn compute_stats(records: &[DirRecord]) -> DirStats {
    let mut stats = DirStats::default();
    let mut modalities: HashMap<String, usize> = HashMap::new();
    let mut other_leaves: HashMap<String, usize> = HashMap::new();
    for r in records {
        if r.in_use {
            stats.in_use += 1;
        }
        match r.record_type.as_str() {
            "PATIENT" => stats.patients += 1,
            "STUDY" => stats.studies += 1,
            "SERIES" => {
                stats.series += 1;
                if let Some(m) = r.str(MODALITY) {
                    *modalities.entry(m).or_default() += 1;
                }
            }
            other => {
                stats.leaves += 1;
                if other != "IMAGE" {
                    *other_leaves.entry(other.to_string()).or_default() += 1;
                }
            }
        }
        if r.file_id.is_some() {
            stats.referenced_files += 1;
            if r.file_exists == Some(true) {
                stats.files_found += 1;
            }
        }
    }
    let mut modalities: Vec<(String, usize)> = modalities.into_iter().collect();
    modalities.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    stats.modalities = modalities;
    let mut other_leaves: Vec<(String, usize)> = other_leaves.into_iter().collect();
    other_leaves.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    stats.other_leaf_types = other_leaves;
    stats
}

fn collect_issues(
    records: &[DirRecord],
    roots: &[usize],
    unreferenced: &[PathBuf],
    consistency_flag: Option<u16>,
    stats: &DirStats,
    issues: &mut Vec<DirIssue>,
) {
    if let Some(flag) = consistency_flag.filter(|&f| f != 0) {
        issues.push(DirIssue::Inconsistent(flag));
    }
    let inactive = records.len() - stats.in_use;
    if inactive > 0 {
        issues.push(DirIssue::InactiveRecords(inactive));
    }
    let missing = stats.referenced_files - stats.files_found;
    if missing > 0 {
        issues.push(DirIssue::MissingFiles(missing));
    }
    if !unreferenced.is_empty() {
        issues.push(DirIssue::UnreferencedFiles(unreferenced.len()));
    }

    // Hierarchy: a child must rank strictly deeper than its parent; PATIENT only at root;
    // SERIES and leaves are not expected directly at the root of a patient-rooted set.
    let mut violations = 0usize;
    for (i, r) in records.iter().enumerate() {
        match r.parent {
            Some(p) => {
                if r.rank() <= records[p].rank() {
                    violations += 1;
                }
            }
            None => {
                if roots.contains(&i)
                    && matches!(r.record_type.as_str(), "STUDY" | "SERIES" | "IMAGE")
                {
                    violations += 1;
                }
            }
        }
    }
    if violations > 0 {
        issues.push(DirIssue::HierarchyViolations(violations));
    }

    let mut seen: HashMap<String, usize> = HashMap::new();
    for r in records {
        if let Some(uid) = r.str(REFERENCED_SOP_INSTANCE_UID) {
            *seen.entry(uid).or_default() += 1;
        }
    }
    let duplicates = seen.values().filter(|&&n| n > 1).count();
    if duplicates > 0 {
        issues.push(DirIssue::DuplicateInstances(duplicates));
    }

    let mixed = records
        .iter()
        .filter(|r| r.record_type == "SERIES")
        .filter(|r| {
            let syntaxes: HashSet<String> = r
                .children
                .iter()
                .filter_map(|&c| records[c].str(REFERENCED_TRANSFER_SYNTAX_UID))
                .collect();
            syntaxes.len() > 1
        })
        .count();
    if mixed > 0 {
        issues.push(DirIssue::MixedTransferSyntax(mixed));
    }
}

// ---------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------

/// Short human name of a storage SOP class UID; falls back to the UID's tail.
pub fn sop_class_name(uid: &str) -> String {
    let uid = uid.trim();
    let name = match uid {
        "1.2.840.10008.5.1.4.1.1.1" => "CR Image",
        "1.2.840.10008.5.1.4.1.1.1.1" => "DX Image",
        "1.2.840.10008.5.1.4.1.1.1.2" => "MG Image",
        "1.2.840.10008.5.1.4.1.1.2" => "CT Image",
        "1.2.840.10008.5.1.4.1.1.2.1" => "Enhanced CT",
        "1.2.840.10008.5.1.4.1.1.4" => "MR Image",
        "1.2.840.10008.5.1.4.1.1.4.1" => "Enhanced MR",
        "1.2.840.10008.5.1.4.1.1.6.1" => "US Image",
        "1.2.840.10008.5.1.4.1.1.7" => "Secondary Capture",
        "1.2.840.10008.5.1.4.1.1.11.1" => "GSPS",
        "1.2.840.10008.5.1.4.1.1.12.1" => "XA Image",
        "1.2.840.10008.5.1.4.1.1.12.2" => "RF Image",
        "1.2.840.10008.5.1.4.1.1.20" => "NM Image",
        "1.2.840.10008.5.1.4.1.1.66" => "Raw Data",
        "1.2.840.10008.5.1.4.1.1.66.4" => "Segmentation",
        "1.2.840.10008.5.1.4.1.1.88.11" => "Basic Text SR",
        "1.2.840.10008.5.1.4.1.1.88.22" => "Enhanced SR",
        "1.2.840.10008.5.1.4.1.1.88.33" => "Comprehensive SR",
        "1.2.840.10008.5.1.4.1.1.88.59" => "Key Object",
        "1.2.840.10008.5.1.4.1.1.104.1" => "Encapsulated PDF",
        "1.2.840.10008.5.1.4.1.1.128" => "PET Image",
        "1.2.840.10008.5.1.4.1.1.130" => "Enhanced PET",
        "1.2.840.10008.5.1.4.1.1.481.1" => "RT Image",
        "1.2.840.10008.5.1.4.1.1.481.2" => "RT Dose",
        "1.2.840.10008.5.1.4.1.1.481.3" => "RT Structure Set",
        "1.2.840.10008.5.1.4.1.1.481.5" => "RT Plan",
        _ => "",
    };
    if !name.is_empty() {
        return name.to_string();
    }
    match uid.strip_prefix("1.2.840.10008.5.1.4.1.1.") {
        Some(tail) => format!("SOP …{tail}"),
        None => uid.to_string(),
    }
}

/// Short human name of a transfer syntax UID; falls back to the UID itself.
pub fn transfer_syntax_name(uid: &str) -> String {
    let uid = uid.trim();
    let name = match uid {
        "1.2.840.10008.1.2" => "Implicit LE",
        "1.2.840.10008.1.2.1" => "Explicit LE",
        "1.2.840.10008.1.2.1.99" => "Deflated LE",
        "1.2.840.10008.1.2.2" => "Explicit BE",
        "1.2.840.10008.1.2.4.50" => "JPEG Baseline",
        "1.2.840.10008.1.2.4.51" => "JPEG Extended",
        "1.2.840.10008.1.2.4.57" => "JPEG Lossless",
        "1.2.840.10008.1.2.4.70" => "JPEG Lossless SV1",
        "1.2.840.10008.1.2.4.80" => "JPEG-LS Lossless",
        "1.2.840.10008.1.2.4.81" => "JPEG-LS Lossy",
        "1.2.840.10008.1.2.4.90" => "JPEG 2000 Lossless",
        "1.2.840.10008.1.2.4.91" => "JPEG 2000",
        "1.2.840.10008.1.2.4.92" => "JPEG 2000 MC Lossless",
        "1.2.840.10008.1.2.4.93" => "JPEG 2000 MC",
        "1.2.840.10008.1.2.4.100" => "MPEG-2",
        "1.2.840.10008.1.2.4.201" => "HTJ2K Lossless",
        "1.2.840.10008.1.2.4.202" => "HTJ2K Lossless RPCL",
        "1.2.840.10008.1.2.4.203" => "HTJ2K",
        "1.2.840.10008.1.2.5" => "RLE Lossless",
        _ => "",
    };
    if name.is_empty() {
        uid.to_string()
    } else {
        name.to_string()
    }
}
