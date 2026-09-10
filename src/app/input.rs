use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui_explorer::Input;

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
                if self.layout.mode == AppMode::Direct {
                    self.handle_direct_key(key.code);
                } else {
                    self.handle_explorer_key(key.code);
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollDown => self.nav_down(3),
                MouseEventKind::ScrollUp => self.nav_up(3),
                MouseEventKind::Down(MouseButton::Left) => {
                    if self.layout.mode == AppMode::Explorer {
                        let hit = |area: Rect| {
                            mouse.column >= area.x
                                && mouse.column < area.x + area.width
                                && mouse.row >= area.y
                                && mouse.row < area.y + area.height
                        };
                        let content_area = if self.dir_active() {
                            self.dirview.as_ref().map(|v| v.area).unwrap_or_default()
                        } else {
                            self.tags.area
                        };
                        if self.has_dicom_loaded() && hit(content_area) {
                            self.layout.focus = Focus::TagTable;
                        } else {
                            self.handle_explorer_click(mouse.column, mouse.row);
                        }
                    }
                    if self.layout.mode == AppMode::Direct || self.layout.focus == Focus::TagTable {
                        let y = mouse.row;
                        if self.dir_active() {
                            let (rows, offset) = self
                                .dirview
                                .as_ref()
                                .map(|v| (v.rows_area, v.table_state.offset()))
                                .unwrap_or_default();
                            if y >= rows.y && y < rows.y + rows.height {
                                let index = offset + (y - rows.y) as usize;
                                self.nav_select(index);
                            }
                        } else if y > self.tags.area.y + 1
                            && y < self.tags.area.y + self.tags.area.height
                        {
                            let row_in_viewport = (y - self.tags.area.y - 2) as usize;
                            let tag_index = self.tags.table_state.offset() + row_in_viewport;
                            self.nav_select(tag_index);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
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

    fn handle_direct_key(&mut self, code: KeyCode) {
        self.marks.message = None;
        if self.handle_search_key(code) {
            return;
        }
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => {
                if !self.search.query.is_empty() {
                    self.clear_search();
                } else if !self.return_to_dicomdir() {
                    self.should_quit = true;
                }
            }
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

    fn handle_explorer_key(&mut self, code: KeyCode) {
        self.marks.message = None;
        match self.layout.focus {
            Focus::Explorer => self.handle_explorer_focus_key(code),
            Focus::TagTable => self.handle_tagtable_focus_key(code),
        }
    }

    fn handle_explorer_focus_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
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
            KeyCode::Enter => {
                let input = key_to_explorer_input(code);
                if let Some(ref mut explorer) = self.layout.explorer {
                    let _ = explorer.handle(input);
                }
                self.check_explorer_selection();
                if !self.meta.diff_mode && self.has_dicom_loaded() {
                    self.layout.focus = Focus::TagTable;
                }
            }
            _ => {
                let input = key_to_explorer_input(code);
                if let Some(ref mut explorer) = self.layout.explorer {
                    let _ = explorer.handle(input);
                }
                self.check_explorer_selection();
            }
        }
    }

    fn handle_tagtable_focus_key(&mut self, code: KeyCode) {
        if self.handle_search_key(code) {
            return;
        }

        match code {
            KeyCode::Tab => {
                self.layout.focus = Focus::Explorer;
            }
            KeyCode::Esc => {
                if !self.search.query.is_empty() {
                    self.clear_search();
                } else if !self.return_to_dicomdir() {
                    self.layout.focus = Focus::Explorer;
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

    fn handle_explorer_click(&mut self, column: u16, row: u16) {
        let area = self.layout.explorer_area;
        let inside = column >= area.x
            && column < area.x + area.width
            && row >= area.y
            && row < area.y + area.height;
        if !inside {
            return;
        }

        self.layout.focus = Focus::Explorer;

        let top = area.y + 1;
        let view_height = area.height.saturating_sub(2);
        if row < top || row >= top + view_height {
            return;
        }

        if let Some(ref mut explorer) = self.layout.explorer {
            let selected = explorer.selected_idx();
            let offset = if selected >= view_height as usize {
                selected - view_height as usize + 1
            } else {
                0
            };
            let clicked = offset + (row - top) as usize;
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

        let current_path = self
            .layout
            .explorer
            .as_ref()
            .map(|e| e.current().path.clone());

        match current_path {
            Some(path) if path.is_file() => {
                self.load_dicom_file(&path);
            }
            _ => {
                self.clear_dicom_state();
            }
        }
    }
}
