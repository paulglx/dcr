use crate::app::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

/// Render the UI
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Create the header row
    let header = Row::new(vec![
        Cell::from("Tag").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("VR").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Value").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    // Create data rows from DICOM tags
    let rows: Vec<Row> = app
        .tags
        .iter()
        .map(|tag| {
            let style = if tag.is_private() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            // Build indentation and tree indicator
            let indent = "  ".repeat(tag.depth);
            let expand_indicator = if tag.is_expandable {
                if tag.is_expanded { "▼ " } else { "▶ " }
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

    // Define column widths
    // Tag column needs extra space for indentation (2 chars per depth) + expand indicator (2 chars)
    let widths = [
        Constraint::Length(30),  // Tag: indentation + expand indicator + (GGGG,EEEE)
        Constraint::Length(36),  // Name: typical tag names
        Constraint::Length(4),   // VR: 2 chars + padding
        Constraint::Fill(1),     // Value: fill remaining space
    ];

    // Create the table widget
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
        )
        .highlight_symbol(">> ");

    // Render the table with state for selection
    frame.render_stateful_widget(table, area, &mut app.table_state);

    // Render help text or search input at the bottom
    render_help(frame, area, app);
}

/// Render help text or search input at the bottom of the screen
fn render_help(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::widgets::Paragraph;

    // Position at the bottom of the table area (inside the border)
    let help_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(1),
        width: area.width.saturating_sub(2),
        height: 1,
    };

    if app.search_mode {
        // Show search input with cursor
        let search_text = format!("/{}_", app.search_query);
        let search = Paragraph::new(search_text)
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(search, help_area);
    } else {
        // Show normal help text
        let help_text = " ↑/↓: Navigate | →: Expand | ←: Collapse | /: Search | q/Esc: Quit ";
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Cyan));
        frame.render_widget(help, help_area);
    }
}
