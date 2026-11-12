use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Child, Command},
    time::{Duration, Instant},
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
    progress: f64,
    entry_editor: EntryEditor,
    dir: TempDir,
    export_process: Option<ExportProcess>,
    launch_started: Option<Instant>,
    export_command: String,
    export_command_args: Vec<String>,
    exported_path: Option<PathBuf>,
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
    pub fn new(config: &Config) -> Result<Self, JournalError> {
        let dir = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;
        let entry_editor = EntryEditor::new(config)?;

        let (export_command, export_command_args) =
            get_export_command(config.export.as_ref().map(|export| export.to_owned()))?;

        let exporter = Self {
            progress: 0.0,
            entry_editor,
            dir,
            export_process: None,
            launch_started: None,
            export_command,
            export_command_args,
            exported_path: None,
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
        let mut update_progress = || {
            // Give the export launch a moment to happen.
            // xdg-open and browsers launching a tab in an open
            // session usually return control before the command has
            // been executed.
            if let Some(launch_started) = self.launch_started
                && launch_started.elapsed() > Duration::from_millis(1500)
            {
                true
            } else {
                let factor = 1000.0;
                let duration: f64 = tick_duration.into();
                let elapsed = duration / factor;
                let new = self.progress + elapsed;
                self.progress = new;
                false
            }
        };
        if let Some(ref export_process) = self.export_process {
            match export_process {
                ExportProcess::Export(_) => {
                    let _ = update_progress();
                    return Ok(false);
                }
                _ => {
                    return Ok(update_progress());
                }
            }
        }
        Ok(update_progress())
    }

    pub fn get_progress(&self) -> f64 {
        self.progress
    }

    fn spawn_browser(&mut self, exported_path: &PathBuf) -> Result<(), JournalError> {
        self.launch_started = Some(Instant::now());

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

    fn spawn_exporter<P: AsRef<std::ffi::OsStr>>(
        &mut self,
        decrypted_paths: &[P],
    ) -> Result<(), JournalError> {
        let mut command = Command::new("emacs");
        command
            .args(decrypted_paths)
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

    pub fn export_single_entry(&mut self, kind: &EntryKind, entry: &str) -> Result<(), JournalError> {
        let filename = entry_filename(kind, entry);
        let journal_path = &self.entry_editor.journal_path;

        let mut encrypted_file_path = PathBuf::new();
        encrypted_file_path.push(journal_path);
        encrypted_file_path.push(&filename);

        let mut style_path = PathBuf::new();
        style_path.push(journal_path);
        style_path.push("style.css.org");

        fs::set_permissions(&self.dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        let identities = load_identities(&self.entry_editor.identities)?;
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

        self.spawn_exporter(&[&input_path])?;

        self.exported_path = Some(input_path);

        Ok(())
    }
}
