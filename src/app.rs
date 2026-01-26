use crate::dicom::DicomTag;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::TableState;
use std::io;

/// Application state
pub struct App {
    /// List of DICOM tags to display
    pub tags: Vec<DicomTag>,
    /// Table state for tracking selection/scroll position
    pub table_state: TableState,
    /// Whether the application should quit
    pub should_quit: bool,
    /// The file name being viewed
    pub file_name: String,
}

impl App {
    /// Create a new App instance with the given tags and file name
    pub fn new(tags: Vec<DicomTag>, file_name: String) -> Self {
        let mut table_state = TableState::default();
        if !tags.is_empty() {
            table_state.select(Some(0));
        }

        Self {
            tags,
            table_state,
            should_quit: false,
            file_name,
        }
    }

    /// Handle keyboard input events
    pub fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            self.should_quit = true;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            self.scroll_down(1);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            self.scroll_up(1);
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Scroll down by the given number of rows
    fn scroll_down(&mut self, amount: usize) {
        if self.tags.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0);
        let max_index = self.tags.len().saturating_sub(1);
        let new_index = (current + amount).min(max_index);
        self.table_state.select(Some(new_index));
    }

    /// Scroll up by the given number of rows
    fn scroll_up(&mut self, amount: usize) {
        if self.tags.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0);
        let new_index = current.saturating_sub(amount);
        self.table_state.select(Some(new_index));
    }

}
