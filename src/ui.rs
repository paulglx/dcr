use crate::app::state::dirview::{DirView, Node, Row as DirRow};
use crate::app::state::display_file_name;
use crate::app::state::layout::explorer_scroll_offset;
use crate::app::state::sort::{self, SortKey, SortMenu};
use crate::app::App;
use crate::app::{AppMode, Focus};
use crate::dicom::dicomdir::{
    self, plural, sop_class_name, transfer_syntax_name, ACCESSION_NUMBER, INSTANCE_NUMBER,
    MODALITY, PATIENT_ID, PATIENT_NAME, REFERENCED_SOP_CLASS_UID, REFERENCED_TRANSFER_SYNTAX_UID,
    SERIES_DESCRIPTION, SERIES_NUMBER, STUDY_DATE, STUDY_DESCRIPTION,
};
use crate::dicom::{parse_dicom_datetime_delta_ms, DiffStatus};
use crate::textdiff::{word_diff, Change};
use crate::validation::ValidationResult;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub fn render(frame: &mut Frame, app: &mut App) {
    match app.layout.mode {
        AppMode::Direct => render_direct(frame, app),
        AppMode::Explorer => render_explorer(frame, app),
    }
}

fn render_direct(frame: &mut Frame, app: &mut App) {
    let full_area = frame.area();

    let (main_area, preview_area) = if app.preview.show {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(full_area);
        (h_chunks[0], Some(h_chunks[1]))
    } else {
        (full_area, None)
    };

    let validation_height = validation_pane_height(app);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(validation_height)])
        .split(main_area);

    let area = chunks[0];
    let validation_area = chunks[1];

    render_validation_pane(frame, validation_area, app);

    if let Some(preview_area) = preview_area {
        render_preview_pane(frame, preview_area, app);
    }

    render_content(frame, area, app, false);
    render_direct_help(frame, area, app);
}

fn render_explorer(frame: &mut Frame, app: &mut App) {
    let full_area = frame.area();
    let has_dicom = app.has_dicom_loaded();

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(full_area);

    let left_area = columns[0];
    let explorer_focused = app.layout.focus == Focus::Explorer;

    let (explorer_area, preview_area) = if has_dicom && app.preview.show {
        let preview_height = 14;
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(preview_height)])
            .split(left_area);
        (left_chunks[0], Some(left_chunks[1]))
    } else {
        (left_area, None)
    };

    app.layout.explorer_area = explorer_area;

    if let Some(ref explorer) = app.layout.explorer {
        let border_color = if explorer_focused {
            Color::Green
        } else {
            Color::DarkGray
        };
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(format!(" {} ", explorer.cwd().display()));
        if app.layout.sort.is_active() {
            block = block.title_top(
                Line::from(Span::styled(
                    format!(" ⇅ {} ", app.layout.sort.key.label()),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .right_aligned(),
            );
        }

        let inner = block.inner(explorer_area);
        frame.render_widget(block, explorer_area);
        render_explorer_rows(frame, inner, app);
    }

    if let Some(menu) = &app.layout.sort_menu {
        render_sort_menu(frame, explorer_area, menu, &app.layout.sort.key);
    }

    if has_dicom {
        let tags_area = columns[1];

        let validation_height = validation_pane_height(app);

        let v_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(validation_height)])
            .split(tags_area);

        render_content(frame, v_chunks[0], app, true);
        render_validation_pane(frame, v_chunks[1], app);

        if let Some(preview_area) = preview_area {
            render_preview_pane(frame, preview_area, app);
        }

        render_explorer_help(frame, v_chunks[0], app);
    } else {
        let empty_area = columns[1];
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" DICOM Viewer ");
        let paragraph = Paragraph::new("Select a DICOM file to view its tags")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(paragraph, empty_area);

        render_explorer_help(frame, explorer_area, app);
    }
}

