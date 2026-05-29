use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui_explorer::Input;
use std::io;

use super::{App, AppMode, Focus};

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
    pub fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if self.layout.mode == AppMode::Direct {
                        self.handle_direct_key(key.code);
                    } else {
                        self.handle_explorer_key(key.code);
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => self.tags.scroll_down(3),
                    MouseEventKind::ScrollUp => self.tags.scroll_up(3),
                    MouseEventKind::Down(MouseButton::Left) => {
                        if self.layout.mode == AppMode::Explorer {
                            let hit = |area: Rect| {
                                mouse.column >= area.x
                                    && mouse.column < area.x + area.width
                                    && mouse.row >= area.y
                                    && mouse.row < area.y + area.height
                            };
                            if self.has_dicom_loaded() && hit(self.tags.area) {
                                self.layout.focus = Focus::TagTable;
                            } else {
                                self.handle_explorer_click(mouse.column, mouse.row);
                            }
                        }
                        if self.layout.mode == AppMode::Direct
                            || self.layout.focus == Focus::TagTable
                        {
                            let y = mouse.row;
                            if y > self.tags.area.y + 1
                                && y < self.tags.area.y + self.tags.area.height
                            {
                                let row_in_viewport = (y - self.tags.area.y - 2) as usize;
                                let tag_index = self.tags.table_state.offset() + row_in_viewport;
                                if tag_index < self.tags.visible.len() {
                                    self.tags.table_state.select(Some(tag_index));
                                }
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        self.tick_preview_debounce();
        Ok(())
    }

    fn clear_search(&mut self) {
        self.search.query.clear();
        self.tags.clear_filter();
    }

    fn handle_direct_key(&mut self, code: KeyCode) {
        if self.search.active {
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
                    self.tags.filter(&self.search.query);
                }
                KeyCode::Char(c) => {
                    self.search.query.push(c);
                    self.tags.filter(&self.search.query);
                }
                _ => {}
            }
        } else {
            match code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    if self.search.query.is_empty() {
                        self.should_quit = true;
                    } else {
                        self.clear_search();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => self.tags.scroll_down(1),
                KeyCode::Up | KeyCode::Char('k') => self.tags.scroll_up(1),
                KeyCode::Char('/') => self.search.active = true,
                KeyCode::Char('p') => self.preview.toggle(self.meta.path.as_deref()),
                KeyCode::Right | KeyCode::Char('l') => self.tags.expand_selected(),
                KeyCode::Left | KeyCode::Char('h') => self.tags.collapse_parent(),
                _ => {}
            }
        }
    }

    fn handle_explorer_key(&mut self, code: KeyCode) {
        match self.layout.focus {
            Focus::Explorer => self.handle_explorer_focus_key(code),
            Focus::TagTable => self.handle_tagtable_focus_key(code),
        }
    }

    fn handle_explorer_focus_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Tab => {
                if self.has_dicom_loaded() {
                    self.layout.focus = Focus::TagTable;
                }
            }
            KeyCode::Char('p') => self.preview.toggle(self.meta.path.as_deref()),
            KeyCode::Enter => {
                let input = key_to_explorer_input(code);
                if let Some(ref mut explorer) = self.layout.explorer {
                    let _ = explorer.handle(input);
                }
                self.check_explorer_selection();
                if self.has_dicom_loaded() {
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
        if self.search.active {
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
                    self.tags.filter(&self.search.query);
                }
                KeyCode::Char(c) => {
                    self.search.query.push(c);
                    self.tags.filter(&self.search.query);
                }
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::Tab => {
                self.layout.focus = Focus::Explorer;
            }
            KeyCode::Esc => {
                if !self.search.query.is_empty() {
                    self.clear_search();
                } else {
                    self.layout.focus = Focus::Explorer;
                }
            }
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.tags.scroll_down(1),
            KeyCode::Up | KeyCode::Char('k') => self.tags.scroll_up(1),
            KeyCode::Char('/') => self.search.active = true,
            KeyCode::Char('p') => self.preview.toggle(self.meta.path.as_deref()),
            KeyCode::Right | KeyCode::Char('l') => self.tags.expand_selected(),
            KeyCode::Left | KeyCode::Char('h') => self.tags.collapse_parent(),
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
            if clicked == selected && explorer.current().is_dir() {
                let _ = explorer.handle(Input::Right);
            } else {
                explorer.set_selected_idx(clicked);
            }
        }

        self.check_explorer_selection();
    }

    fn check_explorer_selection(&mut self) {
        let current_path = self
            .layout
            .explorer
            .as_ref()
            .map(|e| e.current().path().to_path_buf());

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
