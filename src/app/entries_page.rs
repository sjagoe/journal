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

struct EntriesPane {
    entries: Vec<String>,
    selected_index: usize,
    pane_name: String,
    active: bool,
    pane_kind: ActivePane,
}

#[derive(PartialEq)]
enum ActivePane {
    General,
    Narrative,
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

    fn activate(&mut self, active_pane: &ActivePane) {
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

pub struct EntriesPage {
    _documents: Vec<String>,
    general_pane: EntriesPane,
    narrative_pane: EntriesPane,
    active_pane: ActivePane,
}

impl EntriesPage {
    pub fn new(config: &Config) -> EntriesPage {
        let mut entries_path = PathBuf::new();
        entries_path.push(&config.journal_path);
        entries_path.push("entries.json");

        let file = File::open(entries_path).expect("Unable to open entries data");
        let reader = BufReader::new(file);
        let entries: Entries =
            serde_json::from_reader(reader).expect("Unable to deserialize entries");

        let active_pane = ActivePane::General;
        let mut general_pane = EntriesPane::new(
            "General Entries".into(),
            entries.general,
            ActivePane::General,
        );
        general_pane.activate(&active_pane);
        let mut narrative_pane = EntriesPane::new(
            "Narrative Entries".into(),
            entries.narrative,
            ActivePane::Narrative,
        );
        narrative_pane.activate(&active_pane);

        EntriesPage {
            _documents: entries.documents,
            general_pane: general_pane,
            narrative_pane: narrative_pane,
            active_pane: ActivePane::General,
        }
    }

    fn current_pane(&mut self) -> &mut EntriesPane {
        match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        }
    }
}

impl Drawable for EntriesPage {
    fn draw(&self, frame: &mut Frame, area: Rect, content_block: Block) -> Rect {
        let viewport = content_block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)]);

        let [general_area, narrative_area] = layout.areas(viewport);

        let general_pane = Box::<&dyn Drawable>::new(&self.general_pane);
        let general_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .style(Style::default())
            .title(Line::from(general_pane.title()));
        general_pane.draw(frame, general_area, general_block);

        let narrative_pane = Box::<&dyn Drawable>::new(&self.narrative_pane);
        let narrative_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .style(Style::default())
            .title(Line::from(narrative_pane.title()));
        narrative_pane.draw(frame, narrative_area, narrative_block);

        frame.render_widget(content_block, area);

        viewport
    }

    fn title(&self) -> Vec<Span<'_>> {
        vec![Span::from(" ")]
    }
}

impl Navigable for EntriesPage {
    fn home(&mut self, page_size: u16) {
        self.current_pane().home(page_size);
    }

    fn end(&mut self, page_size: u16) {
        self.current_pane().end(page_size);
    }

    fn pagedown(&mut self, page_size: u16) {
        self.current_pane().pagedown(page_size);
    }

    fn pageup(&mut self, page_size: u16) {
        self.current_pane().pageup(page_size);
    }

    fn next_selection(&mut self) {
        self.current_pane().next_selection();
    }

    fn previous_selection(&mut self) {
        self.current_pane().previous_selection();
    }

    fn select(&self) -> Option<String> {
        let pane = match self.active_pane {
            ActivePane::General => &self.general_pane,
            ActivePane::Narrative => &self.narrative_pane,
        };
        pane.select()
    }

    fn selected_item(&self) -> String {
        let pane = match self.active_pane {
            ActivePane::General => &self.general_pane,
            ActivePane::Narrative => &self.narrative_pane,
        };
        pane.selected_item()
    }

    fn switch_pane(&mut self) {
        let new_active = match self.active_pane {
            ActivePane::General => ActivePane::Narrative,
            ActivePane::Narrative => ActivePane::General,
        };
        self.narrative_pane.activate(&new_active);
        self.general_pane.activate(&new_active);
        self.active_pane = new_active;
    }

    fn next_tick(&mut self, _block: bool) -> Result<()> {
        Ok(())
    }
}
