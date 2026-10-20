use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use color_eyre::Result;

use git2::Repository;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding},
};
use serde::Deserialize;

mod entries_pane;

use crate::app::entries_page::entries_pane::{ActivePane, EntriesPane};
use crate::config::Config;
use crate::traits::{Drawable, Navigable};

#[derive(Clone, Debug, Deserialize)]
pub struct Entries {
    documents: Vec<String>,
    general: Vec<String>,
    narrative: Vec<String>,
}

pub struct EntriesPage<'a> {
    _documents: Vec<String>,
    repo: &'a Repository,
    general_pane: EntriesPane,
    narrative_pane: EntriesPane,
    active_pane: ActivePane,
}

impl<'a> EntriesPage<'a> {
    pub fn new(config: &Config, repo: &'a Repository) -> EntriesPage<'a> {
        let mut entries_path = PathBuf::new();
        entries_path.push(&config.journal_path);
        entries_path.push("entries.json");

        let active_pane = ActivePane::General;

        let file = File::open(entries_path).expect("Unable to open entries data");
        let reader = BufReader::new(file);
        let entries: Entries =
            serde_json::from_reader(reader).expect("Unable to deserialize entries");

        EntriesPage {
            _documents: entries.documents,
            repo: repo,
            general_pane: EntriesPane::new(
                &repo,
                entries.general,
                ActivePane::General,
                &active_pane,
            ),
            narrative_pane: EntriesPane::new(
                &repo,
                entries.narrative,
                ActivePane::Narrative,
                &active_pane,
            ),
            active_pane,
        }
    }
}

impl Drawable for EntriesPage<'_> {
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

impl Navigable for EntriesPage<'_> {
    fn home(&mut self, page_size: u16) {
        let pane = match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        };
        pane.home(page_size);
    }

    fn end(&mut self, page_size: u16) {
        let pane = match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        };
        pane.end(page_size);
    }

    fn pagedown(&mut self, page_size: u16) {
        let pane = match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        };
        pane.pagedown(page_size);
    }

    fn pageup(&mut self, page_size: u16) {
        let pane = match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        };
        pane.pageup(page_size);
    }

    fn next_selection(&mut self) {
        let pane = match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        };
        pane.next_selection();
    }

    fn previous_selection(&mut self) {
        let pane = match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        };
        pane.previous_selection();
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
