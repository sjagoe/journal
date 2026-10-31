use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Child, Command},
};

use color_eyre::Result;
use tempfile::TempDir;

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
};

use super::{EntryKind, edit::program_from_string, entry_filename};
use super::{
    age::{decrypt, load_identities},
    edit::EntryEditor,
};

#[derive(Debug)]
enum ExportProcess {
    Export(Child),
    Render(Child),
}

pub struct EntryExporter {
    filename: String,
    progress: f64,
    entry_editor: EntryEditor,
    dir: TempDir,
    export_process: Option<ExportProcess>,
    export_command: String,
    export_command_args: Vec<String>,
}

fn get_export_command(
    configured_export: Option<String>,
) -> Result<(String, Vec<String>), JournalError> {
    match configured_export {
        Some(ref export) => program_from_string(export),
        None => Ok(("xdg-open".to_string(), vec![])),
    }
}

impl EntryExporter {
    pub fn new(config: &Config, kind: &EntryKind, entry: &str) -> Result<Self, JournalError> {
        let filename = entry_filename(kind, entry);
        let dir = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;
        let entry_editor = EntryEditor::new(config)?;

        let (export_command, export_command_args) =
            get_export_command(config.export.as_ref().map(|export| export.to_owned()))?;

        let exporter = Self {
            filename,
            progress: 0.0,
            entry_editor,
            dir,
            export_process: None,
            export_command,
            export_command_args,
        };
        Ok(exporter)
    }

    fn check_progress(&mut self) -> Result<(), JournalError> {
        if let Some(ref mut export_process) = self.export_process {
            match export_process {
                ExportProcess::Export(child) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        if status.success() {
                            self.export_process = None;
                            self.spawn_browser(&self.exported_path())?;
                        } else {
                            let message = format!("Editor exited with code {:?}", status.code());
                            return Err(JournalError::chain(&[
                                ErrorKind::Subprocess,
                                ErrorKind::Wrapped(message),
                            ]));
                        }
                    }
                }
                ExportProcess::Render(child) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        if status.success() {
                            self.export_process = None;
                        } else {
                            let message = format!("Browser exited with code {:?}", status.code());
                            return Err(JournalError::chain(&[
                                ErrorKind::Subprocess,
                                ErrorKind::Wrapped(message),
                            ]));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn tick(&mut self, tick_duration: u8) -> Result<bool, JournalError> {
        self.check_progress()?;
        let mut update_progress = |factor| {
            if self.progress >= 1.0 {
                true
            } else {
                let duration: f64 = tick_duration.into();
                let new = self.progress + (duration / factor);
                self.progress = new;
                false
            }
        };
        if let Some(ref export_process) = self.export_process {
            match export_process {
                ExportProcess::Export(_) => {
                    // Just long progress bars .. we don't really have
                    // anything to go on to monitor progress
                    let _ = update_progress(10000.0);
                    return Ok(false);
                }
                _ => {
                    return Ok(update_progress(1000.0));
                }
            }
        }
        Ok(update_progress(1000.0))
    }

    pub fn get_progress(&self) -> f64 {
        if self.progress >= 1.0 {
            1.0
        } else {
            self.progress
        }
    }

    fn spawn_browser(&mut self, exported_path: &PathBuf) -> Result<(), JournalError> {
        let mut command = Command::new(&self.export_command);
        command.args(&self.export_command_args);
        command.arg(exported_path);

        if let Ok(child) = command.spawn() {
            self.export_process = Some(ExportProcess::Render(child));
        } else {
            return Err(JournalError::chain(&[
                ErrorKind::Subprocess,
                ErrorKind::Wrapped("Failed to execute browser".to_string()),
            ]));
        }

        Ok(())
    }

    fn spawn_exporter(&mut self, decrypted_path: &PathBuf) -> Result<(), JournalError> {
        let mut command = Command::new("emacs");
        command
            .arg("-q")
            .arg(decrypted_path)
            .args([
                "--batch",
                "-f",
                "org-mode",
                "-f",
                "org-html-export-to-html",
                "--kill",
            ])
            .current_dir(self.dir.path())
            .env("_JOURNAL_EMACS_EMBEDDED", "t");

        if let Ok(child) = command.spawn() {
            self.export_process = Some(ExportProcess::Export(child));
        } else {
            return Err(JournalError::chain(&[
                ErrorKind::Subprocess,
                ErrorKind::Wrapped("Failed to execute editor".to_string()),
            ]));
        }

        Ok(())
    }

    fn decrypted_path(&self) -> PathBuf {
        self.dir.path().join("input.org")
    }

    fn exported_path(&self) -> PathBuf {
        self.decrypted_path().with_extension("html")
    }

    pub fn start_export(&mut self) -> Result<(), JournalError> {
        let journal_path = &self.entry_editor.journal_path;

        let mut encrypted_file_path = PathBuf::new();
        encrypted_file_path.push(journal_path);
        encrypted_file_path.push(&self.filename);

        let mut style_path = PathBuf::new();
        style_path.push(journal_path);
        style_path.push("style.css.org");

        fs::set_permissions(&self.dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        let identities = load_identities(&self.entry_editor.identity)?;
        let input_path = self.decrypted_path();

        let target_style_path = self.dir.path().join("style.css.org");

        decrypt(encrypted_file_path, input_path.clone(), &identities).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
        })?;

        symlink(&style_path, &target_style_path).map_err(|_| {
            JournalError::chain(&[
                ErrorKind::FileWriteError,
                ErrorKind::Wrapped("Failed to symlink stylesheel".to_string()),
            ])
        })?;

        self.spawn_exporter(&input_path)?;

        // let exported_path =

        // self.spawn_browser(&exported_path)?;

        // // FIXME this is terrible.
        // // We need to background this and wait for it to complete
        // // The sleep for firefox to read the file before we delete it is also not great...
        // std::thread::sleep(std::time::Duration::from_millis(500));

        Ok(())
    }
}
