use crate::dicom::DicomTag;
use crate::validation::{SopClass, ValidationResult};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use std::io;

/// Application state
pub struct App {
    /// Visible tags to display (flattened based on expansion state)
    pub tags: Vec<DicomTag>,
    /// All unfiltered DICOM tags (hierarchical)
    pub all_tags: Vec<DicomTag>,
    /// Filtered top-level tags when search is active (hierarchical, preserves children)
    filtered_tags: Option<Vec<DicomTag>>,
    /// Table state for tracking selection/scroll position
    pub table_state: TableState,
    /// Whether the application should quit
    pub should_quit: bool,
    /// The file name being viewed (or baseline name in diff mode)
    pub file_name: String,
    /// Whether diff mode is active
    pub diff_mode: bool,
    /// Modified file name in diff mode
    pub modified_name: Option<String>,
    /// Whether search mode is active
    pub search_mode: bool,
    /// Current search query
    pub search_query: String,
    /// Type 1 field validation result
    pub validation_result: ValidationResult,
    /// SOP Class interpretation
    pub sop_class: SopClass,
    pub table_area: Rect,
}

impl App {
    #[allow(dead_code)]
    pub fn new(
        tags: Vec<DicomTag>,
        file_name: String,
        validation_result: ValidationResult,
        sop_class: SopClass,
    ) -> Self {
        Self::new_with_diff(tags, file_name, None, validation_result, sop_class, false)
    }

    pub fn new_with_diff(
        tags: Vec<DicomTag>,
        file_name: String,
        modified_name: Option<String>,
        validation_result: ValidationResult,
        sop_class: SopClass,
        diff_mode: bool,
    ) -> Self {
        let mut table_state = TableState::default();
        let visible_tags = Self::build_visible_tags_from(&tags);
        if !visible_tags.is_empty() {
            table_state.select(Some(0));
        }

        Self {
            tags: visible_tags,
            all_tags: tags,
            filtered_tags: None,
            table_state,
            should_quit: false,
            file_name,
            diff_mode,
            modified_name,
            search_mode: false,
            search_query: String::new(),
            validation_result,
            sop_class,
            table_area: Rect::default(),
        }
    }

    fn build_visible_tags_from(tags: &[DicomTag]) -> Vec<DicomTag> {
        let mut visible = Vec::new();
        Self::collect_visible_tags(tags, &mut visible);
        visible
    }

    fn collect_visible_tags(tags: &[DicomTag], visible: &mut Vec<DicomTag>) {
        for tag in tags {
            visible.push(tag.clone());
            if tag.is_expanded && !tag.children.is_empty() {
                Self::collect_visible_tags(&tag.children, visible);
            }
        }
    }

    fn rebuild_visible_tags(&mut self) {
        // Use filtered_tags if a search filter is active, otherwise use all_tags
        let source = self.filtered_tags.as_ref().unwrap_or(&self.all_tags);
        self.tags = Self::build_visible_tags_from(source);
    }

    /// Returns the hierarchical tag source for path operations
    fn active_tags(&self) -> &Vec<DicomTag> {
        self.filtered_tags.as_ref().unwrap_or(&self.all_tags)
    }

    /// Returns mutable hierarchical tag source for modifications
    fn active_tags_mut(&mut self) -> &mut Vec<DicomTag> {
        if self.filtered_tags.is_some() {
            self.filtered_tags.as_mut().unwrap()
        } else {
            &mut self.all_tags
        }
    }

