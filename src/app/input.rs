use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui_explorer::Input;
use std::io;

use super::state::{AppMode, Focus};
use super::App;

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
                    if self.mode == AppMode::Direct {
                        self.handle_direct_key(key.code);
                    } else {
                        self.handle_explorer_key(key.code);
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => self.scroll_down(3),
                    MouseEventKind::ScrollUp => self.scroll_up(3),
                    MouseEventKind::Down(MouseButton::Left) => {
                        if self.mode == AppMode::Explorer {
                            let hit = |area: Rect| {
                                mouse.column >= area.x
                                    && mouse.column < area.x + area.width
                                    && mouse.row >= area.y
                                    && mouse.row < area.y + area.height
                            };
                            if self.has_dicom_loaded() && hit(self.table_area) {
                                self.focus = Focus::TagTable;
                            } else if hit(self.explorer_area) {
                                self.focus = Focus::Explorer;
                            }
                        }
                        if self.mode == AppMode::Direct || self.focus == Focus::TagTable {
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
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        self.tick_preview_debounce();
        Ok(())
    }

    fn handle_direct_key(&mut self, code: KeyCode) {
        if self.search_mode {
            match code {
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
            match code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    if self.search_query.is_empty() {
                        self.should_quit = true;
                    } else {
                        self.search_query.clear();
                        self.filtered_tags = None;
                        self.rebuild_visible_tags();
                        self.reset_selection();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => self.scroll_down(1),
                KeyCode::Up | KeyCode::Char('k') => self.scroll_up(1),
                KeyCode::Char('/') => self.search_mode = true,
                KeyCode::Char('p') => self.toggle_preview(),
                KeyCode::Right | KeyCode::Char('l') => self.expand_selected(),
                KeyCode::Left | KeyCode::Char('h') => self.collapse_parent(),
                _ => {}
            }
        }
    }

    fn handle_explorer_key(&mut self, code: KeyCode) {
        match self.focus {
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
                    self.focus = Focus::TagTable;
                }
            }
            KeyCode::Char('p') => self.toggle_preview(),
            KeyCode::Enter => {
                let input = key_to_explorer_input(code);
                if let Some(ref mut explorer) = self.explorer {
                    let _ = explorer.handle(input);
                }
                self.check_explorer_selection();
                if self.has_dicom_loaded() {
                    self.focus = Focus::TagTable;
                }
            }
            _ => {
                let input = key_to_explorer_input(code);
                if let Some(ref mut explorer) = self.explorer {
                    let _ = explorer.handle(input);
                }
                self.check_explorer_selection();
            }
        }
    }

    fn handle_tagtable_focus_key(&mut self, code: KeyCode) {
        if self.search_mode {
            match code {
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
            return;
        }

        match code {
            KeyCode::Tab => {
                self.focus = Focus::Explorer;
            }
            KeyCode::Esc => {
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.filtered_tags = None;
                    self.rebuild_visible_tags();
                    self.reset_selection();
                } else {
                    self.focus = Focus::Explorer;
                }
            }
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_up(1),
            KeyCode::Char('/') => self.search_mode = true,
            KeyCode::Char('p') => self.toggle_preview(),
            KeyCode::Right | KeyCode::Char('l') => self.expand_selected(),
            KeyCode::Left | KeyCode::Char('h') => self.collapse_parent(),
            _ => {}
        }
    }

    fn check_explorer_selection(&mut self) {
        let current_path = self
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

    pub fn scroll_down(&mut self, amount: usize) {
        if self.tags.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0);
        let max_index = self.tags.len().saturating_sub(1);
        let new_index = (current + amount).min(max_index);
        self.table_state.select(Some(new_index));
    }

    pub fn scroll_up(&mut self, amount: usize) {
        if self.tags.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0);
        let new_index = current.saturating_sub(amount);
        self.table_state.select(Some(new_index));
    }
}
