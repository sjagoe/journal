use std::{
    collections::HashMap,
    fmt::{self, Display},
    str::FromStr,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, Row, Table},
};

use self::input_chain::InputChain;
use super::Action;
use crate::{
    errors::{ErrorKind, JournalError},
    ui::centered_rect,
};

pub(super) mod input_chain;

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

#[derive(Debug, PartialEq, Eq, Hash)]
enum KeyMapAction {
    Action(Action),
    Input(InputChain),
}

impl KeyMapAction {
    fn to_owned(&self) -> KeyMapAction {
        match self {
            KeyMapAction::Action(action) => KeyMapAction::Action(action.clone()),
            KeyMapAction::Input(input) => KeyMapAction::Input(input.to_owned()),
        }
    }
}

impl Display for KeyMapAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KeyMapAction::Action(action) => write!(f, "{:?}", action),
            KeyMapAction::Input(input) => write!(f, "{}", input),
        }
    }
}

type KeyMapType = HashMap<InputChain, KeyMapAction>;
type KeyMapReverseType = HashMap<KeyMapAction, InputChain>;

#[derive(Debug)]
struct KeyMap {
    map: KeyMapType,
    reverse: KeyMapReverseType,
}

impl KeyMap {
    pub fn keybindings(&self) -> Vec<String> {
        let mut bindings: Vec<String> = Vec::new();

        for action in [
            Action::AppExit,
            Action::ExportEntry,
            Action::NameNewEntry,
            Action::ActivateSelectedEntry,
            Action::Refresh,
            Action::NavigateNext,
            Action::NavigatePrevious,
            Action::NavigatePageUp,
            Action::NavigatePageDown,
            Action::NavigateFirst,
            Action::NavigateLast,
            Action::NavigateSwitchPane,
            Action::NavigateBack,
            Action::GitStageFile,
            Action::GitUnstageFile,
            Action::GitCommitMessage,
        ] {
            let key_action = KeyMapAction::Action(action.clone());
            if let Some(binding) = self.reverse.get(&key_action) {
                let binding_str = binding.to_string();
                bindings.push(format!("{}: {}", binding_str, action));
            }
        }

        bindings
    }

    fn add_keymap(
        keys: &mut KeyMapType,
        reverse: &mut KeyMapReverseType,
        input: &InputChain,
        action: &Action,
    ) {
        for (input, next_input) in input.get_chain().iter() {
            if let Some(next_input) = next_input {
                let next = KeyMapAction::Input(next_input.to_owned());
                keys.insert(input.to_owned(), next);
            } else {
                let map_action = KeyMapAction::Action(action.clone());
                keys.insert(input.to_owned(), map_action.to_owned());
                reverse
                    .entry(map_action)
                    .or_insert_with(|| input.to_owned());
            }
        }
    }

    #[allow(unused)]
    fn new(mapconfig: &HashMap<String, String>) -> Result<Self, JournalError> {
        let mut keys: KeyMapType = HashMap::new();
        let mut reverse: KeyMapReverseType = HashMap::new();

        for (keyseq, action_name) in mapconfig.iter() {
            let input_chain = InputChain::from_str(keyseq)?;
            let action = Action::from_str(action_name)?;
            Self::add_keymap(&mut keys, &mut reverse, &input_chain, &action);
        }

        let map = KeyMap { map: keys, reverse };
        Ok(map)
    }

    pub(self) fn action(&self, input: &InputChain) -> KeyMapAction {
        if let Some(action) = self.map.get(input) {
            action.to_owned()
        } else {
            KeyMapAction::Action(Action::NoOp)
        }
    }
}

impl Display for KeyMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lines: Vec<String> = Vec::new();
        for (input, action) in self.map.iter() {
            let line = format!("{}: {}", input, action);
            lines.push(line);
        }
        write!(f, "{}", lines.join("\n"))
    }
}

const DEFAULT_KEYS: [(&str, Action); 20] = [
    ("C-x", Action::AppExit),
    ("C-e", Action::ExportEntry),
    ("C-g", Action::NavigateBack),
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
        let mut reverse: KeyMapReverseType = HashMap::new();

        for (keyseq, action) in DEFAULT_KEYS {
            let input_chain = InputChain::from_str(keyseq).unwrap();
            Self::add_keymap(&mut keys, &mut reverse, &input_chain, &action);
        }

        KeyMap { map: keys, reverse }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum InputMode {
    Ready(Option<InputChain>),
    NewEntry(Option<InputChain>),
    ExportEntry(Option<InputChain>),
    Commit(Option<InputChain>),
    Error(JournalError),
}

#[derive(Default)]
pub(super) struct InputHandler {
    key_map: KeyMap,
    text_entry: Option<ratin::InputField>,
}

impl InputHandler {
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
            Ok(text_entry.get_text().to_owned())
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
            } => return Ok(Some(Action::TextEntry(event.code))),
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                code: KeyCode::Char(_),
                ..
            } => {
                return Ok(Some(Action::TextEntry(event.code)));
            }
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Char(_),
                ..
            } => {
                return Ok(Some(Action::TextEntry(event.code)));
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
        match mode {
            InputMode::Ready(maybe_input) => {
                let input = if let Some(input) = maybe_input {
                    input.chain(event)
                } else {
                    InputChain::from(event)
                };
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
            InputMode::Error(_) => return Ok(Action::NavigateBack),
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
