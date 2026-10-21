use std::{env, path::PathBuf};
use std::process::Command;

use color_eyre::Result;

use shell_words::split;

use crate::{
    config::Config, errors::{ErrorKind, JournalError}, tui,
};

pub struct EntryEditor {
    editor: Option<Vec<String>>,
    path: PathBuf,
}

impl EntryEditor {
    pub fn new(path: &PathBuf, config: &Config) -> Self {
        let editor = match config.editor {
            Some(ref editor) => Some(editor.to_vec()),
            _ => None,
        };
        EntryEditor {
            editor: editor,
            path: path.to_owned(),
        }
    }

    fn get_editor_command(&self) -> Result<(String, Vec<String>), JournalError> {
        match self.editor {
            Some(ref editor) => {
                let prog = &editor[0];
                let args = &editor[1..];
                Ok::<(String, Vec<String>), JournalError>((prog.to_string(), args.to_vec()))
            }
            _ => {
                if let Some(pager_env) = env::var_os("PAGER") {
                    let pager = pager_env.into_string().map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;;
                    let editor = split(&pager).map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;;
                    let prog = &editor[0];
                    let args = &editor[1..];
                    Ok((prog.to_string(), args.to_vec()))
                } else {
                    let args: Vec<String> = vec![];
                    Ok(("nano".to_string(), args))
                }
            }
        }
    }

    fn spawn_editor(&self) -> Result<(), JournalError> {
        let (prog, args) = self.get_editor_command()?;
        // let (prog, args) = match self.editor {
        //     Some(ref editor) => (&editor[0].to_owned(), &editor[1..].to_vec()),
        //     _ => {
        //         if let Some(pager) = env::var_os("PAGER") {
        //             pager.into_string().expect("Unable to decode PAGER env var")
        //         } else {
        //             ("nano".to_string(), [])
        //         };
        //     }
        // };
        let mut command = Command::new(&prog);

        for arg in args.iter() {
            command.arg(arg);
        }

        command.arg(&self.path);

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
        eprintln!(
            "Opening {} with {:?} ...",
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