    fn expand_selected(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if selected_idx < self.tags.len() {
                let selected_tag = &self.tags[selected_idx];
                if selected_tag.is_expandable && !selected_tag.is_expanded {
                    let path = self.build_path_to_tag(selected_idx);
                    Self::set_expanded_in_tree(self.active_tags_mut(), &path, true);
                    self.rebuild_visible_tags();
                }
            }
        }
    }

    fn collapse_parent(&mut self) {
        if let Some(selected_idx) = self.table_state.selected() {
            if selected_idx < self.tags.len() {
                let current_depth = self.tags[selected_idx].depth;

                if current_depth > 0 {
                    for i in (0..selected_idx).rev() {
                        if self.tags[i].depth < current_depth && self.tags[i].is_expanded {
                            let path = self.build_path_to_tag(i);
                            Self::set_expanded_in_tree(self.active_tags_mut(), &path, false);
                            self.rebuild_visible_tags();
                            self.table_state.select(Some(i));
                            break;
                        }
                    }
                }
            }
        }
    }

    fn build_path_to_tag(&self, visible_idx: usize) -> Vec<usize> {
        let mut path = Vec::new();
        let mut current_idx = 0;
        Self::find_path_to_index(self.active_tags(), visible_idx, &mut current_idx, &mut path);
        path
    }

    fn find_path_to_index(
        tags: &[DicomTag],
        target_idx: usize,
        current_idx: &mut usize,
        path: &mut Vec<usize>,
    ) -> bool {
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

    fn set_expanded_in_tree(tags: &mut [DicomTag], path: &[usize], expanded: bool) {
        if path.is_empty() {
            return;
        }

        let idx = path[0];
        if idx >= tags.len() {
            return;
        }

        if path.len() == 1 {
            tags[idx].is_expanded = expanded;
        } else {
            Self::set_expanded_in_tree(&mut tags[idx].children, &path[1..], expanded);
        }
    }

    pub fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if self.search_mode {
                        match key.code {
                            KeyCode::Esc => {
                                self.search_mode = false;
                                self.search_query.clear();
                                self.filtered_tags = None;
                                self.rebuild_visible_tags();
                                self.reset_selection();
                            }
                            KeyCode::Enter => {
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
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                if self.search_query == "" {
                                    self.should_quit = true;
                                } else {
                                    self.search_query.clear();
                                    self.filtered_tags = None;
                                    self.rebuild_visible_tags();
                                    self.reset_selection();
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                self.scroll_down(1);
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                self.scroll_up(1);
                            }
                            KeyCode::Char('/') => {
                                self.search_mode = true;
                            }
                            KeyCode::Right | KeyCode::Char('l') => {
                                self.expand_selected();
                            }
                            KeyCode::Left | KeyCode::Char('h') => {
                                self.collapse_parent();
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => self.scroll_down(3),
                    MouseEventKind::ScrollUp => self.scroll_up(3),
                    MouseEventKind::Down(MouseButton::Left) => {
                        let y = mouse.row;
                        if y > self.table_area.y + 1
                            && y < self.table_area.y + self.table_area.height
                        {
                            let row_in_viewport = (y - self.table_area.y - 2) as usize;
                            let tag_index = self.table_state.offset() + row_in_viewport;
                            if tag_index < self.tags.len() {
                                self.table_state.select(Some(tag_index));
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    }

    fn scroll_down(&mut self, amount: usize) {
        if self.tags.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0);
        let max_index = self.tags.len().saturating_sub(1);
        let new_index = (current + amount).min(max_index);
        self.table_state.select(Some(new_index));
    }

    fn scroll_up(&mut self, amount: usize) {
        if self.tags.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0);
        let new_index = current.saturating_sub(amount);
        self.table_state.select(Some(new_index));
    }

    fn filter_tags(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_tags = None;
            self.rebuild_visible_tags();
        } else {
            let query = self.search_query.to_lowercase();
            // Filter top-level tags but preserve their full hierarchy (children)
            let filtered: Vec<DicomTag> = self
                .all_tags
                .iter()
                .filter(|tag| {
                    tag.tag.to_lowercase().contains(&query)
                        || tag.name.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
            self.filtered_tags = Some(filtered);
            self.rebuild_visible_tags();
        }
        self.reset_selection();
    }

    fn reset_selection(&mut self) {
        if self.tags.is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state.select(Some(0));
        }
    }
}
