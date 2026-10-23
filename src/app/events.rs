use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, PartialEq)]
pub enum Event {
    Select,
    New,
    Back,
    BackOrExit,
    Home,
    End,
    PageUp,
    PageDown,
    NextSelection,
    PreviousSelection,
    SwitchPane,
    TextEntry(KeyCode),
    Export,
    Invalid,
}

pub struct ActionInfo {
    key: String,
    name: String,
}

impl From<KeyEvent> for Event {
    fn from(key: KeyEvent) -> Event {
        match key.modifiers {
            KeyModifiers::NONE => match key.code {
                KeyCode::Enter => Event::Select,
                KeyCode::Home => Event::Home,
                KeyCode::End => Event::End,
                KeyCode::PageUp => Event::PageUp,
                KeyCode::PageDown => Event::PageDown,
                KeyCode::Up => Event::PreviousSelection,
                KeyCode::Down => Event::NextSelection,
                KeyCode::Tab => Event::SwitchPane,
                KeyCode::Char(_)
                | KeyCode::Backspace
                | KeyCode::Delete
                | KeyCode::Left
                | KeyCode::Right => Event::TextEntry(key.code),
                _ => Event::Invalid,
            },
            KeyModifiers::ALT => match key.code {
                KeyCode::Char('n') => Event::New,
                KeyCode::Char('v') => Event::PageUp,
                _ => Event::Invalid,
            },
            KeyModifiers::CONTROL => match key.code {
                KeyCode::Char('e') => Event::Export,
                KeyCode::Char('g') => Event::Back,
                KeyCode::Char('n') => Event::NextSelection,
                KeyCode::Char('p') => Event::PreviousSelection,
                KeyCode::Char('v') => Event::PageDown,
                KeyCode::Char('x') => Event::BackOrExit,
                _ => Event::Invalid,
            },
            _ => Event::Invalid,
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
            Event::New => ("M-n", "Create new entry"),
            Event::Back => ("C-g", "Back"),
            Event::BackOrExit => ("C-x", "Exit"),
            Event::Home => ("Home", "Go to the top"),
            Event::End => ("End", "Go to the bottom"),
            Event::PageUp => ("M-v", "Page Up"),
            Event::PageDown => ("C-v", "Page Down"),
            Event::NextSelection => ("C-n", "Select the next item"),
            Event::PreviousSelection => ("C-p", "Select the previous item"),
            Event::SwitchPane => ("Tab", "Navigate between entry types"),
            Event::TextEntry(_) => ("", ""),
            Event::Export => (
                "C-e",
                "Export entry to html and open with the default browser",
            ),
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
