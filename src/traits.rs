use color_eyre::Result;

use ratatui::{Frame, layout::Rect, text::Span, widgets::Block};

pub trait Navigable {
    fn home(&mut self, page_size: u16);
    fn end(&mut self, page_size: u16);
    fn pagedown(&mut self, page_size: u16);
    fn pageup(&mut self, page_size: u16);
    fn next_selection(&mut self);
    fn previous_selection(&mut self);
    fn select(&self) -> Option<String>;
    // fn selected_item(&self) -> String;
    fn switch_pane(&mut self);
}

pub trait Drawable {
    fn draw(&self, f: &mut Frame, area: Rect, content_block: Block) -> Rect;

    fn title(&self) -> Vec<Span<'_>>;
}
