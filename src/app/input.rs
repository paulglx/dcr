use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use std::io;

use super::App;

impl App {
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
                                if self.search_query.is_empty() {
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
                            KeyCode::Char('p') => {
                                self.toggle_preview();
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