/// Explorer entries in display order. While sorted, each file shows its sort value
/// (dimmed, right-aligned); diff marks show as an A / B badge at the right edge.
fn render_explorer_rows(frame: &mut Frame, area: Rect, app: &App) {
    let view_height = usize::from(area.height);
    if view_height == 0 || area.width == 0 {
        return;
    }
    let Some(explorer) = &app.layout.explorer else {
        return;
    };
    let rows = app.layout.explorer_rows();
    let selected = app.layout.explorer_row();
    let offset = explorer_scroll_offset(selected, view_height);
    let badge_width = if app.marks.any() { 4 } else { 0 };
    let len = explorer.files().len();

    for (i, file) in rows.iter().enumerate().skip(offset).take(view_height) {
        let row_area = Rect {
            y: area.y + (i - offset) as u16,
            height: 1,
            ..area
        };
        let mut style = Style::default().fg(if file.is_dir {
            Color::LightBlue
        } else {
            Color::White
        });
        if i == selected {
            style = style.bg(Color::DarkGray);
        }
        frame.render_widget(Paragraph::new(file.name.as_str()).style(style), row_area);

        // The value gets the room the name leaves, and at least half the row.
        let free = usize::from(area.width).saturating_sub(badge_width);
        let value_room = free
            .saturating_sub(file.name.chars().count() + 1)
            .max(free / 2);
        let index = app.layout.sort.index_at(i, len);
        if let Some(label) = app.layout.sort.label(index).filter(|_| value_room > 2) {
            let text = format!(" {}", fit_tail(label, value_room - 1));
            let width = text.chars().count() as u16;
            let value_area = Rect {
                x: area.x + area.width - badge_width as u16 - width,
                width,
                ..row_area
            };
            frame.render_widget(
                Paragraph::new(text).style(style.fg(Color::Gray).add_modifier(Modifier::DIM)),
                value_area,
            );
        }

        // Badge color follows the diffing direction: baseline red, modified green (§7).
        let (badge, color) = if app.marks.baseline.as_deref() == Some(file.path.as_path()) {
            (" A ", Color::Red)
        } else if app.marks.modified.as_deref() == Some(file.path.as_path()) {
            (" B ", Color::Green)
        } else {
            continue;
        };
        if area.width < 5 {
            continue;
        }
        let badge_area = Rect {
            x: area.x + area.width - 3,
            width: 3,
            ..row_area
        };
        let badge_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
        frame.render_widget(Paragraph::new(badge).style(badge_style), badge_area);
    }
}

/// Keeps the end of `s` within `width` columns, with a leading `…` when cut (the end of a
/// UID or a time is what tells files apart).
fn fit_tail(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count <= width {
        return s.to_string();
    }
    let tail: String = s.chars().skip(count + 1 - width.max(1)).collect();
    format!("…{tail}")
}

