use crossterm::event::KeyCode;
use entry_editor::EntryEditor;
use git2::Repository;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
};

use time::macros::format_description;
use time::{OffsetDateTime, format_description::StaticFormatDescription};

mod entries_page;
mod entry_editor;
pub mod events;
mod pagination;

use crate::app::entries_page::ActivePane;
use crate::errors::{ErrorKind, JournalError};
use crate::traits::{Drawable, Navigable};
use crate::{app::entries_page::EntriesPage, journal::Journal};
use crate::{
    app::events::{ActionInfo, Event},
    journal::EntryKind,
};
use crate::{config::Config, ui::centered_rect};

pub struct Redraw(pub bool);

static DEFAULT_ENTRY_FILENAME_FORMAT: StaticFormatDescription =
    format_description!("[year]-[month]");

trait SetText {
    fn set_text(&mut self, text: &str);
}

impl SetText for ratin::InputField {
    fn set_text(&mut self, text: &str) {
        for char in text.chars() {
            self.handle_key(KeyCode::Char(char));
        }
    }
}

enum AppMode {
    Standard,
    NewEntry,
    Error(JournalError),
}

pub struct App {
    config: Config,
    journal: Journal,
    entries_page: EntriesPage,
    entry_editor: EntryEditor,
    height: u16,
    mode: AppMode,
    text_entry: Option<ratin::InputField>,
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
            text_entry: None,
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

    fn display_error(&self, frame: &mut Frame, error: &JournalError) {
        let area = centered_rect(
            Constraint::Percentage(60),
            Constraint::Percentage(25),
            frame.area(),
        );
        let popup_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .title(Span::styled(
                " Error ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(Color::DarkGray));
        let content = Paragraph::new(Text::styled(
            error.as_str().to_string(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .block(popup_block);
        frame.render_widget(Clear, area);
        frame.render_widget(content, area);
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let title = Line::from(self.title());

        let content_block = Block::default()
            .padding(Padding::horizontal(0))
            .borders(Borders::TOP)
            .border_type(BorderType::Double)
            .style(Style::default())
            .title(title);

        let page = Box::<&dyn Drawable>::new(&self.entries_page);
        let viewport = page.draw(frame, area, content_block, &self.journal);

        self.set_height(viewport.height);

        if let Some(ref mut text_entry) = self.text_entry {
            let entry_area = centered_rect(
                Constraint::Percentage(60),
                Constraint::Length(3),
                frame.area(),
            );
            frame.render_widget(Clear, entry_area);
            text_entry.render(frame, entry_area);
        }

        if let AppMode::Error(ref error) = self.mode {
            self.display_error(frame, error);
        }
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

    fn new_entry(&mut self, new_entry: &str) -> Result<(), JournalError> {
        let kind = EntryKind::from(&self.entries_page.active_pane);

        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        self.journal = self.journal.with_new_entry(&kind, new_entry, &repo);

        self.edit_entry(new_entry)
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
                    }
                    Event::New => {
                        let now = OffsetDateTime::now_local().map_err(|_| {
                            JournalError::chain(&[ErrorKind::NewEntry, ErrorKind::GetCurrentTime])
                        })?;
                        let default_name =
                            now.format(DEFAULT_ENTRY_FILENAME_FORMAT).map_err(|_| {
                                JournalError::chain(&[
                                    ErrorKind::NewEntry,
                                    ErrorKind::FormatDateTime,
                                ])
                            })?;

                        match self.entries_page.active_pane {
                            ActivePane::General => {
                                self.new_entry(&default_name)?;
                            }
                            ActivePane::Narrative => {
                                let mut text_entry = ratin::InputField::new()
                                    .with_placeholder("New entry name ...")
                                    .with_max_length(50);
                                let name_prefix = format!("{}-", default_name);
                                text_entry.set_text(&name_prefix);
                                self.text_entry = Some(text_entry);
                                self.mode = AppMode::NewEntry;
                            }
                        }
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
            AppMode::NewEntry => match event {
                Event::Select => {
                    self.mode = AppMode::Standard;
                    let entry_name = if let Some(ref mut text_entry) = self.text_entry {
                        text_entry.get_text().to_owned()
                    } else {
                        return Err(JournalError::chain(&[
                            ErrorKind::InvalidMode,
                            ErrorKind::NewEntry,
                            ErrorKind::TextEntryFailed,
                        ]));
                    };
                    self.new_entry(&entry_name)?;
                    self.text_entry = None;
                    return Ok(Redraw(true));
                }
                Event::Back => {
                    self.mode = AppMode::Standard;
                    self.text_entry = None;
                    return Ok(Redraw(true));
                }
                Event::Home => {
                    if let Some(ref mut text_entry) = self.text_entry {
                        text_entry.handle_key(KeyCode::Home);
                    } else {
                        return Err(JournalError::chain(&[
                            ErrorKind::InvalidMode,
                            ErrorKind::NewEntry,
                            ErrorKind::TextEntryFailed,
                        ]));
                    }
                }
                Event::End => {
                    if let Some(ref mut text_entry) = self.text_entry {
                        text_entry.handle_key(KeyCode::End);
                    } else {
                        return Err(JournalError::chain(&[
                            ErrorKind::InvalidMode,
                            ErrorKind::NewEntry,
                            ErrorKind::TextEntryFailed,
                        ]));
                    }
                }
                Event::TextEntry(key) => {
                    if let Some(ref mut text_entry) = self.text_entry {
                        text_entry.handle_key(*key);
                    } else {
                        return Err(JournalError::chain(&[
                            ErrorKind::InvalidMode,
                            ErrorKind::NewEntry,
                            ErrorKind::TextEntryFailed,
                        ]));
                    }
                }
                _ => {}
            },
            AppMode::Error(_) => match event {
                Event::Select | Event::Back => {
                    self.mode = AppMode::Standard;
                    return Ok(Redraw(true));
                }
                _ => {}
            },
        }
        Ok(Redraw(false))
    }

    pub fn error(&mut self, error: JournalError) {
        self.mode = AppMode::Error(error);
    }
}
