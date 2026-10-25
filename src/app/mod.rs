use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use git2::{ErrorCode, Repository};
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

mod entries_page;
mod pagination;

use crate::{
    app::entries_page::{ActivePane, EntriesPage},
    config::Config,
    errors::{ErrorKind, JournalError},
    journal::{EntryKind, Journal, edit::EntryEditor, export::EntryExporter},
    traits::{Drawable, Navigable},
    ui::centered_rect,
};

pub struct Redraw(pub bool);

pub struct EventHandled(pub bool);

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

        let journal = Journal::from_file(&config.entries_path(), &repo);
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

        self.journal = Journal::from_file(&self.config.entries_path(), repo);

        self.entries_page.update(&self.journal);

        Ok(file_changed)
    }

    fn new_entry(&mut self, kind: &EntryKind, new_entry: &str) -> Result<(), JournalError> {
        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        if let Ok(journal) = self.journal.with_new_entry(kind, new_entry, &repo) {
            self.journal = journal;
            let _ = self
                .journal
                .git_add(&repo, self.config.entries_filename())
                .map_err(|err| match err {
                    JournalError::Error(e) => JournalError::chain(&[ErrorKind::GitError, e]),
                });
        }

        let entry_editor = EntryEditor::new(&self.config)?;

        let created = entry_editor
            .create_entry(kind, new_entry)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::NewEntry, e]),
            })?;

        if created {
            self.journal
                .git_add_entry(&repo, kind, new_entry)
                .map_err(|err| match err {
                    JournalError::Error(e) => JournalError::chain(&[ErrorKind::NewEntry, e]),
                })?;
        }

        let changed = self
            .edit_entry(&repo, kind, new_entry)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::NewEntry, e]),
            })?;

        if !changed {
            // Handle when the changed file was not re-read on edit,
            // because we created it but didn't edit afterwards.
            self.journal = Journal::from_file(&self.config.entries_path(), &repo);
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

    fn git_add_entry(&mut self, kind: &EntryKind, entry: &str) -> Result<(), JournalError> {
        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        self.journal
            .git_add_entry(&repo, kind, entry)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::NewEntry, e]),
            })?;

        self.journal = Journal::from_file(&self.config.entries_path(), &repo);

        Ok(())
    }

    fn git_commit(&mut self, message: &str) -> Result<(), JournalError> {
        let repo = match Repository::open(&self.config.journal_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        let mut index = repo
            .index()
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        let parent_commit = match repo.revparse_single("HEAD") {
            Ok(obj) => Some(obj.into_commit().unwrap()),
            // First commit so no parent commit
            Err(e) if e.code() == ErrorCode::NotFound => None,
            Err(e) => return Err(JournalError::Error(ErrorKind::Wrapped(e.to_string()))),
        };

        let mut parents = Vec::new();
        if let Some(ref parent) = parent_commit {
            parents.push(parent);
        }

        let signature = repo
            .signature()
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        let commit_oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents[..],
            )
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        let _commit = repo
            .find_commit(commit_oid)
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        self.journal = Journal::from_file(&self.config.entries_path(), &repo);

        Ok(())
    }

    fn handle_standard_mode_event(&mut self, event: &KeyEvent) -> Result<Redraw, JournalError> {
        let page: Box<&mut dyn Navigable> = Box::new(&mut self.entries_page);
        match event {
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                ..
            } => match event.code {
                // C-x -> Exit
                KeyCode::Char('x') => {
                    return Err(JournalError::Error(ErrorKind::AppExit));
                }
                // C-e -> Export
                KeyCode::Char('e') => {
                    if let Some(entry) = page.select(&self.journal) {
                        let entry_kind = EntryKind::from(&self.entries_page.active_pane);
                        let entry_exporter = EntryExporter::new(&self.config, &entry_kind, &entry)?;
                        self.entry_exporter = Some(entry_exporter);
                        self.begin_export_entry()?;
                        self.mode = AppMode::ExportEntry;
                        return Ok(Redraw(false));
                    }
                }
                // C-n -> Next item
                KeyCode::Char('n') => page.next_selection(),
                // C-p -> Previous item
                KeyCode::Char('p') => page.previous_selection(),
                // C-v -> PgDn
                KeyCode::Char('v') => page.pagedown(self.height),
                _ => {}
            },
            KeyEvent {
                modifiers: KeyModifiers::ALT,
                ..
            } => match event.code {
                // M-v -> PgUp
                KeyCode::Char('v') => page.pageup(self.height),
                // M-n -> New Entry
                KeyCode::Char('n') => {
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
                // M-s for stage file to index
                KeyCode::Char('s') => {
                    if let Some(entry) = page.select(&self.journal) {
                        let entry_kind = EntryKind::from(&self.entries_page.active_pane);
                        self.git_add_entry(&entry_kind, &entry)?;
                    }
                }
                // M-c for commit staged changes
                KeyCode::Char('c') => {
                    let text_entry = ratin::InputField::new()
                        .with_placeholder("Commit message ...")
                        .with_max_length(120);
                    self.text_entry = Some(text_entry);
                    self.mode = AppMode::Commit;
                }
                _ => {}
            },
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                ..
            } => match event.code {
                KeyCode::Enter => {
                    if let Some(entry) = page.select(&self.journal) {
                        let entry_kind = EntryKind::from(&self.entries_page.active_pane);
                        self.edit_existing_entry(&entry_kind, &entry)?;
                    }
                    return Ok(Redraw(true));
                }
                KeyCode::Home => page.home(),
                KeyCode::End => page.end(),
                KeyCode::PageUp => page.pageup(self.height),
                KeyCode::PageDown => page.pagedown(self.height),
                KeyCode::Down => page.next_selection(),
                KeyCode::Up => page.previous_selection(),
                KeyCode::Tab => page.switch_pane(),
                _ => {}
            },
            _ => {}
        };
        Ok(Redraw(false))
    }

    fn get_entered_text(&mut self) -> Result<String, JournalError> {
        if let Some(ref mut text_entry) = self.text_entry {
            Ok(text_entry.get_text().to_owned())
        } else {
            Err(JournalError::Error(ErrorKind::TextEntryFailed))
        }
    }

    fn handle_text_entry_event(
        &mut self,
        event: &KeyEvent,
    ) -> Result<(EventHandled, Redraw), JournalError> {
        let mut send_text_entry = || -> Result<(EventHandled, Redraw), JournalError> {
            if let Some(ref mut text_entry) = self.text_entry {
                text_entry.handle_key(event.code);
            } else {
                return Err(JournalError::chain(&[
                    ErrorKind::InvalidMode,
                    ErrorKind::NewEntry,
                    ErrorKind::TextEntryFailed,
                ]));
            }
            Ok((EventHandled(true), Redraw(false)))
        };
        match event {
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                ..
            } => match event.code {
                KeyCode::Home
                    | KeyCode::End
                    | KeyCode::Delete
                    | KeyCode::Backspace
                    | KeyCode::Char(_) => {
                    return send_text_entry();
                }
                _ => {}
            },
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                ..
            } => if let KeyCode::Char(_) = event.code {
                return send_text_entry();
            }
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                let go_back = if event.code == KeyCode::Char('g') {
                    true
                } else if event.code == KeyCode::Char('x') {
                    if let Some(ref mut text_entry) = self.text_entry {
                        if text_entry.is_empty() {
                            true
                        } else {
                            text_entry.clear();
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };
                if go_back {
                    self.mode = AppMode::Standard;
                    self.text_entry = None;
                    return Ok((EventHandled(true), Redraw(true)));
                }
            }
            _ => {}
        }
        Ok((EventHandled(false), Redraw(false)))
    }

    fn handle_commit_mode_event(&mut self, event: &KeyEvent) -> Result<Redraw, JournalError> {
        if let KeyEvent {
            modifiers: KeyModifiers::NONE,
            ..
        } = event
            && event.code == KeyCode::Enter
        {
            let err = || {
                Err(JournalError::chain(&[
                    ErrorKind::InvalidMode,
                    // ErrorKind::Commit,
                    ErrorKind::TextEntryFailed,
                ]))
            };
            self.mode = AppMode::Standard;
            let commit_message = self.get_entered_text().map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[
                    ErrorKind::InvalidMode,
                    // ErrorKind::Commit,
                    e,
                ]),
            })?;
            // fixme need actial validation
            if commit_message.is_empty() {
                return err();
            }
            self.git_commit(&commit_message)?;
            self.text_entry = None;
            return Ok(Redraw(true));
        }
        Ok(Redraw(false))
    }

    fn handle_new_mode_event(&mut self, event: &KeyEvent) -> Result<Redraw, JournalError> {
        if let KeyEvent {
            modifiers: KeyModifiers::NONE,
            ..
        } = event
            && event.code == KeyCode::Enter
        {
            let err = || {
                Err(JournalError::chain(&[
                    ErrorKind::InvalidMode,
                    ErrorKind::NewEntry,
                    ErrorKind::TextEntryFailed,
                ]))
            };
            self.mode = AppMode::Standard;
            let entry_name = self.get_entered_text().map_err(|err| match err {
                JournalError::Error(e) => {
                    JournalError::chain(&[ErrorKind::InvalidMode, ErrorKind::NewEntry, e])
                }
            })?;
            // fixme need actial validation
            if entry_name.is_empty() {
                return err();
            }
            self.new_entry(&EntryKind::Narrative, &entry_name)?;
            self.text_entry = None;
            return Ok(Redraw(true));
        }
        Ok(Redraw(false))
    }

    pub fn handle_event(&mut self, event: &KeyEvent) -> Result<Redraw, JournalError> {
        match self.mode {
            AppMode::Standard => {
                return self.handle_standard_mode_event(event);
            }
            AppMode::NewEntry => {
                let (handled, redraw) = self.handle_text_entry_event(event)?;
                if !handled.0 {
                    return self.handle_new_mode_event(event);
                }
                return Ok(redraw);
            }
            AppMode::Commit => {
                let (handled, redraw) = self.handle_text_entry_event(event)?;
                if !handled.0 {
                    return self.handle_commit_mode_event(event);
                }
                return Ok(redraw);
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
                    self.mode = AppMode::Standard;
                    return Ok(Redraw(true));
                }
                _ => {}
            },
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