/// Sort picker drawn over the bottom of the explorer pane.
fn render_sort_menu(frame: &mut Frame, area: Rect, menu: &SortMenu, current: &SortKey) {
    let accent = Style::default().fg(Color::Yellow);
    let dim = Style::default().fg(Color::DarkGray);
    let mut lines: Vec<Line> = Vec::new();
    match menu {
        SortMenu::Pick(highlighted) => {
            for (row, label) in sort::MENU.iter().enumerate() {
                let marker = if row == current.menu_row() {
                    "● "
                } else {
                    "  "
                };
                let label = if row == sort::MENU_OTHER && matches!(current, SortKey::Tag(_)) {
                    format!("Other: {}", current.label())
                } else {
                    label.to_string()
                };
                let mut style = Style::default().fg(Color::White);
                if row == *highlighted {
                    style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
                }
                lines.push(Line::from(vec![
                    Span::styled(format!(" {} ", row + 1), dim),
                    Span::styled(marker, accent),
                    Span::styled(label, style),
                ]));
            }
            lines.push(Line::from(Span::styled(
                " ↑/↓ Enter: Sort | Esc: Close",
                dim,
            )));
        }
        SortMenu::Tag { input, error } => {
            lines.push(Line::from(Span::styled(
                " Sort by tag (then filename):",
                dim,
            )));
            lines.push(Line::from(vec![
                Span::styled(" > ", accent),
                Span::styled(format!("{input}_"), Style::default().fg(Color::White)),
            ]));
            lines.push(if *error {
                Line::from(Span::styled(
                    " Unknown tag",
                    Style::default().fg(Color::Red),
                ))
            } else {
                Line::from(Span::styled(" e.g. InstanceNumber or 0020,0013", dim))
            });
            lines.push(Line::from(Span::styled(" Enter: Sort | Esc: Back", dim)));
        }
    }

    let height = (lines.len() as u16 + 2).min(area.height);
    let width = area.width.saturating_sub(2).clamp(1, 40);
    let popup = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(height + 1),
        width,
        height,
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(accent)
        .title(" Sort by ");
    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

/// The validation pane is 3 rows tall, 4 when it has a list of problems to show.
fn validation_pane_height(app: &App) -> u16 {
    let has_details = match &app.dirview {
        Some(view) => !view.dir.issues.is_empty(),
        None => matches!(&app.meta.validation_result, ValidationResult::Invalid(_)),
    };
    if has_details {
        4
    } else {
        3
    }
}

/// Main content pane: the DICOMDIR tree when one is loaded and not toggled to raw,
/// otherwise the tag table.
fn render_content(frame: &mut Frame, area: Rect, app: &mut App, in_explorer: bool) {
    if app.dir_active() {
        render_dir_view(frame, area, app, in_explorer);
    } else {
        render_tag_table(frame, area, app, in_explorer);
    }
}

fn content_border_color(app: &App, in_explorer: bool) -> Color {
    if in_explorer && app.layout.focus != Focus::TagTable {
        Color::DarkGray
    } else {
        Color::Green
    }
}

fn render_dir_view(frame: &mut Frame, area: Rect, app: &mut App, in_explorer: bool) {
    let border_color = content_border_color(app, in_explorer);
    let Some(view) = app.dirview.as_mut() else {
        return;
    };

    let folder = display_file_name(&view.dir.base_dir);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(" DICOMDIR · {} ", folder));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(inner);

    frame.render_widget(Paragraph::new(dir_summary_lines(view)), chunks[0]);
    frame.render_widget(
        Paragraph::new(Line::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(Color::DarkGray),
        )),
        chunks[1],
    );

    let rows_area = chunks[2];
    let rows: Vec<Row> = view
        .visible
        .iter()
        .map(|row| Row::new(vec![Cell::from(dir_row_line(view, row))]))
        .collect();
    let table = Table::new(rows, [Constraint::Fill(1)]).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    view.area = area;
    view.rows_area = rows_area;
    frame.render_stateful_widget(table, rows_area, &mut view.table_state);
}

