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

mod entries_pane;

use crate::app::entries_page::entries_pane::{ActivePane, EntriesPane};
use crate::config::Config;
use crate::traits::{Drawable, Navigable};

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Entries {
    documents: Vec<String>,
    general: Vec<String>,
    narrative: Vec<String>,
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
