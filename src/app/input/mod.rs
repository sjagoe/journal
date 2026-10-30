use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, layout::Constraint, widgets::Clear};

use crate::{
    errors::{ErrorKind, JournalError},
    ui::centered_rect,
};

use super::Action;

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

pub(super) struct InputChain {
    prior_inputs: Vec<KeyEvent>,
}

pub(super) enum InputMode {
    Ready,
    ChainedInput(InputChain),
    NewEntry,
    ExportEntry,
    Commit,
    Error(JournalError),
}

#[derive(Default)]
pub(super) struct InputHandler {
    text_entry: Option<ratin::InputField>,
}

impl InputHandler {
    fn get_entered_text(&self) -> Result<String, JournalError> {
        if let Some(ref text_entry) = self.text_entry {
            Ok(text_entry.get_text().to_owned())
        } else {
            Err(JournalError::Error(ErrorKind::TextEntryFailed))
        }
    }

    fn handle_standard_mode_event(&self, event: &KeyEvent) -> Result<Action, JournalError> {
        let action = match event {
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                ..
            } => match event.code {
                // C-x -> Exit
                KeyCode::Char('x') => Action::AppExit,
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

    pub(super) fn action(
        &self,
        mode: &InputMode,
        event: &KeyEvent,
    ) -> Result<Action, JournalError> {
        match mode {
            InputMode::Ready => {
                return self.handle_standard_mode_event(event);
            }
            InputMode::NewEntry | InputMode::Commit => {
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
                                    JournalError::Error(e) => JournalError::chain(&[
                                        ErrorKind::InvalidMode,
                                        ErrorKind::NewEntry,
                                        e,
                                    ]),
                                })?;

                                match mode {
                                    InputMode::NewEntry => return Ok(Action::CreateEntry(text)),
                                    InputMode::Commit => return Ok(Action::GitPerformCommit(text)),
                                    _ => {}
                                }
                            }
                            Err(e) => return Ok(Action::DisplayError(e)),
                        }
                    }
                    KeyEvent {
                        modifiers: KeyModifiers::NONE,
                        code: KeyCode::Home | KeyCode::End | KeyCode::Delete | KeyCode::Backspace,
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
            InputMode::ExportEntry => {}
            InputMode::Error(_) => match event {
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
            InputMode::ChainedInput(_) => {}
        }
        Ok(Action::NoOp)
    }

    pub(super) fn prepare_entry(&mut self, placeholder: &str, default: Option<&str>) {
        let mut text_entry = ratin::InputField::new()
            .with_placeholder(placeholder)
            .with_max_length(50);
        if let Some(default_text) = default {
            text_entry.set_text(default_text);
        }
        self.text_entry = Some(text_entry);
    }

    pub(super) fn clear_text_entry(&mut self) -> Result<(), JournalError> {
        if let Some(ref mut text_entry) = self.text_entry {
            text_entry.clear();
        } else {
            return Err(JournalError::chain(&[
                ErrorKind::InvalidMode,
                ErrorKind::NewEntry,
                ErrorKind::TextEntryFailed,
            ]));
        }
        Ok(())
    }

    pub(super) fn handle_text_entry(&mut self, keycode: &KeyCode) -> Result<(), JournalError> {
        if let Some(ref mut text_entry) = self.text_entry {
            text_entry.handle_key(*keycode);
        } else {
            return Err(JournalError::chain(&[
                ErrorKind::InvalidMode,
                ErrorKind::NewEntry,
                ErrorKind::TextEntryFailed,
            ]));
        }
        Ok(())
    }

    pub(super) fn render_input(&mut self, frame: &mut Frame) {
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
}