/// Two-line strip above the tree: what the file-set holds, and how healthy it is.
fn dir_summary_lines(view: &DirView) -> Vec<Line<'static>> {
    let dir = &view.dir;
    let stats = &dir.stats;
    let dim = Style::default().fg(Color::DarkGray);
    let bold = Style::default().add_modifier(Modifier::BOLD);

    let mut counts = vec![
        plural(stats.patients, "patient", "patients"),
        plural(stats.studies, "study", "studies"),
        format!("{} series", stats.series),
    ];
    let images = stats.leaves - stats.other_leaf_types.iter().map(|(_, n)| n).sum::<usize>();
    counts.push(plural(images, "image", "images"));
    for (kind, n) in &stats.other_leaf_types {
        counts.push(format!("{n} {}", kind.to_lowercase()));
    }
    let mut first = vec![Span::styled(format!(" {}", counts.join(" · ")), bold)];
    if !stats.modalities.is_empty() {
        first.push(Span::raw("      "));
        for (i, (modality, n)) in stats.modalities.iter().enumerate() {
            if i > 0 {
                first.push(Span::raw("  "));
            }
            first.push(Span::styled(
                format!("{modality} ×{n}"),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    let mut second = vec![
        Span::styled(" File-set ID: ", dim),
        Span::raw(dir.file_set_id.clone().unwrap_or_else(|| "(none)".into())),
        Span::styled("   consistency: ", dim),
    ];
    match dir.consistency_flag {
        Some(0) => second.push(Span::raw("0")),
        Some(flag) => second.push(Span::styled(
            flag.to_string(),
            Style::default().fg(Color::Red),
        )),
        None => second.push(Span::styled("(none)", dim)),
    }
    second.push(Span::styled("   in use: ", dim));
    let total = dir.records.len();
    let in_use_style = if stats.in_use < total {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    second.push(Span::styled(
        format!("{}/{}", stats.in_use, total),
        in_use_style,
    ));
    second.push(Span::styled("   on disk: ", dim));
    let disk_style = if stats.files_found < stats.referenced_files {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };
    second.push(Span::styled(
        format!("{}/{}", stats.files_found, stats.referenced_files),
        disk_style,
    ));
    if !dir.linked_by_offsets {
        second.push(Span::styled(
            "   (tree from record order)",
            Style::default().fg(Color::Yellow),
        ));
    }

    vec![Line::from(first), Line::from(second)]
}

/// Pads `s` to `width` columns, truncating with `…` when longer.
fn fit(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count > width {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    } else {
        format!("{s:<width$}")
    }
}

/// Indentation plus the ▼/▶ fold indicator of a tree row.
fn tree_prefix(depth: usize, expandable: bool, expanded: bool) -> String {
    let indicator = match (expandable, expanded) {
        (false, _) => "  ",
        (true, false) => "▶ ",
        (true, true) => "▼ ",
    };
    format!("{}{indicator}", "  ".repeat(depth))
}

fn dir_row_line(view: &DirView, row: &DirRow) -> Line<'static> {
    let prefix = tree_prefix(row.depth, row.expandable, row.expanded);
    let dim = Style::default().fg(Color::DarkGray);

    match &row.node {
        Node::Record(idx) => record_line(view, *idx, prefix),
        Node::Layout => {
            let mut spans = vec![
                Span::raw(prefix),
                Span::styled(
                    "LAYOUT  ",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "{} · {} referenced",
                    plural(view.folders.len(), "folder", "folders"),
                    plural(view.dir.stats.referenced_files, "file", "files"),
                )),
            ];
            let unreferenced = view.dir.unreferenced.len();
            spans.push(Span::raw(" · "));
            spans.push(Span::styled(
                format!("{unreferenced} unreferenced on disk"),
                if unreferenced > 0 {
                    Style::default().fg(Color::Red)
                } else {
                    dim
                },
            ));
            Line::from(spans)
        }
        Node::Folder(i) => {
            let folder = &view.folders[*i];
            let name = if folder.name.is_empty() {
                "(root)".to_string()
            } else {
                format!("{}/", folder.name)
            };
            let parents: Vec<String> = folder
                .parents
                .iter()
                .map(|&p| view.dir.records[p].summary().replace(" · ", " "))
                .collect();
            Line::from(vec![
                Span::raw(prefix),
                Span::styled(fit(&name, 28), Style::default().fg(Color::Blue)),
                Span::raw(format!(" {:>6} files   ", folder.file_count)),
                Span::styled(format!("← {}", parents.join(", ")), dim),
            ])
        }
        Node::Unreferenced => Line::from(vec![
            Span::raw(prefix),
            Span::styled(
                format!(
                    "unreferenced on disk: {}",
                    plural(view.dir.unreferenced.len(), "file", "files")
                ),
                Style::default().fg(Color::Red),
            ),
        ]),
        Node::UnreferencedFile(i) => Line::from(vec![
            Span::raw(prefix),
            Span::styled(view.dir.unreferenced[*i].display().to_string(), dim),
        ]),
    }
}

fn record_line(view: &DirView, idx: usize, prefix: String) -> Line<'static> {
    let dir = &view.dir;
    let record = &dir.records[idx];
    let inactive = !record.in_use;
    let dim = Style::default().fg(Color::DarkGray);
    let plain = if inactive { dim } else { Style::default() };
    let type_style = if inactive {
        dim
    } else {
        match record.record_type.as_str() {
            "PATIENT" => Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            "STUDY" => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            "SERIES" => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            _ => Style::default(),
        }
    };

    let mut spans = vec![
        Span::raw(prefix),
        Span::styled(format!("{:<8}", record.record_type), type_style),
    ];
    let none = |s: Option<String>, fallback: &str| -> (String, Style) {
        match s {
            Some(v) => (v, plain),
            None => (fallback.to_string(), dim),
        }
    };

    match record.record_type.as_str() {
        "PATIENT" => {
            let (name, style) = none(
                record.str(PATIENT_NAME).map(|n| n.replace('^', " ")),
                "(no name)",
            );
            spans.push(Span::styled(fit(&name, 28), style));
            spans.push(Span::styled("  ID ", dim));
            let (id, style) = none(record.str(PATIENT_ID), "-");
            spans.push(Span::styled(fit(&id, 16), style));
            spans.push(Span::styled(
                format!("  {}", plural(record.children.len(), "study", "studies")),
                dim,
            ));
        }
        "STUDY" => {
            let (date, style) = none(
                record.str(STUDY_DATE).map(|d| dicomdir::format_da(&d)),
                "(no date)",
            );
            spans.push(Span::styled(fit(&date, 10), style));
            spans.push(Span::raw("  "));
            let (desc, style) = none(record.str(STUDY_DESCRIPTION), "(no description)");
            spans.push(Span::styled(fit(&desc, 30), style));
            spans.push(Span::styled("  acc ", dim));
            let (acc, style) = none(record.str(ACCESSION_NUMBER), "-");
            spans.push(Span::styled(fit(&acc, 16), style));
            spans.push(Span::styled(
                format!(
                    "  {} series · {} img",
                    record.children.len(),
                    record.leaf_count
                ),
                dim,
            ));
        }
        "SERIES" => {
            let (num, style) = none(record.str(SERIES_NUMBER).map(|n| format!("#{n}")), "#-");
            spans.push(Span::styled(fit(&num, 6), style));
            let (modality, style) = none(record.str(MODALITY), "-");
            spans.push(Span::styled(
                fit(&modality, 4),
                if inactive {
                    dim
                } else {
                    style.fg(Color::Yellow)
                },
            ));
            let (desc, style) = none(record.str(SERIES_DESCRIPTION), "(no description)");
            spans.push(Span::styled(fit(&desc, 28), style));
            spans.push(Span::styled(
                fit(&format!("{} img", record.leaf_count), 10),
                plain,
            ));
            let range = dir
                .instance_range(idx)
                .map(|r| r.to_string())
                .unwrap_or_default();
            let range_style = if range.contains('(') {
                Style::default().fg(Color::Yellow)
            } else {
                plain
            };
            spans.push(Span::styled(fit(&range, 18), range_style));
            let folders = dir.child_folders(idx);
            match folders.len() {
                0 => {}
                1 => spans.push(Span::styled(
                    if folders[0].is_empty() {
                        "(root)".to_string()
                    } else {
                        format!("{}/", folders[0])
                    },
                    dim,
                )),
                n => spans.push(Span::styled(
                    format!("{n} folders"),
                    Style::default().fg(Color::Yellow),
                )),
            }
        }
        _ => {
            let (num, style) = none(record.str(INSTANCE_NUMBER).map(|n| format!("#{n}")), "");
            spans.push(Span::styled(fit(&num, 7), style));
            let (file, style) = none(record.file_name().map(str::to_string), "(no file)");
            spans.push(Span::styled(fit(&file, 22), style));
            let (sop, style) = none(
                record
                    .str(REFERENCED_SOP_CLASS_UID)
                    .map(|u| sop_class_name(&u)),
                "",
            );
            spans.push(Span::styled(fit(&sop, 20), style));
            let (ts, style) = none(
                record
                    .str(REFERENCED_TRANSFER_SYNTAX_UID)
                    .map(|u| transfer_syntax_name(&u)),
                "",
            );
            spans.push(Span::styled(fit(&ts, 20), style));
            match record.file_exists {
                Some(true) => spans.push(Span::styled("✓", Style::default().fg(Color::Green))),
                Some(false) => {
                    spans.push(Span::styled("✗ missing", Style::default().fg(Color::Red)))
                }
                None => {}
            }
        }
    }

    if inactive {
        spans.push(Span::styled("  (inactive)", dim));
    }
    Line::from(spans)
}

fn render_tag_table(frame: &mut Frame, area: Rect, app: &mut App, in_explorer: bool) {
    let diff_mode = app.meta.diff_mode;
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let mut header_cells = vec![];
    if diff_mode {
        header_cells.push(Cell::from(" "));
    }
    header_cells.extend(["  Tag", "Name", "VR", "Value"].map(Cell::from));
    let header = Row::new(header_cells).style(header_style).height(1);

    let rows: Vec<Row> = app
        .tags
        .visible
        .iter()
        .map(|tag| {
            let (marker, marker_style) = diff_marker(tag.diff_status.as_ref());
            let row_style = match tag.diff_status {
                Some(DiffStatus::Added) | Some(DiffStatus::Deleted) => marker_style,
                Some(_) => Style::default(),
                None if tag.is_private() => Style::default().fg(Color::DarkGray),
                None => Style::default(),
            };
            let value_cell = match (&tag.diff_status, &tag.baseline_value) {
                (Some(DiffStatus::Changed), Some(baseline)) => {
                    Cell::from(changed_value_line(baseline, &tag.value, &tag.vr))
                }
                (Some(DiffStatus::Changed), None) => {
                    Cell::from(tag.value.as_str()).style(Style::default().fg(Color::Blue))
                }
                _ => Cell::from(tag.value.as_str()).style(row_style),
            };

            let mut row_cells = vec![];
            if diff_mode {
                row_cells.push(Cell::from(marker).style(marker_style));
            }
            let tag_display = format!(
                "{}{}",
                tree_prefix(tag.depth, tag.is_expandable, tag.is_expanded),
                tag.tag
            );
            row_cells.extend([
                Cell::from(tag_display).style(row_style),
                Cell::from(tag.name.as_str()).style(row_style),
                Cell::from(tag.vr.as_str()).style(row_style),
                value_cell,
            ]);
            Row::new(row_cells)
        })
        .collect();

    let widths: Vec<Constraint> = if app.meta.diff_mode {
        vec![
            Constraint::Length(1),
            Constraint::Length(16),
            Constraint::Length(36),
            Constraint::Length(4),
            Constraint::Fill(1),
        ]
    } else {
        vec![
            Constraint::Length(16),
            Constraint::Length(36),
            Constraint::Length(4),
            Constraint::Fill(1),
        ]
    };

    let title = if app.meta.diff_mode {
        let changes = if app.tags.changes_only {
            " [changes only]"
        } else {
            ""
        };
        if let Some(ref modified_name) = app.meta.modified_name {
            format!(
                " DICOM Diff: {} ↔ {}{} ",
                app.meta.name, modified_name, changes
            )
        } else {
            format!(" DICOM Diff: {}{} ", app.meta.name, changes)
        }
    } else if app.dirview.is_some() {
        format!(" DICOMDIR (raw tags): {} ", app.meta.name)
    } else {
        format!(" DICOM Viewer: {} ", app.meta.name)
    };

    let border_color = content_border_color(app, in_explorer);

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(title),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    app.tags.area = area;
    frame.render_stateful_widget(table, area, &mut app.tags.table_state);
}

/// Marker column glyph and colour for a diff status (`+`, `-`, `M`, or blank).
fn diff_marker(status: Option<&DiffStatus>) -> (&'static str, Style) {
    match status {
        Some(DiffStatus::Added) => ("+", Style::default().fg(Color::Green)),
        Some(DiffStatus::Deleted) => ("-", Style::default().fg(Color::Red)),
        Some(DiffStatus::Changed) => ("M", Style::default().fg(Color::Blue)),
        Some(DiffStatus::Unchanged) | None => (" ", Style::default()),
    }
}

/// Word-level diff of a changed value, followed by the time delta for DA/TM/DT values.
fn changed_value_line(baseline: &str, modified: &str, vr: &str) -> Line<'static> {
    let mut line = render_inline_diff(baseline, modified);
    if let Some(delta_ms) = parse_dicom_datetime_delta_ms(vr, baseline, modified) {
        line.spans.push(Span::styled(
            format!(" ({delta_ms:+} ms)"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    line
}

fn render_inline_diff(baseline: &str, modified: &str) -> Line<'static> {
    let spans = word_diff(baseline, modified)
        .into_iter()
        .map(|(change, text)| {
            let style = match change {
                Change::Delete => Style::default()
                    .fg(Color::Red)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::CROSSED_OUT),
                Change::Insert => Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                Change::Equal => Style::default(),
            };
            Span::styled(text.to_string(), style)
        })
        .collect::<Vec<_>>();

    Line::from(spans)
}

/// Bottom row of the content pane: the search prompt, else a status message, else a
/// pending diff-mark hint, else `help`.
fn render_status_line(frame: &mut Frame, area: Rect, app: &App, help: &str) {
    let status_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(1),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let (text, color) = if app.search.active {
        (format!("/{}_", app.search.query), Color::Yellow)
    } else if let Some(message) = &app.marks.message {
        (format!(" {message} "), Color::Yellow)
    } else if let Some(hint) = (!app.meta.diff_mode)
        .then(|| pending_mark_hint(app))
        .flatten()
    {
        (hint, Color::Yellow)
    } else {
        (help.to_string(), Color::Cyan)
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(color)),
        status_area,
    );
}

fn render_direct_help(frame: &mut Frame, area: Rect, app: &App) {
    let help_text = if app.meta.diff_mode {
        " ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | c: Changes only | p: Preview | q/Esc: Quit "
    } else if app.dir_active() {
        " ↑/↓: Navigate | →/←: Expand/Collapse | Enter: Open file | t: Raw tags | /: Search | p: Preview | q/Esc: Quit "
    } else if app.dirview.is_some() {
        " ↑/↓: Navigate | →: Expand | ←: Collapse | t: Directory | /: Search | p: Preview | q/Esc: Quit "
    } else if app.dir_return.is_some() {
        " ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview | Esc: Back to DICOMDIR | q: Quit "
    } else {
        " ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview | q/Esc: Quit "
    };
    render_status_line(frame, area, app, help_text);
}

fn render_explorer_help(frame: &mut Frame, area: Rect, app: &App) {
    let help_text = match app.layout.focus {
        Focus::Explorer => {
            if app.meta.diff_mode {
                " ↑/↓: Navigate | a/b: Re-mark | x: Swap | c: Changes only | Tab: Tags | Esc: Exit diff | q: Quit "
            } else if app.has_dicom_loaded() {
                " ↑/↓: Navigate | →: Enter | ←: Back | Tab: Tags | a/b: Mark diff | s: Sort | p: Preview | q: Quit "
            } else {
                " ↑/↓: Navigate | →: Enter dir | ←: Parent dir | a/b: Mark diff | s: Sort | q: Quit "
            }
        }
        Focus::TagTable => {
            if app.meta.diff_mode {
                " Tab/Esc: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | c: Changes only | p: Preview "
            } else if app.dir_active() {
                " Tab: Explorer | ↑/↓: Navigate | →/←: Expand/Collapse | Enter: Open file | t: Raw tags | /: Search | p: Preview "
            } else if app.dirview.is_some() {
                " Tab/Esc: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | t: Directory | /: Search | p: Preview "
            } else if app.dir_return.is_some() {
                " Tab: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview | Esc: Back to DICOMDIR "
            } else {
                " Tab/Esc: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview "
            }
        }
    };
    render_status_line(frame, area, app, help_text);
}

fn pending_mark_hint(app: &App) -> Option<String> {
    match (&app.marks.baseline, &app.marks.modified) {
        (Some(a), None) => Some(format!(
            " A: {} — press 'b' on another file to diff | Esc: Cancel ",
            display_file_name(a)
        )),
        (None, Some(b)) => Some(format!(
            " B: {} — press 'a' on another file to diff | Esc: Cancel ",
            display_file_name(b)
        )),
        _ => None,
    }
}

fn render_preview_pane(frame: &mut Frame, area: Rect, app: &mut App) {
    let title = if app.meta.diff_mode {
        Line::from(vec![
            Span::raw(" Preview — "),
            Span::styled("A only: red", Style::default().fg(Color::Red)),
            Span::raw(" · "),
            Span::styled("B only: green", Style::default().fg(Color::Green)),
            Span::raw(" "),
        ])
    } else {
        Line::from(" Preview ")
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(title);

    if let Some(ref error) = app.preview.error {
        let paragraph = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red))
            .block(block);
        frame.render_widget(paragraph, area);
    } else if app.preview.has_image() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        app.preview.render(inner, frame.buffer_mut());
    } else {
        let paragraph = Paragraph::new("Decoding...")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(paragraph, area);
    }
}

