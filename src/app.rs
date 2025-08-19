use color_eyre::Result;
use ratatui::{
    layout::Rect,
    prelude::{Modifier, Text},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        block::{Padding, Title},
        Block, Borders, Paragraph,
    },
    Frame,
};

use crate::config::Config;

pub struct App {
    config: Config,
}

impl App {
    pub fn new(config: &Config) -> App {
        App {
            config: config.clone(),
        }
    }
    pub fn title(&self) -> Vec<Span> {
        let mut parts = vec![Span::from(" ")];

        parts.push(Span::styled(
            "Entries",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));

        parts.push(Span::from(" "));
        parts
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect) {
        let title = Title::from(self.title());
        let content_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .style(Style::default())
            .title(title);

        content_block.inner(area);
        // let viewport = if let Some(page) = match self.mode() {
        //     AppMode::BrowseRefs => Some(Box::<&dyn Drawable>::new(&self.refs_page)),
        //     AppMode::BrowseTrees => Some(Box::<&dyn Drawable>::new(
        //         self.tree_pages
        //             .last()
        //             .expect("No tree browsing page in tree mode"),
        //     )),
        //     AppMode::ViewBlob => Some(Box::<&dyn Drawable>::new(
        //         self.blob_pager
        //             .as_ref()
        //             .expect("No blob browser page in blob mode"),
        //     )),
        //     _ => None,
        // } {
        //     page.draw(f, area, content_block)
        // } else {
        //     content_block.inner(area)
        // };

        // self.set_height(viewport.height);

        // if let Some(error) = self.active_error {
        //     self.display_error(f, &error);
        // }
    }
}
