use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug)]
pub enum Event {
    Select,
    // Back,
    Home,
    End,
    PageUp,
    PageDown,
    NextSelection,
    PreviousSelection,
    SwitchPane,
    // ExternalEditor,
    Exit,
    Invalid,
}

pub struct ActionInfo {
    key: String,
    name: String,
}

impl From<KeyEvent> for Event {
    fn from(key: KeyEvent) -> Event {
        match (key.code, key.modifiers.bits()) {
            (KeyCode::Enter, 0) => Event::Select,
            (KeyCode::Home, 0) => Event::Home,
            (KeyCode::End, 0) => Event::End,
            (KeyCode::PageUp, 0) => Event::PageUp,
            (KeyCode::PageDown, 0) => Event::PageDown,
            (KeyCode::Up, 0) => Event::PreviousSelection,
            (KeyCode::Down, 0) => Event::NextSelection,
            (KeyCode::Tab, 0) => Event::SwitchPane,
            (keycode, modifiers) => {
                if modifiers == KeyModifiers::ALT.bits() {
                    match keycode {
                        KeyCode::Char('v') => Event::PageUp,
                        _ => Event::Invalid,
                    }
                } else if modifiers == KeyModifiers::CONTROL.bits() {
                    match keycode {
                        KeyCode::Char('n') => Event::NextSelection,
                        KeyCode::Char('p') => Event::PreviousSelection,
                        KeyCode::Char('v') => Event::PageDown,
                        _ => Event::Invalid,
                    }
                } else {
                    Event::Invalid
                }
            }
        }
    }
}

impl From<Event> for ActionInfo {
    fn from(action: Event) -> ActionInfo {
        ActionInfo::from(&action)
    }
}

impl From<&Event> for ActionInfo {
    fn from(action: &Event) -> ActionInfo {
        let (key, name) = match action {
            Event::Select => ("Enter", "Select"),
            Event::Exit => ("C-x", "Exit"),
            Event::Home => ("Home", "Go to the top"),
            Event::End => ("End", "Go to the bottom"),
            Event::PageUp => ("M-v", "Page Up"),
            Event::PageDown => ("C-v", "Page Down"),
            Event::NextSelection => ("C-n", "Select the next item"),
            Event::PreviousSelection => ("C-p", "Select the previous item"),
            Event::SwitchPane => ("Tab", "Navigate between entry types"),
            // We never want to see this but have to define it
            Event::Invalid => ("invalid", "invalid"),
        };
        ActionInfo {
            key: key.to_string(),
            name: name.to_string(),
        }
    }
}

impl fmt::Display for ActionInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.name)
    }
}
