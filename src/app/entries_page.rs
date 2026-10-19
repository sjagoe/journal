use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use color_eyre::Result;

use ratatui::{
    layout::Rect,
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem},
    Frame,
};

use crate::app::pagination::pagination;
use crate::traits::{Drawable, Navigable};
use crate::config::Config;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Entries {
    generic: Vec<String>,
    narrative: Vec<String>,
}

impl Entries {
    fn len(&self) -> usize {
        self.narrative.len() + self.generic.len()
    }

    fn items(&self) -> Vec<String> {
        self.narrative.iter().cloned().chain(self.generic.iter().cloned()).collect()
    }
}

pub struct EntriesPage {
    entries: Entries,
    selected_index: usize,
}

impl EntriesPage {
    pub fn new(config: &Config) -> EntriesPage {
        let mut entries_path = PathBuf::new();
        entries_path.push(&config.journal_path);
        entries_path.push("entries.json");

        let file = File::open(entries_path)
            .expect("Unable to open entries data");
        let reader = BufReader::new(file);
        let entries = serde_json::from_reader(reader)
            .expect("Unable to deserialize entries");

        EntriesPage {
            entries: entries,
            selected_index: 0,
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn items(&self) -> Vec<String> {
        self.entries.items()
    }
}

impl Drawable for EntriesPage {
    fn draw(&self, f: &mut Frame, area: Rect, content_block: Block) -> Rect {
        let viewport = content_block.inner(area);

        let mut list_items = Vec::<ListItem>::new();
        let items = self.items();

        let visible = viewport.height;
        let (_page, _pages, page_start_index) =
            pagination(items.len(), visible.into(), self.selected_index);

        let end_slice = if page_start_index + usize::from(visible) >= items.len() {
            items.len()
        } else {
            page_start_index + usize::from(visible)
        };
        let display_items = &items[page_start_index..end_slice];

        for (pos, item) in display_items.iter().enumerate() {
            let style = if pos + page_start_index == self.selected_index {
                Style::default().fg(Color::Black).bg(Color::Cyan)
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
        f.render_widget(content, area);
        viewport
    }

    fn title(&self) -> String {
        "title".into()
    }
}
impl Navigable for EntriesPage {
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
        let items = &self.items();
        if items.is_empty() {
            "".to_string()
        } else {
            items[self.selected_index].to_string()
        }
    }

    fn next_tick(&mut self, _block: bool) -> Result<()> {
        Ok(())
    }
}
