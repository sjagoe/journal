use std::{collections::HashMap, str::FromStr};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{Frame, layout::Constraint, widgets::Clear};
use regex::Regex;

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

#[derive(Default, Hash, PartialEq, Eq)]
pub(super) struct InputChain {
    prior_inputs: Vec<KeyEvent>,
}

impl InputChain {
    fn chain(&self, next: &KeyEvent) -> InputChain {
        let mut inputs: Vec<KeyEvent> = self.prior_inputs.to_owned();
        inputs.push(*next);
        InputChain {
            prior_inputs: inputs,
        }
    }
}

impl FromStr for InputChain {
    type Err = JournalError;

    fn from_str(value: &str) -> Result<InputChain, Self::Err> {
        let mut events: Vec<KeyEvent> = Vec::new();

        for item in value.split(" ") {
            let event = parse_keymap_str(item)?;
            events.push(event);
        }

        let chain = InputChain {
            prior_inputs: events,
        };
        Ok(chain)
    }
}

impl From<&KeyEvent> for InputChain {
    fn from(value: &KeyEvent) -> InputChain {
        InputChain {
            prior_inputs: vec![*value],
        }
    }
}


type KeyMapType = HashMap<InputChain, Action>;

struct KeyMap {
    map: KeyMapType,
}

fn parse_keymap_str(value: &str) -> Result<KeyEvent, JournalError> {
    let make_event = |modifiers: KeyModifiers, code: KeyCode| KeyEvent {
        modifiers,
        code,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };

    if let Some(keycode) = match value {
        "Enter" | "Return" => Some(KeyCode::Enter),
        "End" => Some(KeyCode::End),
        "Home" => Some(KeyCode::Home),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),
        "Up" => Some(KeyCode::Up),
        "Down" => Some(KeyCode::Down),
        "Tab" => Some(KeyCode::Tab),
        _ => None,
    } {
        return Ok(make_event(KeyModifiers::NONE, keycode));
    }

    let keymap_parser = Regex::new(r"^((?<mod>C|M)-)?(?<key>[a-z])$").unwrap();

    if let Some(captures) = keymap_parser.captures(value) {
        let modifier = if let Some(modifier) = captures.name("mod") {
            match modifier.as_str() {
                "C" => KeyModifiers::CONTROL,
                "M" => KeyModifiers::ALT,
                _ => {
                    let msg = format!("Unable to parse keymap modifier: {:?}", value);
                    return Err(JournalError::Error(ErrorKind::Wrapped(msg)));
                }
            }
        } else {
            KeyModifiers::NONE
        };
        let character = if let Some(character) = captures.name("key") {
            character.as_str().chars().next().unwrap()
        } else {
            let msg = format!("Unable to parse keymap character: {:?}", value);
            return Err(JournalError::Error(ErrorKind::Wrapped(msg)))
        };
        let event = make_event(
            modifier,
            KeyCode::Char(character),
        );
        Ok(event)
    } else {
        let msg = format!("Unable to parse keymap: {:?}", value);
        Err(JournalError::Error(ErrorKind::Wrapped(msg)))
    }
}

impl KeyMap {
    fn new(mapconfig: &HashMap<String, String>) -> Result<Self, JournalError> {
        let mut keys: KeyMapType = HashMap::new();

        for (keyseq, action_name) in mapconfig.iter() {
            let input = InputChain::from_str(&keyseq)?;
            let action = Action::from_str(&action_name)?;
            keys.insert(input, action);
        }

        let map = KeyMap {
            map: keys,
        };
        Ok(map)
    }

    pub(self) fn action(&self, input: &InputChain) -> Result<Action, JournalError> {
        if let Some(action) = self.map.get(input) {
            Ok(action.clone())
        } else {
            Ok(Action::NoOp)
        }
    }
}

const DEFAULT_KEYS: [(&str, Action); 19] = [
    ("C-x", Action::AppExit),
    ("C-e", Action::ExportEntry),
    ("C-n", Action::NavigateNext),
    ("C-p", Action::NavigatePrevious),
    ("C-v", Action::NavigatePageDown),
    ("M-v", Action::NavigatePageUp),
    ("M-n", Action::NameNewEntry),
    ("M-s", Action::GitStageFile),
    ("M-u", Action::GitUnstageFile),
    ("M-c", Action::GitCommitMessage),
    ("g", Action::Refresh),
    ("Enter", Action::ActivateSelectedEntry),
    ("Home", Action::NavigateFirst),
    ("End", Action::NavigateLast),
    ("PageUp", Action::NavigatePageUp),
    ("PageDown", Action::NavigatePageDown),
    ("Up", Action::NavigatePrevious),
    ("Down", Action::NavigateNext),
    ("Tab", Action::NavigateSwitchPane),
];

impl Default for KeyMap {
    fn default() -> Self {
        let mut keys: KeyMapType = HashMap::new();

        for (keyseq, action) in DEFAULT_KEYS {
            let input = InputChain::from_str(&keyseq).unwrap();
            keys.insert(input, action);
        }

        KeyMap {
            map: keys,
        }
    }
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
    key_map: KeyMap,
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
        let input = InputChain::from(event);
        return self.key_map.action(&input);
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
