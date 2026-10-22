use ratatui::{Frame, layout::Rect, text::Span, widgets::Block};

use crate::journal::Journal;

pub trait Navigable {
    fn home(&mut self, page_size: u16);
    fn end(&mut self, page_size: u16);
    fn pagedown(&mut self, page_size: u16);
    fn pageup(&mut self, page_size: u16);
    fn next_selection(&mut self);
    fn previous_selection(&mut self);
    fn select(&self, journal: &Journal) -> Option<String>;
    // fn selected_item(&self) -> String;
    fn switch_pane(&mut self);
}

pub trait Drawable {
    fn draw(&self, f: &mut Frame, area: Rect, content_block: Block, journal: &Journal) -> Rect;

    fn title(&self) -> Vec<Span<'_>>;
}
