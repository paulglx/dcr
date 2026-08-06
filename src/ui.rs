use crate::app::{AppMode, Focus};
use crate::app::App;
use crate::dicom::{parse_dicom_datetime_delta_ms, DiffStatus};
use crate::validation::{SopClass, ValidationResult};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};
use ratatui_image::StatefulImage;
use similar::{ChangeTag, TextDiff};

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

    let validation_height = if matches!(&app.meta.validation_result, ValidationResult::Invalid(_)) {
        4
    } else {
        3
    };

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

    render_tag_table(frame, area, app, false);
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
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(format!(" {} ", explorer.cwd().display()));

        let inner = block.inner(explorer_area);
        frame.render_widget(block, explorer_area);
        frame.render_widget_ref(explorer.widget(), inner);
    }

    render_explorer_marks(frame, explorer_area, app);

    if has_dicom {
        let tags_area = columns[1];

        let validation_height =
            if matches!(&app.meta.validation_result, ValidationResult::Invalid(_)) {
                4
            } else {
                3
            };

        let v_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(validation_height)])
            .split(tags_area);

        render_tag_table(frame, v_chunks[0], app, true);
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

fn render_explorer_marks(frame: &mut Frame, area: Rect, app: &App) {
    if !app.marks.any() {
        return;
    }
    let Some(ref explorer) = app.layout.explorer else {
        return;
    };

    let top = area.y + 1;
    let view_height = area.height.saturating_sub(2) as usize;
    if view_height == 0 || area.width < 5 {
        return;
    }

    // Mirror the scroll-offset computation used for mouse click resolution.
    let selected = explorer.selected_idx();
    let offset = if selected >= view_height {
        selected - view_height + 1
    } else {
        0
    };

    for (i, file) in explorer
        .files()
        .iter()
        .enumerate()
        .skip(offset)
        .take(view_height)
    {
        // Badge color follows the diffing direction: baseline red, modified green (§7).
        let (badge, color) = if app.marks.baseline.as_deref() == Some(file.path()) {
            (" A ", Color::Red)
        } else if app.marks.modified.as_deref() == Some(file.path()) {
            (" B ", Color::Green)
        } else {
            continue;
        };
        let badge_area = Rect {
            x: area.x + area.width - 4,
            y: top + (i - offset) as u16,
            width: 3,
            height: 1,
        };
        let badge_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
        frame.render_widget(Paragraph::new(badge).style(badge_style), badge_area);
    }
}

