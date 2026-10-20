use color_eyre::Result;

use ratatui::{
    Frame,
    layout::Rect,
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem},
};

use crate::app::pagination::pagination;
use crate::traits::{Drawable, Navigable};

#[derive(PartialEq)]
pub enum ActivePane {
    General,
    Narrative,
}

pub struct EntriesPane {
    entries: Vec<String>,
    selected_index: usize,
    pane_name: String,
    active: bool,
    pane_kind: ActivePane,
}

impl EntriesPane {
    pub fn new(name: String, entries: Vec<String>, pane_kind: ActivePane) -> EntriesPane {
        EntriesPane {
            entries,
            selected_index: 0,
            pane_name: name,
            active: false,
            pane_kind,
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn activate(&mut self, active_pane: &ActivePane) {
        self.active = active_pane == &self.pane_kind;
    }
}

impl Drawable for EntriesPane {
    fn draw(&self, frame: &mut Frame, area: Rect, content_block: Block) -> Rect {
        let viewport = content_block.inner(area);

        let mut list_items = Vec::<ListItem>::new();

        let visible = viewport.height;
        let (_page, _pages, page_start_index) =
            pagination(self.entries.len(), visible.into(), self.selected_index);

        let end_slice = if page_start_index + usize::from(visible) >= self.entries.len() {
            self.entries.len()
        } else {
            page_start_index + usize::from(visible)
        };
        let display_items = &self.entries[page_start_index..end_slice];

        for (pos, item) in display_items.iter().enumerate() {
            let style = if pos + page_start_index == self.selected_index {
                let [fg, bg] = match self.active {
                    true => [Color::Black, Color::Cyan],
                    false => [Color::White, Color::DarkGray],
                };
                Style::default().fg(fg).bg(bg)
            } else {
                Style::default().fg(Color::Gray)
            };
            let line = Line::from(vec![
                Span::styled(
                    format!("{:10}", "ref"),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(item, style),
            ]);
            list_items.push(ListItem::new(line));
        }

        let content = List::new(list_items).block(content_block);
        frame.render_widget(content, area);

        viewport
    }

    fn title(&self) -> Vec<Span<'_>> {
        let mut parts = vec![Span::from(" ")];
        let fg = match self.active {
            true => Color::Green,
            false => Color::Gray,
        };

        parts.push(Span::styled(
            self.pane_name.clone(),
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ));

        parts.push(Span::from(" "));
        parts
    }
}

impl Navigable for EntriesPane {
    fn home(&mut self, _page_size: u16) {
        self.selected_index = 0;
    }

    fn end(&mut self, _page_size: u16) {
        self.selected_index = if self.len() > 0 { self.len() - 1 } else { 0 };
    }

    fn pagedown(&mut self, page_size: u16) {
        let h: usize = page_size.into();
        let index = self.selected_index.saturating_add(h);
        self.selected_index = index.min(self.len().saturating_sub(1));
    }

    fn pageup(&mut self, page_size: u16) {
        let h: usize = page_size.into();
        self.selected_index = self.selected_index.saturating_sub(h);
    }

    fn next_selection(&mut self) {
        if self.selected_index < self.len() - 1 {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    fn previous_selection(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.len() - 1;
        }
    }

    fn select(&self) -> Option<String> {
        Some("test".into())
    }

    fn selected_item(&self) -> String {
        let items = &self.entries;
        if items.is_empty() {
            "".to_string()
        } else {
            items[self.selected_index].to_string()
        }
    }

    fn switch_pane(&mut self) {}

    fn next_tick(&mut self, _block: bool) -> Result<()> {
        Ok(())
    }
}
