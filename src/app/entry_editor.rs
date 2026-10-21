use std::path::PathBuf;
use std::process::Command;

use color_eyre::Result;

use crate::{
    errors::{ErrorKind, JournalError},
    tui,
};

pub struct EntryEditor {
    editor: String,
    path: PathBuf,
}

impl EntryEditor {
    pub fn new(path: &PathBuf, editor: &str) -> Self {
        EntryEditor {
            editor: editor.to_owned(),
            path: path.to_owned(),
        }
    }

    fn spawn_editor(&self) -> Result<(), JournalError> {
        let mut command = Command::new(&self.editor)
            .arg(&self.path)
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
        eprintln!(
            "Opening {} with {} ...",
            self.path.to_string_lossy(),
            self.editor
        );

        let error = self.spawn_editor().err();
        tui::init().map_err(|_| JournalError::Error(ErrorKind::TerminalInit))?;

        if let Some(e) = error {
            return Err(e);
        }
        Ok(())
    }
}
