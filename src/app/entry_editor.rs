use std::path::PathBuf;
use std::process::Command;

use color_eyre::Result;

use crate::{
    errors::{ErrorKind, JournalError},
    tui,
};

pub struct EntryEditor {
    editor: String,
    name: String,
}

impl EntryEditor {
    pub fn new(name: &str, editor: &str) -> Self {
        EntryEditor {
            editor: editor.to_string(),
            name: name.to_string(),
        }
    }

    fn spawn_editor(&self) -> Result<(), JournalError> {
        let mut file = PathBuf::new();

        let mut command = Command::new(&self.editor)
            .arg(file.as_path())
            .spawn()
            .map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        let status = command
            .wait()
            .map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        if status.success() {
            Ok(())
        } else {
            Err(JournalError::Error(ErrorKind::Subprocess))
        }
    }

    pub fn display(&self) -> Result<(), JournalError> {
        tui::restore().map_err(|_| JournalError::Error(ErrorKind::TerminalInit))?;
        eprintln!("Opening {} with {} ...", self.name, self.editor);

        let error = self.spawn_editor().err();
        tui::init().map_err(|_| JournalError::Error(ErrorKind::TerminalInit))?;

        if let Some(e) = error {
            return Err(e);
        }
        Ok(())
    }
}
