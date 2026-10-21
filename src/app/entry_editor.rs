use std::process::Command;

use color_eyre::Result;

// use shell_words::split;

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
    tui,
};

pub struct EntryEditor {
    identity: String,
    editor: Option<Vec<String>>,
    journal_path: String,
    filename: String,
}

impl EntryEditor {
    pub fn new(entry: &str, config: &Config) -> Result<Self, JournalError> {
        let filename = format!("{}.age", entry);

        let editor = match config.editor {
            Some(ref editor) => Some(editor.to_vec()),
            _ => None,
        };

        let entry_editor = EntryEditor {
            identity: config.identity.to_owned(),
            editor: editor,
            filename,
            journal_path: config.journal_path.to_owned(),
        };
        Ok(entry_editor)
    }

    // fn get_editor_command(&self) -> Result<(String, Vec<String>), JournalError> {
    //     match self.editor {
    //         Some(ref editor) => {
    //             let prog = &editor[0];
    //             let args = &editor[1..];
    //             Ok::<(String, Vec<String>), JournalError>((prog.to_string(), args.to_vec()))
    //         }
    //         _ => {
    //             if let Some(pager_env) = env::var_os("PAGER") {
    //                 let pager = pager_env.into_string().map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;;
    //                 let editor = split(&pager).map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;;
    //                 let prog = &editor[0];
    //                 let args = &editor[1..];
    //                 Ok((prog.to_string(), args.to_vec()))
    //             } else {
    //                 let args: Vec<String> = vec![];
    //                 Ok(("nano".to_string(), args))
    //             }
    //         }
    //     }
    // }

    fn spawn_editor(&self) -> Result<(), JournalError> {
        let mut command = Command::new("ragenix");
        command.args([
            "--identity",
            &self.identity,
            "--edit",
            &self.filename,
            "--rules",
            "./rules.nix",
        ]);
        command.current_dir(&self.journal_path);

        let mut process = command
            .spawn()
            .map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        let status = process
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
        eprintln!("Opening {} with {:?} ...", self.filename, self.editor);

        let error = self.spawn_editor().err();
        tui::init().map_err(|_| JournalError::Error(ErrorKind::TerminalInit))?;

        if let Some(e) = error {
            return Err(e);
        }
        Ok(())
    }
}
