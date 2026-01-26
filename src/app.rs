use crate::dicom::DicomTag;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::TableState;
use std::io;

/// Application state
pub struct App {
    /// List of DICOM tags to display (filtered view)
    pub tags: Vec<DicomTag>,
    /// All unfiltered DICOM tags
    pub all_tags: Vec<DicomTag>,
    /// Table state for tracking selection/scroll position
    pub table_state: TableState,
    /// Whether the application should quit
    pub should_quit: bool,
    /// The file name being viewed
    pub file_name: String,
    /// Whether search mode is active
    pub search_mode: bool,
    /// Current search query
    pub search_query: String,
}

impl App {
    /// Create a new App instance with the given tags and file name
    pub fn new(tags: Vec<DicomTag>, file_name: String) -> Self {
        let mut table_state = TableState::default();
        if !tags.is_empty() {
            table_state.select(Some(0));
        }

        Self {
            tags: tags.clone(),
            all_tags: tags,
            table_state,
            should_quit: false,
            file_name,
            search_mode: false,
            search_query: String::new(),
        }
    }

    /// Handle keyboard input events
    pub fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if self.search_mode {
                        // Handle search mode input
                        match key.code {
                            KeyCode::Esc => {
                                // Cancel search and restore full list
                                self.search_mode = false;
                                self.search_query.clear();
                                self.tags = self.all_tags.clone();
                                self.reset_selection();
                            }
                            KeyCode::Enter => {
                                // Confirm search and keep filter active
                                self.search_mode = false;
                            }
                            KeyCode::Backspace => {
                                self.search_query.pop();
                                self.filter_tags();
                            }
                            KeyCode::Char(c) => {
                                self.search_query.push(c);
                                self.filter_tags();
                            }
                            _ => {}
                        }
                    } else {
                        // Normal mode
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
                            KeyCode::Char('/') => {
                                // Activate search mode
                                self.search_mode = true;
                                self.search_query.clear();
                            }
                            _ => {}
                        }
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

    /// Filter tags based on the current search query
    fn filter_tags(&mut self) {
        if self.search_query.is_empty() {
            self.tags = self.all_tags.clone();
        } else {
            let query = self.search_query.to_lowercase();
            self.tags = self
                .all_tags
                .iter()
                .filter(|tag| {
                    tag.tag.to_lowercase().contains(&query)
                        || tag.name.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
        }
        self.reset_selection();
    }

    /// Reset the table selection to the first item
    fn reset_selection(&mut self) {
        if self.tags.is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state.select(Some(0));
        }
    }
}
