use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::Position;
use ratatui_explorer::Input;

use super::state::layout::explorer_scroll_offset;
use super::{App, AppMode, DiffSide, Focus};

fn key_to_explorer_input(code: KeyCode) -> Input {
    match code {
        KeyCode::Char('j') | KeyCode::Down => Input::Down,
        KeyCode::Char('k') | KeyCode::Up => Input::Up,
        KeyCode::Char('h') | KeyCode::Left => Input::Left,
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => Input::Right,
        KeyCode::Home => Input::Home,
        KeyCode::End => Input::End,
        KeyCode::PageUp => Input::PageUp,
        KeyCode::PageDown => Input::PageDown,
        _ => Input::None,
    }
}

impl App {
    /// Applies one terminal event. Any preview change it causes is picked up by the next
    /// `request_pending_preview`.
    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                self.marks.message = None;
                match (&self.layout.mode, &self.layout.focus) {
                    (AppMode::Explorer, Focus::Explorer) => {
                        self.handle_explorer_focus_key(key.code)
                    }
                    _ => self.handle_content_key(key.code),
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollDown => self.nav_down(3),
                MouseEventKind::ScrollUp => self.nav_up(3),
                MouseEventKind::Down(MouseButton::Left) => {
                    self.handle_click(Position::new(mouse.column, mouse.row))
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_click(&mut self, at: Position) {
        if self.layout.mode == AppMode::Explorer {
            let content_area = if self.dir_active() {
                self.dirview.as_ref().map(|v| v.area).unwrap_or_default()
            } else {
                self.tags.area
            };
            if self.has_dicom_loaded() && content_area.contains(at) {
                self.layout.focus = Focus::TagTable;
            } else {
                self.handle_explorer_click(at);
            }
        }
        if self.layout.mode == AppMode::Direct || self.layout.focus == Focus::TagTable {
            // Rows start right under the pane's top border (plus the header in the tag table).
            let (rows_top, offset, in_rows) = if self.dir_active() {
                let (rows, offset) = self
                    .dirview
                    .as_ref()
                    .map(|v| (v.rows_area, v.table_state.offset()))
                    .unwrap_or_default();
                let in_rows = at.y >= rows.y && at.y < rows.y + rows.height;
                (rows.y, offset, in_rows)
            } else {
                let area = self.tags.area;
                let in_rows = at.y > area.y + 1 && at.y < area.y + area.height;
                (area.y + 2, self.tags.table_state.offset(), in_rows)
            };
            if in_rows {
                self.nav_select(offset + usize::from(at.y - rows_top));
            }
        }
    }

    fn clear_search(&mut self) {
        self.search.query.clear();
        self.apply_search();
    }

    /// Shared handling of the search prompt keys. Returns true if the key was consumed.
    fn handle_search_key(&mut self, code: KeyCode) -> bool {
        if !self.search.active {
            return false;
        }
        match code {
            KeyCode::Esc => {
                self.search.active = false;
                self.clear_search();
            }
            KeyCode::Enter => {
                self.search.active = false;
            }
            KeyCode::Backspace => {
                self.search.query.pop();
                self.apply_search();
            }
            KeyCode::Char(c) => {
                self.search.query.push(c);
                self.apply_search();
            }
            _ => {}
        }
        true
    }

    /// Keys for the content pane (tag table or directory tree), in direct mode or with the
    /// tag table focused in explorer mode. The two differ only in what `Esc` falls back to
    /// and in `Tab` handing focus back to the explorer.
    fn handle_content_key(&mut self, code: KeyCode) {
        if self.handle_search_key(code) {
            return;
        }
        let in_explorer = self.layout.mode == AppMode::Explorer;
        match code {
            KeyCode::Tab if in_explorer => self.layout.focus = Focus::Explorer,
            KeyCode::Esc => {
                if !self.search.query.is_empty() {
                    self.clear_search();
                } else if !self.return_to_dicomdir() {
                    if in_explorer {
                        self.layout.focus = Focus::Explorer;
                    } else {
                        self.should_quit = true;
                    }
                }
            }
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.nav_down(1),
            KeyCode::Up | KeyCode::Char('k') => self.nav_up(1),
            KeyCode::Char('/') => self.search.active = true,
            KeyCode::Char('p') => self.toggle_preview(),
            KeyCode::Char('t') => self.toggle_dir_raw(),
            KeyCode::Char('c') => self.toggle_changes_only(),
            KeyCode::Enter => self.open_selected_record(),
            KeyCode::Right | KeyCode::Char('l') => self.nav_expand(),
            KeyCode::Left | KeyCode::Char('h') => self.nav_collapse(),
            _ => {}
        }
    }

    fn handle_explorer_focus_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => {
                if self.meta.diff_mode {
                    self.exit_diff();
                } else if self.marks.any() {
                    self.marks.clear();
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('a') => self.mark_for_diff(DiffSide::Baseline),
            KeyCode::Char('b') => self.mark_for_diff(DiffSide::Modified),
            KeyCode::Char('x') => self.swap_diff_marks(),
            KeyCode::Char('c') => self.toggle_changes_only(),
            KeyCode::Tab => {
                if self.has_dicom_loaded() {
                    self.layout.focus = Focus::TagTable;
                }
            }
            KeyCode::Char('p') => self.toggle_preview(),
            _ => {
                if let Some(explorer) = &mut self.layout.explorer {
                    let _ = explorer.handle(key_to_explorer_input(code));
                }
                self.check_explorer_selection();
                // Enter on a file also moves into the tags it just opened.
                if code == KeyCode::Enter && !self.meta.diff_mode && self.has_dicom_loaded() {
                    self.layout.focus = Focus::TagTable;
                }
            }
        }
    }

    fn handle_explorer_click(&mut self, at: Position) {
        let area = self.layout.explorer_area;
        if !area.contains(at) {
            return;
        }
        self.layout.focus = Focus::Explorer;

        let top = area.y + 1;
        let view_height = usize::from(area.height.saturating_sub(2));
        if at.y < top || usize::from(at.y - top) >= view_height {
            return;
        }

        if let Some(explorer) = &mut self.layout.explorer {
            let selected = explorer.selected_idx();
            let clicked = explorer_scroll_offset(selected, view_height) + usize::from(at.y - top);
            if clicked >= explorer.files().len() {
                return;
            }
            if clicked == selected && explorer.current().is_dir {
                let _ = explorer.handle(Input::Right);
            } else {
                explorer.set_selected_idx(clicked);
            }
        }

        self.check_explorer_selection();
    }

    fn check_explorer_selection(&mut self) {
        if self.meta.diff_mode {
            return;
        }
        match self.layout.current_path() {
            Some(path) if path.is_file() => self.load_dicom_file(&path),
            _ => self.clear_dicom_state(),
        }
    }
}