fn render_validation_pane(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(view) = &app.dirview {
        render_dicomdir_health(frame, area, view);
        return;
    }

    let (title, border_color) = match &app.meta.validation_result {
        ValidationResult::Valid => (" ✓ All required fields present ", Color::Blue),
        ValidationResult::Invalid(_) => (" ✗ Missing required fields ", Color::Red),
        ValidationResult::NotApplicable => (" Validation not applicable ", Color::DarkGray),
    };

    let sop = &app.meta.sop_class;
    let mut lines = vec![Line::from(format!(
        "SOP Class: {} ({})",
        sop.name(),
        sop.uid()
    ))];
    if let ValidationResult::Invalid(missing) = &app.meta.validation_result {
        lines.push(problem_line("Missing:   ", &missing.join(", ")));
    }
    render_status_pane(frame, area, title, border_color, lines);
}

/// A red `label: details` line listing the problems found.
fn problem_line(label: &'static str, details: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(label, Style::default().fg(Color::Red)),
        Span::styled(
            details.to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    ])
}

/// Bordered pane whose title and frame share one colour.
fn render_status_pane(
    frame: &mut Frame,
    area: Rect,
    title: impl Into<Line<'static>>,
    color: Color,
    lines: Vec<Line<'static>>,
) {
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color))
            .title(title)
            .title_style(Style::default().fg(color)),
    );
    frame.render_widget(paragraph, area);
}

