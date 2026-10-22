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

use crate::config::Config;
use crate::errors::JournalError;
use crate::traits::{Drawable, Navigable};
use crate::{app::entries_page::EntriesPage, journal::Journal};
use crate::{
    app::events::{ActionInfo, Event},
    journal::EntryKind,
};

pub struct Redraw(pub bool);

enum AppMode {
    Standard,
    NewEntry,
    Error,
}

pub struct App {
    config: Config,
    journal: Journal,
    entries_page: EntriesPage,
    entry_editor: EntryEditor,
    height: u16,
    mode: AppMode,
}

impl App {
    pub fn new(config: &Config) -> Result<App, JournalError> {
        let repo = match Repository::open(&config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        let journal = Journal::from_file(&config.entries_path(), &repo);
        let entries_page = EntriesPage::new(&journal);
        let entry_editor = EntryEditor::new(config)?;
        let app = App {
            config: config.clone(),
            journal,
            entries_page,
            entry_editor,
            height: 0,
            mode: AppMode::Standard,
        };
        Ok(app)
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
        let actions = [Event::Exit, Event::Select, Event::New];

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

    fn edit_entry(&mut self, entry: &str) -> Result<(), JournalError> {
        let file_changed = self.entry_editor.edit(entry)?;
        if !file_changed {
            return Ok(());
        }

        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };
        self.journal = Journal::from_file(&self.config.entries_path(), &repo);

        self.entries_page.update(&self.journal);

        Ok(())
    }

    fn new_entry(&mut self) -> Result<(), JournalError> {
        let new_entry = "new-entry";
        let kind = EntryKind::from(&self.entries_page.active_pane);

        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        self.journal = self.journal.with_new_entry(&kind, &new_entry, &repo);

        self.edit_entry(&new_entry)
    }

    pub fn handle_event(&mut self, event: &Event) -> Result<Redraw, JournalError> {
        let page: Box<&mut dyn Navigable> = Box::new(&mut self.entries_page);
        match self.mode {
            AppMode::Standard => {
                match event {
                    Event::Select => {
                        if let Some(entry) = page.select(&self.journal) {
                            self.edit_entry(&entry)?
                        }
                        return Ok(Redraw(true));
                    },
                    Event::New => {
                        self.mode = AppMode::NewEntry;
                        return Ok(Redraw(true));
                    }
                    Event::Home => page.home(),
                    Event::End => page.end(),
                    Event::PageUp => page.pageup(self.height),
                    Event::PageDown => page.pagedown(self.height),
                    Event::NextSelection => page.next_selection(),
                    Event::PreviousSelection => page.previous_selection(),
                    Event::SwitchPane => page.switch_pane(),
                    _ => {}
                };
            }
            AppMode::NewEntry => {
                match event {
                    Event::Select => {
                        self.mode = AppMode::Standard;
                        self.new_entry()?;
                        return Ok(Redraw(true));
                    }
                    Event::Back => {
                        self.mode = AppMode::Standard;
                        return Ok(Redraw(true));
                    }
                    _ => {}
                }
            }
            AppMode::Error => {
                match event {
                    Event::Select | Event::Back => {
                        self.mode = AppMode::Standard;
                        return Ok(Redraw(true));
                    }
                    _ => {}
                }
            }
        }
        Ok(Redraw(false))
    }

    pub fn error(&mut self, _error: JournalError) {}
}
