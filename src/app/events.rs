use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug)]
pub enum Event {
    Select,
    New,
    Back,
    Home,
    End,
    PageUp,
    PageDown,
    NextSelection,
    PreviousSelection,
    SwitchPane,
    TextEntry(KeyCode),
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
        match key {
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Enter,
                ..
            } => Event::Select,
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Home,
                ..
            } => Event::Home,
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::End,
                ..
            } => Event::End,
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::PageUp,
                ..
            } => Event::PageUp,
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::PageDown,
                ..
            } => Event::PageDown,
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Up,
                ..
            } => Event::PreviousSelection,
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Down,
                ..
            } => Event::NextSelection,
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Tab,
                ..
            } => Event::SwitchPane,
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                code: KeyCode::Char(_)
                | KeyCode::Backspace
                | KeyCode::Delete
                | KeyCode::Left
                | KeyCode::Right,
                ..
            } => Event::TextEntry(key.code),
            KeyEvent {
                modifiers: KeyModifiers::ALT,
                code: KeyCode::Char(character),
                ..
            } => match character {
                'n' => Event::New,
                'v' => Event::PageUp,
                _ => Event::Invalid,
            }
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                code: KeyCode::Char(character),
                ..
            } => match character {
                'g' => Event::Back,
                'n' => Event::NextSelection,
                'p' => Event::PreviousSelection,
                'v' => Event::PageDown,
                _ => Event::Invalid,
            }
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
            Event::Exit => ("C-x", "Exit"),
            Event::Home => ("Home", "Go to the top"),
            Event::End => ("End", "Go to the bottom"),
            Event::PageUp => ("M-v", "Page Up"),
            Event::PageDown => ("C-v", "Page Down"),
            Event::NextSelection => ("C-n", "Select the next item"),
            Event::PreviousSelection => ("C-p", "Select the previous item"),
            Event::SwitchPane => ("Tab", "Navigate between entry types"),
            Event::TextEntry(_) => ("", ""),
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
