use ratatui::{
    Frame,
    layout::Rect,
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Paragraph,
        block::{Padding, Title},
    },
};

mod entries_page;
mod events;
mod pagination;

use crate::app::entries_page::EntriesPage;
use crate::app::events::{ActionInfo, Event};
use crate::config::Config;
use crate::traits::Drawable;

pub struct App {
    _config: Config,
    entries_page: EntriesPage,
}

impl App {
    pub fn new(config: &Config) -> App {
        App {
            _config: config.clone(),
            entries_page: EntriesPage::new(config),
        }
    }
    pub fn title(&self) -> Vec<Span<'_>> {
        let mut parts = vec![Span::from(" ")];

        parts.push(Span::styled(
            "Journal Navigator",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

        parts.push(Span::from(" "));
        parts
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let title = Title::from(self.title());
        let content_block = Block::default()
            .padding(Padding::horizontal(0))
            .borders(Borders::TOP)
            .border_type(BorderType::Double)
            .style(Style::default())
            .title(title);

        let page = Box::<&dyn Drawable>::new(&self.entries_page);
        page.draw(frame, area, content_block);

        // self.set_height(viewport.height);

        // if let Some(error) = self.active_error {
        //     self.display_error(f, &error);
        // }
    }

    pub fn draw_context_hint(&self, f: &mut Frame, area: Rect) {
        let actions = vec![Event::Exit];

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
