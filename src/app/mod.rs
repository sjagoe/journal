use crossterm::event::KeyEvent;
use git2::Repository;
use input::{InputHandler, InputMode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Modifier,
    style::{Color, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, LineGauge, Padding, Paragraph},
};

use time::macros::format_description;
use time::{OffsetDateTime, format_description::StaticFormatDescription};

mod actions;
mod entries_page;
mod input;
mod pagination;

use crate::{
    app::{
        actions::Action,
        entries_page::{ActivePane, EntriesPage},
    },
    config::Config,
    errors::{ErrorKind, JournalError},
    journal::{EntryKind, Journal, edit::EntryEditor, export::EntryExporter, git},
    traits::{Drawable, Navigable},
    ui::centered_rect,
};

pub struct Redraw(pub bool);

#[allow(unused)]
pub struct EventHandled(pub bool);

static DEFAULT_ENTRY_FILENAME_FORMAT: StaticFormatDescription =
    format_description!("[year]-[month]");
static NARRATIVE_ENTRY_FILENAME_SUFFIX_FORMAT: StaticFormatDescription =
    format_description!("-[day]_");

pub struct App<'a> {
    config: Config,
    journal: Journal,
    entries_page: EntriesPage,
    input_handler: InputHandler<'a>,
    height: u16,
    mode: InputMode,
    entry_exporter: Option<EntryExporter>,
}

impl<'a> App<'a> {
    pub fn new(config: &Config) -> Result<App<'_>, JournalError> {
        let repo = match Repository::open(&config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        let input_handler = InputHandler::new(config)?;

        let journal = Journal::from_file(&config.journal_path, &repo);
        let entries_page = EntriesPage::new(&journal);
        let app = App {
            config: config.clone(),
            journal,
            entries_page,
            input_handler,
            height: 0,
            mode: InputMode::Ready(None),
            entry_exporter: None,
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

    fn draw_export_progress(&mut self, frame: &mut Frame) {
        if let Some(ref exporter) = self.entry_exporter {
            let progress = exporter.get_progress();
            let progress_area = centered_rect(
                Constraint::Percentage(60),
                Constraint::Length(3),
                frame.area(),
            );

            let progress_bar = LineGauge::default()
                .block(Block::bordered().title(" Exporting ... "))
                .filled_style(Style::new().white().on_black().bold())
                .filled_symbol(symbols::block::FULL)
                .ratio(progress);

            frame.render_widget(Clear, progress_area);
            frame.render_widget(progress_bar, progress_area);
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(15), Constraint::Length(5)]);
        let [content_area, keys_area] = layout.areas(area);

        let title = Line::from(self.title());
        let content_block = Block::default()
            .padding(Padding::horizontal(0))
            .borders(Borders::TOP)
            .border_type(BorderType::Double)
            .style(Style::default())
            .title(title);

        let page = Box::<&dyn Drawable>::new(&self.entries_page);
        let content_viewport = page.draw(frame, content_area, content_block, &self.journal);

        self.set_height(content_viewport.height);

        let keys_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::NONE)
            .style(Style::default());
        self.input_handler
            .draw(frame, keys_area, keys_block, &self.mode);

        match self.mode {
            InputMode::Error(ref error) => self.display_error(frame, error),
            InputMode::NewEntry(_) | InputMode::Commit(_) => self.input_handler.render_input(frame),
            InputMode::ExportEntry(_) => self.draw_export_progress(frame),
            _ => {}
        }

        if let InputMode::Error(ref error) = self.mode {
            self.display_error(frame, error);
        }
    }

    // pub fn draw_context_hint(&self, f: &mut Frame, area: Rect) {
    //     let actions = [Event::BackOrExit, Event::Select, Event::New];

    //     let keys_hint = actions
    //         .iter()
    //         .map(|a| ActionInfo::from(a).to_string())
    //         .collect::<Vec<String>>()
    //         .join(" | ");
    //     let content = Span::styled(keys_hint, Style::default());
    //     let block = Block::default()
    //         .padding(Padding::horizontal(1))
    //         .style(Style::default().fg(Color::Black).bg(Color::Gray));
    //     let hint = Paragraph::new(Line::from(content)).block(block);
    //     f.render_widget(hint, area);
    // }

    fn edit_entry(
        &mut self,
        repo: &Repository,
        kind: &EntryKind,
        entry: &str,
    ) -> Result<bool, JournalError> {
        let entry_editor = EntryEditor::new(&self.config)?;
        let file_changed = entry_editor.edit(kind, entry)?;
        if !file_changed {
            return Ok(file_changed);
        }

        self.refresh_from_repo(repo);

        Ok(file_changed)
    }

