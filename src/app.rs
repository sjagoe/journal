use entry_editor::EntryEditor;
use git2::Repository;
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
mod entry_editor;
pub mod events;
mod pagination;

use crate::app::events::{ActionInfo, Event};
use crate::config::Config;
use crate::errors::JournalError;
use crate::traits::{Drawable, Navigable};
use crate::{app::entries_page::EntriesPage, journal::Journal};

pub struct Redraw(pub bool);

pub struct App {
    config: Config,
    journal: Journal,
    entries_page: EntriesPage,
    height: u16,
}

impl App {
    pub fn new(config: &Config) -> App {
        let repo = match Repository::open(&config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        let journal = Journal::from_file(&config.entries_path(), &repo);
        let entries_page = EntriesPage::new(&journal);
        App {
            config: config.clone(),
            journal,
            entries_page,
            height: 0,
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

    pub fn set_height(&mut self, h: u16) {
        self.height = h;
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
        let viewport = page.draw(frame, area, content_block, &self.journal);

        self.set_height(viewport.height);

        // if let Some(error) = self.active_error {
        //     self.display_error(f, &error);
        // }
    }

    pub fn draw_context_hint(&self, f: &mut Frame, area: Rect) {
        let actions = [Event::Exit];

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

    fn edit_entry(&self, entry: &str) -> Result<(), JournalError> {
        let external_editor = EntryEditor::new(entry, &self.config)?;
        let file_changed = external_editor.edit()?;
        if !file_changed {
            return Ok(());
        }

        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };
        self.journal.reload(&repo)?;

        Ok(())
    }

    pub fn handle_event(&mut self, event: &Event) -> Result<Redraw, JournalError> {
        let page: Box<&mut dyn Navigable> = Box::new(&mut self.entries_page);
        match event {
            Event::Select => {
                if let Some(entry) = page.select(&self.journal) {
                    self.edit_entry(&entry)?
                }
                return Ok(Redraw(true));
            }
            Event::Home => page.home(self.height),
            Event::End => page.end(self.height),
            Event::PageUp => page.pageup(self.height),
            Event::PageDown => page.pagedown(self.height),
            Event::NextSelection => page.next_selection(),
            Event::PreviousSelection => page.previous_selection(),
            Event::SwitchPane => page.switch_pane(),
            _ => {}
        };
        Ok(Redraw(false))
    }

    pub fn error(&mut self, _error: JournalError) {}
}
