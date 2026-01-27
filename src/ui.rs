use crate::app::App;
use crate::validation::{SopClass, ValidationResult};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub fn render(frame: &mut Frame, app: &mut App) {
    let full_area = frame.area();

    let validation_height = if matches!(&app.validation_result, ValidationResult::Invalid(_)) {
        4
    } else {
        3
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(validation_height),
        ])
        .split(full_area);

    let area = chunks[0];
    let validation_area = chunks[1];

    render_validation_pane(frame, validation_area, app);

    let header = Row::new(vec![
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
    ])
    .height(1);

    let rows: Vec<Row> = app
        .tags
        .iter()
        .map(|tag| {
            let style = if tag.is_private() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

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

            Row::new(vec![
                Cell::from(tag_display),
                Cell::from(tag.name.as_str()),
                Cell::from(tag.vr.as_str()),
                Cell::from(tag.value.as_str()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(16),
        Constraint::Length(36),
        Constraint::Length(4),
        Constraint::Fill(1),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" DICOM Viewer: {} ", app.file_name)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(table, area, &mut app.table_state);

    render_help(frame, area, app);
}

fn render_help(frame: &mut Frame, area: Rect, app: &App) {
    let help_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(1),
        width: area.width.saturating_sub(2),
        height: 1,
    };

    if app.search_mode {
        let search_text = format!("/{}_", app.search_query);
        let search = Paragraph::new(search_text).style(Style::default().fg(Color::Yellow));
        frame.render_widget(search, help_area);
    } else {
        let help_text = " ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | q/Esc: Quit ";
        let help = Paragraph::new(help_text).style(Style::default().fg(Color::Cyan));
        frame.render_widget(help, help_area);
    }
}

fn render_validation_pane(frame: &mut Frame, area: Rect, app: &App) {
    let sop_class_text = match &app.sop_class {
        SopClass::Ct => "CT Image Storage",
        SopClass::Mr => "MR Image Storage",
        SopClass::Other(_) => "Other",
        SopClass::Unknown => "Unknown",
    };

    let sop_class_uid = match &app.sop_class {
        SopClass::Ct => "1.2.840.10008.5.1.4.1.1.2",
        SopClass::Mr => "1.2.840.10008.5.1.4.1.1.4",
        SopClass::Other(uid) => uid.as_str(),
        SopClass::Unknown => "N/A",
    };

    let (title, border_color) = match &app.validation_result {
        ValidationResult::Valid => (" ✓ All required fields present ", Color::Blue),
        ValidationResult::Invalid(_) => (" ✗ Missing required fields ", Color::Red),
        ValidationResult::NotApplicable => (" Validation not applicable ", Color::DarkGray),
    };

    let mut lines = vec![Line::from(vec![Span::raw(format!(
        "SOP Class: {} ({})",
        sop_class_text, sop_class_uid
    ))])];

    if let ValidationResult::Invalid(missing) = &app.validation_result {
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
