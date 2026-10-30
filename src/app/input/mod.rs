use std::{cmp::Ordering, collections::HashMap, str::FromStr};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, Row, Table},
};
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

#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub(super) struct InputChain {
    prior_inputs: Vec<KeyEvent>,
}

impl PartialOrd for InputChain {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.to_string().cmp(&other.to_string()))
    }
}

impl Ord for InputChain {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl ToString for InputChain {
    fn to_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for input in self.prior_inputs.iter() {
            let mut input_str: Vec<String> = Vec::new();
            match input.modifiers {
                KeyModifiers::CONTROL => input_str.push("C-".to_string()),
                KeyModifiers::ALT => input_str.push("M-".to_string()),
                _ => {}
            }
            match input.code {
                KeyCode::Enter => input_str.push("Enter".to_string()),
                KeyCode::Home => input_str.push("Home".to_string()),
                KeyCode::End => input_str.push("End".to_string()),
                KeyCode::PageUp => input_str.push("PageUp".to_string()),
                KeyCode::PageDown => input_str.push("PageDown".to_string()),
                KeyCode::Up => input_str.push("Up".to_string()),
                KeyCode::Down => input_str.push("Down".to_string()),
                KeyCode::Tab => input_str.push("Tab".to_string()),
                KeyCode::Char(c) => input_str.push(c.to_string()),
                _ => {}
            }
            parts.push(input_str.join(""));
        }
        parts.join(" ")
    }
}

impl InputChain {
    fn to_owned(&self) -> InputChain {
        InputChain {
            prior_inputs: self.prior_inputs.to_owned(),
        }
    }

    fn from_vec(inputs: &Vec<KeyEvent>) -> InputChain {
        InputChain {
            prior_inputs: inputs.to_owned(),
        }
    }

    fn chain(&self, next: &KeyEvent) -> InputChain {
        let mut inputs: Vec<KeyEvent> = self.prior_inputs.to_owned();
        inputs.push(*next);
        InputChain {
            prior_inputs: inputs,
        }
    }

    fn get_chain(&self) -> Vec<(InputChain, Option<InputChain>)> {
        let mut inputs: Vec<KeyEvent> = Vec::new();
        let mut chain1: Vec<InputChain> = Vec::new();
        let mut chain2: Vec<Option<InputChain>> = Vec::new();

        for event in self.prior_inputs.iter() {
            inputs.push(*event);
            if !chain1.is_empty() {
                chain2.push(Some(InputChain::from_vec(&inputs)));
            }
            chain1.push(InputChain::from_vec(&inputs));
        }
        chain2.push(None);

        let mut chain: Vec<(InputChain, Option<InputChain>)> = Vec::new();

        for (a, b) in std::iter::zip(chain1, chain2) {
            chain.push((a, b));
        }

        chain
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

#[derive(Debug)]
enum KeyMapAction {
    Action(Action),
    Input(InputChain),
}

impl KeyMapAction {
    fn to_string(&self) -> String {
        match self {
            KeyMapAction::Action(action) => format!("{:?}", action),
            KeyMapAction::Input(input) => input.to_string(),
        }
    }

    fn to_owned(&self) -> KeyMapAction {
        match self {
            KeyMapAction::Action(action) => KeyMapAction::Action(action.clone()),
            KeyMapAction::Input(input) => KeyMapAction::Input(input.to_owned()),
        }
    }
}

type KeyMapType = HashMap<InputChain, KeyMapAction>;

#[derive(Debug)]
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
            return Err(JournalError::Error(ErrorKind::Wrapped(msg)));
        };
        let event = make_event(modifier, KeyCode::Char(character));
        Ok(event)
    } else {
        let msg = format!("Unable to parse keymap: {:?}", value);
        Err(JournalError::Error(ErrorKind::Wrapped(msg)))
    }
}

impl KeyMap {
    pub fn keybindings(&self) -> Vec<String> {
        let mut bindings: Vec<String> = Vec::new();

        for (binding, key_action) in self.map.iter() {
            if let KeyMapAction::Action(action) = key_action {
                let binding_str = binding.to_string();
                if binding_str.starts_with("C-") || binding_str.starts_with("M-") {
                    bindings.push(format!("{}: {}", binding_str, action.to_string()));
                }
            }
        }

        bindings.sort();
        bindings
    }

    pub fn to_string(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for (input, action) in self.map.iter() {
            let line = format!("{}: {}", input.to_string(), action.to_string());
            lines.push(line);
        }
        lines.join("\n")
    }

    // fn new(mapconfig: &HashMap<String, String>) -> Result<Self, JournalError> {
    //     let mut keys: KeyMapType = HashMap::new();

    //     for (keyseq, action_name) in mapconfig.iter() {
    //         let input = InputChain::from_str(&keyseq)?;
    //         let action = Action::from_str(&action_name)?;
    //         keys.insert(input, action);
    //     }

    //     let map = KeyMap {
    //         map: keys,
    //     };
    //     Ok(map)
    // }

    pub(self) fn action(&self, input: &InputChain) -> KeyMapAction {
        if let Some(action) = self.map.get(input) {
            action.to_owned()
        } else {
            KeyMapAction::Action(Action::NoOp)
        }
    }
}

const DEFAULT_KEYS: [(&str, Action); 19] = [
    ("C-x C-c", Action::AppExit),
    ("C-u C-u C-e", Action::ExportEntry),
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
            let input_chain = InputChain::from_str(&keyseq).unwrap();
            for (input, next_input) in input_chain.get_chain().iter() {
                if let Some(next_input) = next_input {
                    let next = KeyMapAction::Input(next_input.to_owned());
                    keys.insert(input.to_owned(), next);
                } else {
                    keys.insert(input.to_owned(), KeyMapAction::Action(action.clone()));
                }
            }
        }

        KeyMap { map: keys }
    }
}

#[derive(Debug, Clone)]
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

        match mode {
            InputMode::Ready | InputMode::ChainedInput(_) => {
                let bindings = &self.key_map.keybindings()[..];
                for row in bindings.chunks(widths.len()) {
                    let mut row_vec: Vec<String> = Vec::new();
                    for item in row.iter() {
                        row_vec.push(item.to_string());
                    }
                    rows.push(Row::new(row_vec));
                }
            }
            _ => {}
        }

        let table = Table::default().rows(rows).widths(widths).block(block);
        frame.render_widget(table, hint_area);

        if let InputMode::ChainedInput(input) = mode {
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
        match self.key_map.action(&input) {
            KeyMapAction::Action(action) => match action {
                Action::NoOp => Action::NextMode(InputMode::Ready),
                _ => action,
            },
            KeyMapAction::Input(_) => Action::NextMode(InputMode::ChainedInput(input.clone())),
        }
    }

    pub(super) fn action(
        &self,
        mode: &InputMode,
        event: &KeyEvent,
    ) -> Result<Action, JournalError> {
        match mode {
            InputMode::Ready => {
                let input = InputChain::from(event);
                return Ok(self.handle_chained_input(&input));
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
            InputMode::ChainedInput(input) => {
                let next_input = input.chain(event);
                return Ok(self.handle_chained_input(&next_input));
            }
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