/// Replaces the type-1 validation pane while a DICOMDIR is loaded: record and file
/// counts plus the list of consistency problems found.
fn render_dicomdir_health(frame: &mut Frame, area: Rect, view: &DirView) {
    let dir = &view.dir;
    let stats = &dir.stats;
    let (title, border_color) = if dir.is_healthy() {
        (" ✓ DICOMDIR consistent ".to_string(), Color::Blue)
    } else {
        (
            format!(
                " ✗ DICOMDIR: {} ",
                plural(dir.issues.len(), "issue", "issues")
            ),
            Color::Red,
        )
    };

    let mut lines = vec![Line::from(vec![Span::raw(format!(
        "Records: {} ({} in use) · Files: {}/{} on disk · Unreferenced: {} · Links: {}",
        dir.records.len(),
        stats.in_use,
        stats.files_found,
        stats.referenced_files,
        dir.unreferenced.len(),
        if dir.linked_by_offsets {
            "byte offsets"
        } else {
            "record order"
        },
    ))])];

    if !dir.issues.is_empty() {
        let issues: Vec<String> = dir.issues.iter().map(|i| i.to_string()).collect();
        lines.push(problem_line("Issues:    ", &issues.join(", ")));
    }
    render_status_pane(frame, area, title, border_color, lines);
}
