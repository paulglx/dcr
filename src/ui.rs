use crate::app::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

/// Render the UI
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Create the header row
    let header = Row::new(vec![
        Cell::from("Tag").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("VR").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Value").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ])
    .height(1)
    .bottom_margin(1);

    // Create data rows from DICOM tags
    let rows: Vec<Row> = app
        .tags
        .iter()
        .map(|tag| {
            Row::new(vec![
                Cell::from(Text::from(tag.tag.clone())),
                Cell::from(Text::from(tag.vr.clone())),
                Cell::from(Text::from(tag.value.clone())),
            ])
        })
        .collect();

    // Define column widths
    let widths = [
        Constraint::Length(13),  // Tag: (GGGG,EEEE) = 11 chars + padding
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

    // Render help text at the bottom
    render_help(frame, area);
}

/// Render help text at the bottom of the screen
fn render_help(frame: &mut Frame, area: Rect) {
    use ratatui::widgets::Paragraph;

    let help_text = " ↑/↓/j/k: Navigate | q/Esc: Quit ";
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Cyan));

    // Position help at the bottom of the table area (inside the border)
    let help_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(1),
        width: area.width.saturating_sub(2),
        height: 1,
    };

    frame.render_widget(help, help_area);
}
