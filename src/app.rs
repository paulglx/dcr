use crate::dicom::DicomTag;
use crate::validation::ValidationResult;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::TableState;
use std::io;

/// Application state
pub struct App {
    /// Visible tags to display (flattened based on expansion state)
    pub tags: Vec<DicomTag>,
    /// All unfiltered DICOM tags (hierarchical)
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
    /// Type 1 field validation result
    pub validation_result: ValidationResult,
}

impl App {
    /// Create a new App instance with the given tags, file name, and validation result
    pub fn new(tags: Vec<DicomTag>, file_name: String, validation_result: ValidationResult) -> Self {
        let mut table_state = TableState::default();
        let visible_tags = Self::build_visible_tags_from(&tags);
        if !visible_tags.is_empty() {
            table_state.select(Some(0));
        }

        Self {
            tags: visible_tags,
            all_tags: tags,
            table_state,
            should_quit: false,
            file_name,
            search_mode: false,
            search_query: String::new(),
            validation_result,
        }
    }

    /// Build the visible tags list from hierarchical tags based on expansion state
    fn build_visible_tags_from(tags: &[DicomTag]) -> Vec<DicomTag> {
        let mut visible = Vec::new();
        Self::collect_visible_tags(tags, &mut visible);
        visible
    }

    /// Recursively collect visible tags based on expansion state
    fn collect_visible_tags(tags: &[DicomTag], visible: &mut Vec<DicomTag>) {
        for tag in tags {
            visible.push(tag.clone());
            if tag.is_expanded && !tag.children.is_empty() {
                Self::collect_visible_tags(&tag.children, visible);
            }
        }
    }

    /// Rebuild the visible tags list from all_tags
    fn rebuild_visible_tags(&mut self) {
        self.tags = Self::build_visible_tags_from(&self.all_tags);
    }

    /// Expand the currently selected tag if it's a sequence
    fn expand_selected(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if selected_idx < self.tags.len() {
                let selected_tag = &self.tags[selected_idx];
                if selected_tag.is_expandable && !selected_tag.is_expanded {
                    let path = self.build_path_to_tag(selected_idx);
                    Self::set_expanded_in_tree(&mut self.all_tags, &path, true);
                    self.rebuild_visible_tags();
                }
            }
        }
    }

    /// Collapse the closest expanded parent of the current selection
    fn collapse_parent(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if selected_idx < self.tags.len() {
                let current_depth = self.tags[selected_idx].depth;
                
                if current_depth > 0 {
                    // Find the closest expanded parent (previous tag with lower depth)
                    for i in (0..selected_idx).rev() {
                        if self.tags[i].depth < current_depth && self.tags[i].is_expanded {
                            let path = self.build_path_to_tag(i);
                            Self::set_expanded_in_tree(&mut self.all_tags, &path, false);
                            self.rebuild_visible_tags();
                            // Move selection to the collapsed parent
                            self.table_state.select(Some(i));
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Build a path (list of indices) to the tag at the given visible index
    fn build_path_to_tag(&self, visible_idx: usize) -> Vec<usize> {
        let mut path = Vec::new();
        let mut current_idx = 0;
        Self::find_path_to_index(&self.all_tags, visible_idx, &mut current_idx, &mut path);
        path
    }

    /// Recursively find the path to a tag at the given visible index
    fn find_path_to_index(tags: &[DicomTag], target_idx: usize, current_idx: &mut usize, path: &mut Vec<usize>) -> bool {
        for (i, tag) in tags.iter().enumerate() {
            if *current_idx == target_idx {
                path.push(i);
                return true;
            }
            *current_idx += 1;

            if tag.is_expanded && !tag.children.is_empty() {
                path.push(i);
                if Self::find_path_to_index(&tag.children, target_idx, current_idx, path) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }

    /// Set a tag's expansion state in the tree using the path
    fn set_expanded_in_tree(tags: &mut [DicomTag], path: &[usize], expanded: bool) {
        if path.is_empty() {
            return;
        }

        let idx = path[0];
        if idx >= tags.len() {
            return;
        }

        if path.len() == 1 {
            // This is the target tag
            tags[idx].is_expanded = expanded;
        } else {
            // Recurse into children
            Self::set_expanded_in_tree(&mut tags[idx].children, &path[1..], expanded);
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
                            KeyCode::Right | KeyCode::Char('l') => {
                                // Expand sequence
                                self.expand_selected();
                            }
                            KeyCode::Left | KeyCode::Char('h') => {
                                // Collapse closest parent
                                self.collapse_parent();
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
            self.rebuild_visible_tags();
        } else {
            let query = self.search_query.to_lowercase();
            // Flatten all tags and filter
            let visible = Self::build_visible_tags_from(&self.all_tags);
            self.tags = visible
                .into_iter()
                .filter(|tag| {
                    tag.tag.to_lowercase().contains(&query)
                        || tag.name.to_lowercase().contains(&query)
                })
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
