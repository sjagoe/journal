use std::{
    fmt::{self, Display},
    str::FromStr,
};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use regex::Regex;

use crate::errors::{ErrorKind, JournalError};

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

#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub(crate) struct InputChain {
    prior_inputs: Vec<KeyEvent>,
}

impl Display for InputChain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
        write!(f, "{}", parts.join(" "))
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

    pub(super) fn chain(&self, next: &KeyEvent) -> InputChain {
        let mut inputs: Vec<KeyEvent> = self.prior_inputs.to_owned();
        inputs.push(*next);
        InputChain {
            prior_inputs: inputs,
        }
    }

    pub(super) fn get_chain(&self) -> Vec<(InputChain, Option<InputChain>)> {
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