fn render_tag_table(frame: &mut Frame, area: Rect, app: &mut App, in_explorer: bool) {
    let mut header_cells = vec![];
    if app.meta.diff_mode {
        header_cells.push(
            Cell::from(" ").style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }
    header_cells.extend(vec![
        Cell::from("  Tag").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Name").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("VR").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Value").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = app
        .tags
        .visible
        .iter()
        .map(|tag| {
            let indent = "  ".repeat(tag.depth);
            let expand_indicator = if tag.is_expandable {
                if tag.is_expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            let tag_display = format!("{}{}{}", indent, expand_indicator, tag.tag);

            let (row_style, value_cell) = if let Some(diff_status) = &tag.diff_status {
                match diff_status {
                    DiffStatus::Deleted => (
                        Style::default().fg(Color::Red),
                        Cell::from(tag.value.as_str()).style(Style::default().fg(Color::Red)),
                    ),
                    DiffStatus::Added => (
                        Style::default().fg(Color::Green),
                        Cell::from(tag.value.as_str()).style(Style::default().fg(Color::Green)),
                    ),
                    DiffStatus::Changed => {
                        let value_cell = if let Some(ref baseline) = tag.baseline_value {
                            let mut line = render_inline_diff(baseline, &tag.value);
                            if let Some(delta_ms) =
                                parse_dicom_datetime_delta_ms(&tag.vr, baseline, &tag.value)
                            {
                                let suffix = if delta_ms >= 0 {
                                    format!(" (+{} ms)", delta_ms)
                                } else {
                                    format!(" ({} ms)", delta_ms)
                                };
                                line.spans.push(Span::styled(
                                    suffix,
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                            Cell::from(line)
                        } else {
                            Cell::from(tag.value.as_str()).style(Style::default().fg(Color::Blue))
                        };
                        (Style::default(), value_cell)
                    }
                    DiffStatus::Unchanged => (
                        Style::default(),
                        Cell::from(tag.value.as_str()).style(Style::default()),
                    ),
                }
            } else {
                let base_style = if tag.is_private() {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                };
                (base_style, Cell::from(tag.value.as_str()).style(base_style))
            };

            let mut row_cells = vec![];

            if app.meta.diff_mode {
                let (indicator, indicator_style) = if let Some(diff_status) = &tag.diff_status {
                    match diff_status {
                        DiffStatus::Added => ("+", Style::default().fg(Color::Green)),
                        DiffStatus::Deleted => ("-", Style::default().fg(Color::Red)),
                        DiffStatus::Changed => ("M", Style::default().fg(Color::Blue)),
                        DiffStatus::Unchanged => (" ", Style::default()),
                    }
                } else {
                    (" ", Style::default())
                };
                row_cells.push(Cell::from(indicator).style(indicator_style));
            }

            row_cells.extend(vec![
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
        if let Some(ref modified_name) = app.meta.modified_name {
            format!(" DICOM Diff: {} ↔ {} ", app.meta.name, modified_name)
        } else {
            format!(" DICOM Diff: {} ", app.meta.name)
        }
    } else {
        format!(" DICOM Viewer: {} ", app.meta.name)
    };

    let border_color = if in_explorer && app.layout.focus == Focus::TagTable {
        Color::Green
    } else if in_explorer {
        Color::DarkGray
    } else {
        Color::Green
    };

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

fn render_inline_diff(baseline: &str, modified: &str) -> Line<'static> {
    let diff = TextDiff::from_words(baseline, modified);
    let mut spans = Vec::new();

    for change in diff.iter_all_changes() {
        let text = change.value();
        let style = match change.tag() {
            ChangeTag::Delete => Style::default()
                .fg(Color::Red)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::CROSSED_OUT),
            ChangeTag::Insert => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            ChangeTag::Equal => Style::default(),
        };
        spans.push(Span::styled(text.to_string(), style));
    }

    Line::from(spans)
}

fn render_direct_help(frame: &mut Frame, area: Rect, app: &App) {
    let help_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(1),
        width: area.width.saturating_sub(2),
        height: 1,
    };

    if app.search.active {
        let search_text = format!("/{}_", app.search.query);
        let search = Paragraph::new(search_text).style(Style::default().fg(Color::Yellow));
        frame.render_widget(search, help_area);
    } else {
        let help_text = " ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview | q/Esc: Quit ";
        let help = Paragraph::new(help_text).style(Style::default().fg(Color::Cyan));
        frame.render_widget(help, help_area);
    }
}

fn render_explorer_help(frame: &mut Frame, area: Rect, app: &App) {
    let help_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(1),
        width: area.width.saturating_sub(2),
        height: 1,
    };

    if app.search.active {
        let search_text = format!("/{}_", app.search.query);
        let search = Paragraph::new(search_text).style(Style::default().fg(Color::Yellow));
        frame.render_widget(search, help_area);
        return;
    }

    if let Some(ref message) = app.marks.message {
        let message = Paragraph::new(format!(" {} ", message))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(message, help_area);
        return;
    }

    if !app.meta.diff_mode {
        if let Some(hint) = pending_mark_hint(app) {
            let hint = Paragraph::new(hint).style(Style::default().fg(Color::Yellow));
            frame.render_widget(hint, help_area);
            return;
        }
    }

    let help_text = match app.layout.focus {
        Focus::Explorer => {
            if app.meta.diff_mode {
                " ↑/↓: Navigate | a/b: Re-mark | x: Swap | Tab: Tags | Esc: Exit diff | q: Quit "
            } else if app.has_dicom_loaded() {
                " ↑/↓: Navigate | →: Enter | ←: Back | Tab: Tags | a/b: Mark diff | p: Preview | q: Quit "
            } else {
                " ↑/↓: Navigate | →: Enter dir | ←: Parent dir | a/b: Mark diff | q: Quit "
            }
        }
        Focus::TagTable => {
            " Tab/Esc: Explorer | ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | p: Preview "
        }
    };
    let help = Paragraph::new(help_text).style(Style::default().fg(Color::Cyan));
    frame.render_widget(help, help_area);
}

fn pending_mark_hint(app: &App) -> Option<String> {
    let file_name = |path: &std::path::Path| {
        path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string())
    };
    match (&app.marks.baseline, &app.marks.modified) {
        (Some(a), None) => Some(format!(
            " A: {} — press 'b' on another file to diff | Esc: Cancel ",
            file_name(a)
        )),
        (None, Some(b)) => Some(format!(
            " B: {} — press 'a' on another file to diff | Esc: Cancel ",
            file_name(b)
        )),
        _ => None,
    }
}

fn render_preview_pane(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Preview ");

    if let Some(ref error) = app.preview.error {
        let paragraph = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red))
            .block(block);
        frame.render_widget(paragraph, area);
    } else if let Some(ref mut protocol) = app.preview.image {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let image = StatefulImage::new(None);
        frame.render_stateful_widget(image, inner, protocol);
    } else {
        let paragraph = Paragraph::new("Decoding...")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(paragraph, area);
    }
}

fn render_validation_pane(frame: &mut Frame, area: Rect, app: &App) {
    let sop_class_text = match &app.meta.sop_class {
        SopClass::Ct => "CT Image Storage",
        SopClass::Mr => "MR Image Storage",
        SopClass::Other(_) => "Other",
        SopClass::Unknown => "Unknown",
    };

    let sop_class_uid = match &app.meta.sop_class {
        SopClass::Ct => "1.2.840.10008.5.1.4.1.1.2",
        SopClass::Mr => "1.2.840.10008.5.1.4.1.1.4",
        SopClass::Other(uid) => uid.as_str(),
        SopClass::Unknown => "N/A",
    };

    let (title, border_color) = match &app.meta.validation_result {
        ValidationResult::Valid => (" ✓ All required fields present ", Color::Blue),
        ValidationResult::Invalid(_) => (" ✗ Missing required fields ", Color::Red),
        ValidationResult::NotApplicable => (" Validation not applicable ", Color::DarkGray),
    };

    let mut lines = vec![Line::from(vec![Span::raw(format!(
        "SOP Class: {} ({})",
        sop_class_text, sop_class_uid
    ))])];

    if let ValidationResult::Invalid(missing) = &app.meta.validation_result {
        let missing_text = missing.join(", ");
        lines.push(Line::from(vec![
            Span::styled("Missing:   ", Style::default().fg(Color::Red)),
            Span::styled(
                missing_text,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title)
            .title_style(Style::default().fg(border_color)),
    );

    frame.render_widget(paragraph, area);
}
