use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use color_eyre::Result;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding},
};

use crate::app::pagination::pagination;
use crate::config::Config;
use crate::traits::{Drawable, Navigable};

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Entries {
    documents: Vec<String>,
    general: Vec<String>,
    narrative: Vec<String>,
}

impl Entries {
    fn len(&self) -> usize {
        self.narrative.len() + self.general.len()
    }

    fn items(&self) -> Vec<String> {
        self.narrative
            .iter()
            .cloned()
            .chain(self.general.iter().cloned())
            .collect()
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

        let file = File::open(entries_path).expect("Unable to open entries data");
        let reader = BufReader::new(file);
        let entries = serde_json::from_reader(reader).expect("Unable to deserialize entries");

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

    fn draw_pane(&self, frame: &mut Frame, items: &Vec<String>, area: Rect, content_block: Block) {
        let viewport = content_block.inner(area);

        let mut list_items = Vec::<ListItem>::new();

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
        frame.render_widget(content, area);
    }
}

impl Drawable for EntriesPage {
    fn draw(&self, frame: &mut Frame, area: Rect, content_block: Block) -> Rect {
        let viewport = content_block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)]);

        let [general_pane, narrative_pane] = layout.areas(area);

        let general_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .style(Style::default())
            .title(Line::from("General Entries"));

        self.draw_pane(frame, &self.entries.general, general_pane, general_block);

        let narrative_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .style(Style::default())
            .title(Line::from("Narrative Entries"));

        self.draw_pane(
            frame,
            &self.entries.narrative,
            narrative_pane,
            narrative_block,
        );

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