    fn set_journal(&mut self, journal: Journal) {
        self.journal = journal;
        self.entries_page.update(&self.journal);
    }

    fn new_entry(&mut self, kind: &EntryKind, new_entry: &str) -> Result<(), JournalError> {
        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        let entry_editor = EntryEditor::new(&self.config)?;

        let created = entry_editor
            .create_entry(kind, new_entry)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::NewEntry, e]),
            })?;

        if created {
            self.journal
                .git_index_add_entry(&repo, kind, new_entry)
                .map_err(|err| match err {
                    JournalError::Error(e) => JournalError::chain(&[ErrorKind::NewEntry, e]),
                })?;
            self.refresh_from_repo(&repo);
        }

        let changed = self
            .edit_entry(&repo, kind, new_entry)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::NewEntry, e]),
            })?;

        if !changed {
            // Handle when the changed file was not re-read on edit,
            // because we created it but didn't edit afterwards.
            self.refresh_from_repo(&repo);
        }

        Ok(())
    }

    fn edit_existing_entry(&mut self, kind: &EntryKind, entry: &str) -> Result<(), JournalError> {
        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        self.edit_entry(&repo, kind, entry)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::EditingEntry, e]),
            })?;

        Ok(())
    }

    fn begin_export_entry(&mut self) -> Result<(), JournalError> {
        if let Some(ref mut entry_exporter) = self.entry_exporter {
            entry_exporter.start_export().map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::ExportingEntry, e]),
            })?;
        }
        Ok(())
    }

    fn refresh_from_repo(&mut self, repo: &Repository) {
        self.set_journal(Journal::from_file(&self.config.journal_path, repo));
    }

    fn refresh(&mut self) -> Result<(), JournalError> {
        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        self.refresh_from_repo(&repo);

        Ok(())
    }

    fn git_index_entry(
        &mut self,
        action: &dyn Fn(&Journal, &Repository) -> Result<(), JournalError>,
    ) -> Result<(), JournalError> {
        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        action(&self.journal, &repo).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::NewEntry, e]),
        })?;

        self.refresh_from_repo(&repo);

        Ok(())
    }

    fn git_add_entry(&mut self, kind: &EntryKind, entry: &str) -> Result<(), JournalError> {
        let action =
            |journal: &Journal, repo: &Repository| journal.git_index_add_entry(repo, kind, entry);

        self.git_index_entry(&action)?;

        Ok(())
    }

    fn git_remove_entry(&mut self, kind: &EntryKind, entry: &str) -> Result<(), JournalError> {
        let action = |journal: &Journal, repo: &Repository| {
            journal.git_index_remove_entry(repo, kind, entry)
        };

        self.git_index_entry(&action)?;

        Ok(())
    }

    fn git_commit(&mut self, message: &str) -> Result<(), JournalError> {
        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        git::commit(&repo, message)
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        self.refresh_from_repo(&repo);

        Ok(())
    }

    pub fn handle_event(&mut self, event: &KeyEvent) -> Result<Redraw, JournalError> {
        let action = self.input_handler.action(&self.mode, event)?;

        let page: Box<&mut dyn Navigable> = Box::new(&mut self.entries_page);

        match action {
            Action::AppExit => return Err(JournalError::Error(ErrorKind::AppExit)),
            Action::DisplayError(err) => return Err(err),

            Action::ExportEntry => {
                if let Some(entry) = page.select(&self.journal) {
                    let entry_kind = EntryKind::from(&self.entries_page.active_pane);
                    let entry_exporter = EntryExporter::new(&self.config, &entry_kind, &entry)?;
                    self.entry_exporter = Some(entry_exporter);
                    self.begin_export_entry()?;
                    self.mode = InputMode::ExportEntry(None);
                    return Ok(Redraw(false));
                }
            }

            Action::NameNewEntry => {
                let now = OffsetDateTime::now_local().map_err(|_| {
                    JournalError::chain(&[ErrorKind::NewEntry, ErrorKind::GetCurrentTime])
                })?;
                let default_name = now.format(DEFAULT_ENTRY_FILENAME_FORMAT).map_err(|_| {
                    JournalError::chain(&[ErrorKind::NewEntry, ErrorKind::FormatDateTime])
                })?;

                match self.entries_page.active_pane {
                    ActivePane::General => {
                        self.new_entry(&EntryKind::General, &default_name)?;
                    }
                    ActivePane::Narrative => {
                        let name_suffix = now
                            .format(NARRATIVE_ENTRY_FILENAME_SUFFIX_FORMAT)
                            .map_err(|_| {
                                JournalError::chain(&[
                                    ErrorKind::NewEntry,
                                    ErrorKind::FormatDateTime,
                                ])
                            })?;
                        let name_prefix = format!("{}{}", default_name, name_suffix);

                        self.input_handler.prepare_entry(
                            "Entry Name",
                            "New entry name ...",
                            Some(&name_prefix),
                        );

                        self.mode = InputMode::NewEntry(None);
                    }
                }
                return Ok(Redraw(true));
            }

            Action::CreateEntry(ref entry_name) => {
                let err = || {
                    Err(JournalError::chain(&[
                        ErrorKind::InvalidMode,
                        ErrorKind::NewEntry,
                        ErrorKind::TextEntryFailed,
                    ]))
                };
                self.mode = InputMode::Ready(None);
                // fixme need actial validation
                if entry_name.is_empty() {
                    return err();
                }
                self.new_entry(&EntryKind::Narrative, entry_name)?;

                return Ok(Redraw(true));
            }

            Action::ActivateSelectedEntry => {
                if let Some(entry) = page.select(&self.journal) {
                    let entry_kind = EntryKind::from(&self.entries_page.active_pane);
                    self.edit_existing_entry(&entry_kind, &entry)?;
                }
                return Ok(Redraw(true));
            }

            Action::Refresh => {
                self.refresh()?;
                return Ok(Redraw(true));
            }

            Action::NavigateNext => page.next_selection(),
            Action::NavigatePrevious => page.previous_selection(),
            Action::NavigatePageUp => page.pageup(self.height),
            Action::NavigatePageDown => page.pagedown(self.height),
            Action::NavigateFirst => page.home(),
            Action::NavigateLast => page.end(),
            Action::NavigateSwitchPane => page.switch_pane(),
            Action::NavigateBack => {
                self.mode = InputMode::Ready(None);
                return Ok(Redraw(true));
            }

            Action::GitStageFile => {
                if let Some(entry) = page.select(&self.journal) {
                    let entry_kind = EntryKind::from(&self.entries_page.active_pane);
                    self.git_add_entry(&entry_kind, &entry)?;
                }
            }

            Action::GitUnstageFile => {
                if let Some(entry) = page.select(&self.journal) {
                    let entry_kind = EntryKind::from(&self.entries_page.active_pane);
                    self.git_remove_entry(&entry_kind, &entry)?;
                }
            }

            Action::GitCommitMessage => {
                self.input_handler
                    .prepare_entry("Commit", "Commit message ...", None);
                self.mode = InputMode::Commit(None);
            }

            Action::GitPerformCommit(ref commit_message) => {
                let err = || {
                    Err(JournalError::chain(&[
                        ErrorKind::InvalidMode,
                        // ErrorKind::Commit,
                        ErrorKind::TextEntryFailed,
                    ]))
                };
                self.mode = InputMode::Ready(None);
                // fixme need actial validation
                if commit_message.is_empty() {
                    return err();
                }
                self.git_commit(commit_message)?;
                return Ok(Redraw(true));
            }

            Action::TextEntry(ref key_event) => {
                self.input_handler.handle_text_entry(key_event)?;
            }
            Action::ClearTextEntry => {
                self.input_handler.clear_text_entry()?;
                // fixme this is a weird workaround
                if let Some(mode) = match self.mode {
                    InputMode::NewEntry(_) => Some(InputMode::NewEntry(None)),
                    InputMode::Commit(_) => Some(InputMode::Commit(None)),
                    _ => None,
                } {
                    self.mode = mode;
                }
            }

            Action::NextMode(mode) => {
                self.mode = mode;
            }

            Action::NoOp => {}
        }

        Ok(Redraw(false))
    }

    pub fn tick(&mut self, tick_duration: u8) -> Result<Redraw, JournalError> {
        if let InputMode::ExportEntry(_) = self.mode
            && let Some(ref mut exporter) = self.entry_exporter
        {
            match exporter.tick(tick_duration) {
                Ok(export_complete) => {
                    if export_complete {
                        self.mode = InputMode::Ready(None);
                        self.entry_exporter = None;
                    }
                }
                Err(e) => {
                    self.mode = InputMode::Ready(None);
                    self.entry_exporter = None;
                    return Err(e);
                }
            }
        }
        Ok(Redraw(false))
    }

    pub fn error(&mut self, error: JournalError) {
        self.mode = InputMode::Error(error);
    }
}
