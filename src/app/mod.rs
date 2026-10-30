use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use git2::Repository;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
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
mod pagination;

use crate::{
    app::{actions::Action, entries_page::{ActivePane, EntriesPage}},
    config::Config,
    errors::{ErrorKind, JournalError},
    journal::{EntryKind, Journal, edit::EntryEditor, export::EntryExporter, git},
    traits::{Drawable, Navigable},
    ui::centered_rect,
};

pub struct Redraw(pub bool);

pub struct EventHandled(pub bool);

static DEFAULT_ENTRY_FILENAME_FORMAT: StaticFormatDescription =
    format_description!("[year]-[month]");
static NARRATIVE_ENTRY_FILENAME_SUFFIX_FORMAT: StaticFormatDescription =
    format_description!("-[day]_");

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

#[derive(Debug)]
enum AppMode {
    Standard,
    NewEntry,
    ExportEntry,
    Commit,
    Error(JournalError),
}

pub struct App {
    config: Config,
    journal: Journal,
    entries_page: EntriesPage,
    height: u16,
    mode: AppMode,
    text_entry: Option<ratin::InputField>,
    entry_exporter: Option<EntryExporter>,
}

impl App {
    pub fn new(config: &Config) -> Result<App, JournalError> {
        let repo = match Repository::open(&config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        let journal = Journal::from_file(&config.journal_path, &repo);
        let entries_page = EntriesPage::new(&journal);
        let app = App {
            config: config.clone(),
            journal,
            entries_page,
            height: 0,
            mode: AppMode::Standard,
            text_entry: None,
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

    fn draw_text_input(&mut self, frame: &mut Frame) {
        if let Some(ref mut text_entry) = self.text_entry {
            let entry_area = centered_rect(
                Constraint::Percentage(60),
                Constraint::Length(3),
                frame.area(),
            );
            frame.render_widget(Clear, entry_area);
            text_entry.render(frame, entry_area);
        }
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

        match self.mode {
            AppMode::Error(ref error) => self.display_error(frame, error),
            AppMode::NewEntry | AppMode::Commit => self.draw_text_input(frame),
            AppMode::ExportEntry => self.draw_export_progress(frame),
            _ => {}
        }

        if let AppMode::Error(ref error) = self.mode {
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

    fn handle_standard_mode_event(&self, event: &KeyEvent) -> Result<Action, JournalError> {
        let action = match event {
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                ..
            } => match event.code {
                // C-x -> Exit
                KeyCode::Char('x') => {
                    Action::AppExit
                }
                // C-e -> Export
                KeyCode::Char('e') => Action::ExportEntry,

                // C-n -> Next item
                KeyCode::Char('n') => Action::NavigateNext,
                // C-p -> Previous item
                KeyCode::Char('p') => Action::NavigatePrevious,
                // C-v -> PgDn
                KeyCode::Char('v') => Action::NavigatePageDown,
                _ => Action::NoOp,
            },
            KeyEvent {
                modifiers: KeyModifiers::ALT,
                ..
            } => match event.code {
                // M-v -> PgUp
                KeyCode::Char('v') => Action::NavigatePageUp,

                // M-n -> New Entry
                KeyCode::Char('n') => Action::NameNewEntry,

                // M-s for stage file to index
                KeyCode::Char('s') => Action::GitStageFile,

                // M-u for unstage file to index
                KeyCode::Char('u') => Action::GitUnstageFile,

                // M-c for commit staged changes
                KeyCode::Char('c') => Action::GitCommitMessage,

                _ => Action::NoOp,
            },
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                ..
            } => match event.code {
                KeyCode::Enter => Action::ActivateSelectedEntry,

                KeyCode::Home => Action::NavigateFirst,
                KeyCode::End => Action::NavigateLast,
                KeyCode::PageUp => Action::NavigatePageUp,
                KeyCode::PageDown => Action::NavigatePageDown,
                KeyCode::Down => Action::NavigateNext,
                KeyCode::Up => Action::NavigatePrevious,
                KeyCode::Tab => Action::NavigateSwitchPane,
                // Pressing g wil refresh the view from repository
                KeyCode::Char('g') => Action::Refresh,
                _ => Action::NoOp,
            },
            _ => Action::NoOp,
        };
        Ok(action)
    }

    fn get_entered_text(&self) -> Result<String, JournalError> {
        if let Some(ref text_entry) = self.text_entry {
            Ok(text_entry.get_text().to_owned())
        } else {
            Err(JournalError::Error(ErrorKind::TextEntryFailed))
        }
    }

    fn event_to_action(&self, event: &KeyEvent) -> Result<Action, JournalError> {
        match self.mode {
            AppMode::Standard => {
                return self.handle_standard_mode_event(event);
            }
            AppMode::NewEntry | AppMode::Commit => {
                match event {
                    KeyEvent {
                        modifiers: KeyModifiers::NONE,
                        code: KeyCode::Enter,
                        ..
                    } => {
                        match self.get_entered_text() {
                            Ok(text) => {
                                // fixme validation for both input kinds
                                // Missing ErrorKind::Commit
                                let _ = self.get_entered_text().map_err(|err| match err {
                                    JournalError::Error(e) => {
                                        JournalError::chain(&[ErrorKind::InvalidMode, ErrorKind::NewEntry, e])
                                    }
                                })?;

                                match self.mode {
                                    AppMode::NewEntry => return Ok(Action::CreateEntry(text)),
                                    AppMode::Commit => return Ok(Action::GitPerformCommit(text)),
                                    _ => {}
                                }
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    KeyEvent {
                        modifiers: KeyModifiers::NONE,
                        code: KeyCode::Home
                            | KeyCode::End
                            | KeyCode::Delete
                            | KeyCode::Backspace,
                        ..
                    } => return Ok(Action::TextEntry(event.code)),
                    KeyEvent {
                        modifiers: KeyModifiers::SHIFT,
                        code: KeyCode::Char(_),
                        ..
                    } => {
                        return Ok(Action::TextEntry(event.code));
                    }
                    KeyEvent {
                        modifiers: KeyModifiers::NONE,
                        code: KeyCode::Char(_),
                        ..
                    } => {
                        return Ok(Action::TextEntry(event.code));
                    }
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char('g'),
                        ..
                    } => return Ok(Action::NavigateBack),
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char('x'),
                        ..
                    } => {
                        let text = self.get_entered_text()?;
                        if text.is_empty() {
                            return Ok(Action::NavigateBack);
                        } else {
                            return Ok(Action::ClearTextEntry);
                        }
                    }
                    _ => {}
                }
            }
            AppMode::ExportEntry => {}
            AppMode::Error(_) => match event {
                KeyEvent {
                    modifiers: KeyModifiers::NONE,
                    code: KeyCode::Enter,
                    ..
                }
                | KeyEvent {
                    modifiers: KeyModifiers::CONTROL,
                    code: KeyCode::Char('g') | KeyCode::Char('x'),
                    ..
                } => {
                    return Ok(Action::NavigateBack);
                }
                _ => {}
            },
        }
        Ok(Action::NoOp)
    }

    pub fn handle_event(&mut self, event: &KeyEvent) -> Result<Redraw, JournalError> {
        let action = self.event_to_action(event)?;

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
                    self.mode = AppMode::ExportEntry;
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
                        let name_suffix = now.format(NARRATIVE_ENTRY_FILENAME_SUFFIX_FORMAT).map_err(|_| {
                            JournalError::chain(&[ErrorKind::NewEntry, ErrorKind::FormatDateTime])
                        })?;
                        let name_prefix = format!("{}{}", &default_name, &name_suffix);

                        let mut text_entry = ratin::InputField::new()
                            .with_placeholder("New entry name ...")
                            .with_max_length(50);
                        text_entry.set_text(&name_prefix);
                        self.text_entry = Some(text_entry);
                        self.mode = AppMode::NewEntry;
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
                self.mode = AppMode::Standard;
                // fixme need actial validation
                if entry_name.is_empty() {
                    return err();
                }
                self.new_entry(&EntryKind::Narrative, &entry_name)?;
                self.text_entry = None;
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
                self.mode = AppMode::Standard;
                self.text_entry = None;
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
                let text_entry = ratin::InputField::new()
                    .with_placeholder("Commit message ...")
                    .with_max_length(120);
                self.text_entry = Some(text_entry);
                self.mode = AppMode::Commit;
            }

            Action::GitPerformCommit(ref commit_message) => {
                let err = || {
                    Err(JournalError::chain(&[
                        ErrorKind::InvalidMode,
                        // ErrorKind::Commit,
                        ErrorKind::TextEntryFailed,
                    ]))
                };
                self.mode = AppMode::Standard;
                // fixme need actial validation
                if commit_message.is_empty() {
                    return err();
                }
                self.git_commit(&commit_message)?;
                self.text_entry = None;
                return Ok(Redraw(true));
            }

            Action::TextEntry(ref keycode) => {
                if let Some(ref mut text_entry) = self.text_entry {
                    text_entry.handle_key(*keycode);
                } else {
                    return Err(JournalError::chain(&[
                        ErrorKind::InvalidMode,
                        ErrorKind::NewEntry,
                        ErrorKind::TextEntryFailed,
                    ]));
                }
            }
            Action::ClearTextEntry => {
                if let Some(ref mut text_entry) = self.text_entry {
                    text_entry.clear();
                } else {
                    return Err(JournalError::chain(&[
                        ErrorKind::InvalidMode,
                        ErrorKind::NewEntry,
                        ErrorKind::TextEntryFailed,
                    ]));
                }
            }

            Action::NoOp => {}
        }

        Ok(Redraw(false))
    }

    pub fn tick(&mut self, tick_duration: u8) -> Result<Redraw, JournalError> {
        if let AppMode::ExportEntry = self.mode
            && let Some(ref mut exporter) = self.entry_exporter
        {
            match exporter.tick(tick_duration) {
                Ok(export_complete) => {
                    if export_complete {
                        self.mode = AppMode::Standard;
                        self.entry_exporter = None;
                    }
                }
                Err(e) => {
                    self.mode = AppMode::Standard;
                    self.entry_exporter = None;
                    return Err(e);
                }
            }
        }
        Ok(Redraw(false))
    }

    pub fn error(&mut self, error: JournalError) {
        self.mode = AppMode::Error(error);
    }
}
