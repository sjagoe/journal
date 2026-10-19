use ratatui::{
    Frame, layout::Rect, prelude::Modifier, style::{Color, Style}, text::{Line, Span}, widgets::{
        Block, Borders, Paragraph, block::{Padding, Title},
    },
};

mod entries_page;
mod pagination;
mod navigation;

use crate::config::Config;
use crate::app::navigation::{ActionInfo, NavigationAction};

pub struct App {
    _config: Config,
}

impl App {
    pub fn new(config: &Config) -> App {
        App {
            _config: config.clone(),
        }
    }
    pub fn title(&self) -> Vec<Span<'_>> {
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

        f.render_widget(content_block, area);
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

    pub fn draw_context_hint(&self, f: &mut Frame, area: Rect) {
        let actions = vec![
            NavigationAction::Exit,
        ];

        let keys_hint = actions
            .iter()
            .map(|a| ActionInfo::from(a).to_string())
            .collect::<Vec<String>>()
            .join(" | ");
        let content = Span::styled(keys_hint, Style::default());
        let block = Block::default()
            .padding(Padding::horizontal(1))
            .style(Style::default().fg(Color::Black).bg(Color::Gray));
        let hint = Paragraph::new(Line::from(content)).block(block);
        f.render_widget(hint, area);
    }
}
