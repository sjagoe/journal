use crossterm::event::KeyCode;
use ratatui::{Frame, layout::Rect};

pub(crate) struct InputField {
}

impl InputField {
    pub(crate) fn new(title: &str, placeholder: &str) -> Self {
        Self { }
    }

    pub(crate) fn set_text(&mut self, text: &str) {}

    pub(crate) fn get_text(&self) -> &str {
        ""
    }

    pub(crate) fn clear(&mut self) {}

    pub(crate) fn handle_key(&mut self, code: &KeyCode) {}

    pub(crate) fn draw(&self, frame: &mut Frame, area: Rect) {}
}
