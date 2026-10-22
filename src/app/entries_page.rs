use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Padding},
    Frame,
};

mod entries_pane;

use crate::traits::{Drawable, Navigable};
use crate::{
    app::entries_page::entries_pane::{ActivePane, EntriesPane},
    journal::Journal,
};

pub struct EntriesPage {
    general_pane: EntriesPane,
    narrative_pane: EntriesPane,
    pub active_pane: ActivePane,
}

impl EntriesPage {
    pub fn new(journal: &Journal) -> EntriesPage {
        let active_pane = ActivePane::General;

        EntriesPage {
            general_pane: EntriesPane::new(journal, ActivePane::General, &active_pane),
            narrative_pane: EntriesPane::new(journal, ActivePane::Narrative, &active_pane),
            active_pane,
        }
    }

    pub fn update(&mut self, journal: &Journal) {
        self.general_pane.update(journal);
        self.narrative_pane.update(journal);
    }
}

impl Drawable for EntriesPage {
    fn draw(&self, frame: &mut Frame, area: Rect, content_block: Block, journal: &Journal) -> Rect {
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
        general_pane.draw(frame, general_area, general_block, journal);

        let narrative_pane = Box::<&dyn Drawable>::new(&self.narrative_pane);
        let narrative_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .style(Style::default())
            .title(Line::from(narrative_pane.title()));
        narrative_pane.draw(frame, narrative_area, narrative_block, journal);

        frame.render_widget(content_block, area);

        viewport
    }

    fn title(&self) -> Vec<Span<'_>> {
        vec![Span::from(" ")]
    }
}

impl Navigable for EntriesPage {
    fn home(&mut self) {
        let pane = match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        };
        pane.home();
    }

    fn end(&mut self) {
        let pane = match self.active_pane {
            ActivePane::General => &mut self.general_pane,
            ActivePane::Narrative => &mut self.narrative_pane,
        };
        pane.end();
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

    fn select(&self, journal: &Journal) -> Option<String> {
        let pane = match self.active_pane {
            ActivePane::General => &self.general_pane,
            ActivePane::Narrative => &self.narrative_pane,
        };
        pane.select(journal)
    }

    // fn selected_item(&self) -> String {
    //     let pane = match self.active_pane {
    //         ActivePane::General => &self.general_pane,
    //         ActivePane::Narrative => &self.narrative_pane,
    //     };
    //     pane.selected_item()
    // }

    fn switch_pane(&mut self) {
        let new_active = match self.active_pane {
            ActivePane::General => ActivePane::Narrative,
            ActivePane::Narrative => ActivePane::General,
        };
        self.narrative_pane.activate(&new_active);
        self.general_pane.activate(&new_active);
        self.active_pane = new_active;
    }
}
