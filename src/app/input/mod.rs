use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Row, Table},
};
use ratatui_textarea::TextArea;

use self::{
    input_chain::InputChain,
    key_map::{KeyMap, KeyMapAction},
};

use super::Action;
use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
    ui::centered_rect,
};

pub(super) mod input_chain;
mod key_map;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum InputMode {
    Ready(Option<InputChain>),
    NewEntry(Option<InputChain>),
    ExportEntry(Option<InputChain>),
    ExportEntryComplete(Option<InputChain>),
    Commit(Option<InputChain>),
    Error(JournalError),
}

pub(super) struct InputHandler<'a> {
    key_map: KeyMap,
    text_entry: Option<TextArea<'a>>,
}

impl<'a> InputHandler<'a> {
    pub(super) fn new(config: &Config) -> Result<Self, JournalError> {
        let key_map = KeyMap::new(&config.key_map)?;
        Ok(Self {
            key_map,
            text_entry: None,
        })
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, block: Block, mode: &InputMode) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)]);

        let [hint_area, current_area] = layout.areas(area);

        let mut rows: Vec<Row> = Vec::new();

        let widths = [
            Constraint::Min(5),
            Constraint::Min(5),
            Constraint::Min(5),
            Constraint::Min(5),
            Constraint::Min(5),
        ];

        let bindings = &self.key_map.keybindings()[..];
        for row in bindings.chunks(widths.len()) {
            let mut row_vec: Vec<String> = Vec::new();
            for item in row.iter() {
                row_vec.push(item.to_string());
            }
            rows.push(Row::new(row_vec));
        }

        let table = Table::default().rows(rows).widths(widths).block(block);
        frame.render_widget(table, hint_area);

        if let Some(input) = match mode {
            InputMode::Ready(input) => input,
            InputMode::NewEntry(input) => input,
            InputMode::ExportEntry(input) => input,
            InputMode::Commit(input) => input,
            _ => &None,
        } {
            let current_entry = Line::from(Span::from(input.to_string()));
            frame.render_widget(current_entry, current_area);
        }
    }

    fn get_entered_text(&self) -> Result<String, JournalError> {
        if let Some(ref text_entry) = self.text_entry {
            let text = &text_entry.lines()[0];
            Ok(text.to_owned())
        } else {
            Err(JournalError::Error(ErrorKind::TextEntryFailed))
        }
    }

    fn handle_chained_input(&self, input: &InputChain) -> Action {
        match self.key_map.action(input) {
            KeyMapAction::Action(action) => match action {
                Action::NoOp => Action::NextMode(InputMode::Ready(None)),
                _ => action,
            },
            KeyMapAction::Input(_) => Action::NextMode(InputMode::Ready(Some(input.clone()))),
        }
    }

    fn handle_text_entry_event(
        &self,
        mode: &InputMode,
        event: &KeyEvent,
    ) -> Result<Option<Action>, JournalError> {
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
                            InputMode::NewEntry(_) => return Ok(Some(Action::CreateEntry(text))),
                            InputMode::Commit(_) => {
                                return Ok(Some(Action::GitPerformCommit(text)));
                            }
                            _ => {}
                        }
                    }
                    Err(e) => return Ok(Some(Action::DisplayError(e))),
                }
            }
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Home | KeyCode::End | KeyCode::Delete | KeyCode::Backspace,
                ..
            } => return Ok(Some(Action::TextEntry(*event))),
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                code: KeyCode::Char(_),
                ..
            } => {
                return Ok(Some(Action::TextEntry(*event)));
            }
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Char(_),
                ..
            } => {
                return Ok(Some(Action::TextEntry(*event)));
            }
            _ => {}
        }
        Ok(None)
    }

    pub(super) fn action(
        &self,
        mode: &InputMode,
        event: &KeyEvent,
    ) -> Result<Action, JournalError> {
        let input_chain = |maybe_input: &Option<InputChain>| -> InputChain {
            if let Some(input) = maybe_input {
                input.chain(event)
            } else {
                InputChain::from(event)
            }
        };

        match mode {
            InputMode::Ready(maybe_input) => {
                let input = input_chain(&maybe_input);
                return Ok(self.handle_chained_input(&input));
            }
            InputMode::NewEntry(maybe_input) | InputMode::Commit(maybe_input) => {
                let input = if let Some(input) = maybe_input {
                    input.chain(event)
                } else {
                    if let Some(action) = self.handle_text_entry_event(mode, event)? {
                        return Ok(action);
                    }
                    InputChain::from(event)
                };

                let action = match self.key_map.action(&input) {
                    KeyMapAction::Action(action) => match action {
                        Action::NavigateBack => action,
                        Action::AppExit => {
                            let text = self.get_entered_text()?;
                            if text.is_empty() {
                                Action::NavigateBack
                            } else {
                                Action::ClearTextEntry
                            }
                        }
                        Action::NoOp => match mode {
                            InputMode::NewEntry(_) => Action::NextMode(InputMode::NewEntry(None)),
                            InputMode::Commit(_) => Action::NextMode(InputMode::Commit(None)),
                            _ => Action::NoOp,
                        },
                        _ => Action::NoOp,
                    },
                    KeyMapAction::Input(_) => match mode {
                        InputMode::NewEntry(_) => {
                            Action::NextMode(InputMode::NewEntry(Some(input.clone())))
                        }
                        InputMode::Commit(_) => {
                            Action::NextMode(InputMode::Commit(Some(input.clone())))
                        }
                        _ => Action::NoOp,
                    },
                };

                return Ok(action);
            }
            InputMode::ExportEntry(_) => {}
            InputMode::ExportEntryComplete(maybe_input) => {
                let input = input_chain(&maybe_input);
                return match self.key_map.action(&input) {
                    KeyMapAction::Action(action) => match action {
                        Action::NavigateBack | Action::ActivateSelectedEntry | Action::AppExit => {
                            Ok(Action::NextMode(InputMode::Ready(None)))
                        }
                        _ => Ok(Action::NoOp),
                    },
                    _ => Ok(Action::NoOp),
                };
            }
            InputMode::Error(_) => return Ok(Action::NavigateBack),
        }
        Ok(Action::NoOp)
    }

    pub(super) fn prepare_entry(&mut self, title: &str, placeholder: &str, default: Option<&str>) {
        let mut text_entry = TextArea::default();
        text_entry.set_cursor_line_style(Style::default());
        text_entry.set_placeholder_text(placeholder);
        // text_entry.set_max_histories(0);
        if let Some(default_text) = default {
            text_entry.insert_str(default_text);
        }

        let edit_block = Block::default()
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .title(Span::styled(
                format!(" {} ", title),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ))
            .style(Style::default());
        text_entry.set_block(edit_block);

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

    pub(super) fn handle_text_entry(&mut self, event: &KeyEvent) -> Result<(), JournalError> {
        if let Some(ref mut text_entry) = self.text_entry {
            text_entry.input(*event);
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
            frame.render_widget(&*text_entry, entry_area);
        }
    }
}
