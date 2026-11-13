use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};

use color_eyre::Result;

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
};

use super::{EntryKind, Journal, decrypted_journal::DecryptedJournal, edit::program_from_string, entry_filename};
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
    journal: DecryptedJournal,
    encrypted_path: String,
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
    pub fn new(config: &Config, journal: &Journal) -> Result<Self, JournalError> {
        let decrypted = DecryptedJournal::new(config, journal)?;

        let (export_command, export_command_args) =
            get_export_command(config.export.as_ref().map(|export| export.to_owned()))?;

        let exporter = Self {
            progress: 0.0,
            journal: decrypted,
            encrypted_path: journal.path.clone(),
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
                            let exported_path = self
                                .exported_path
                                .as_ref()
                                .expect("Should have been set at start of export")
                                .clone();
                            self.export_process = None;
                            self.spawn_browser(&exported_path)?;
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
        decrypted_path: P,
    ) -> Result<(), JournalError> {
        let mut command = Command::new("emacs");
        command
            .arg(decrypted_path)
            .args([
                "--batch",
                "-f",
                "org-mode",
                "-f",
                "org-html-export-to-html",
                "--kill",
            ])
            .current_dir(self.journal.dir.path())
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

    fn decrypt_item<D: AsRef<Path>, E: AsRef<Path>>(
        &self,
        dir: D,
        encrypted_path: E,
        identities: &Vec<Box<dyn age::Identity>>,
    ) -> Result<PathBuf, JournalError> {
        let mut decrypted = PathBuf::from(dir.as_ref());
        let filename = encrypted_path.as_ref().file_name().unwrap();
        decrypted.push(filename);
        decrypted.set_extension("org");

        decrypt(encrypted_path, &decrypted, identities).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
        })?;

        Ok(decrypted)
    }

    pub fn export_single_entry(
        &mut self,
        kind: &EntryKind,
        entry: &str,
    ) -> Result<(), JournalError> {
        let filename = entry_filename(kind, entry);

        let mut style_path = PathBuf::new();
        style_path.push(&self.encrypted_path);
        style_path.push("style.css.org");

        let input_path = self.journal.item_path(&filename);

        let target_style_path = self.journal.dir.path().join("style.css.org");
        symlink(&style_path, &target_style_path).map_err(|_| {
            JournalError::chain(&[
                ErrorKind::FileWriteError,
                ErrorKind::Wrapped("Failed to symlink stylesheel".to_string()),
            ])
        })?;

        self.spawn_exporter(&input_path)?;

        self.exported_path = Some(input_path.with_extension("html"));

        Ok(())
    }

    pub fn export_journal(&mut self, journal: &Journal) -> Result<(), JournalError> {
        let mut index: Vec<String> = Vec::new();

        index.push("#+SETUPFILE: style.css.org".to_string());
        index.push("* Index".to_string());

        let mut style_path = PathBuf::new();
        style_path.push(&self.encrypted_path);
        style_path.push("style.css.org");

        for section in journal.sections() {
            let kind_path = self
                .journal
                .dir
                .path()
                .join(String::from(&section.kind));
            index.push(format!("** {}\n", String::from(&section.kind)));

            let target_style_path = kind_path.join("style.css.org");
            symlink(&style_path, &target_style_path).map_err(|_| {
                JournalError::chain(&[
                    ErrorKind::FileWriteError,
                    ErrorKind::Wrapped("Failed to symlink stylesheel".to_string()),
                ])
            })?;

            for entry_name in journal.section(&section.kind).iter_entries() {
                let filename = entry_filename(&section.kind, entry_name);
                let decrypted_path = self.journal.item_path(&filename);

                let link = format!("[[file:{}][~{}~]]", decrypted_path.display(), entry_name);
                index.push(link);

                self.spawn_exporter(&decrypted_path)?;
            }
        }

        let target_style_path = self.journal.dir.path().join("style.css.org");
        symlink(&style_path, &target_style_path).map_err(|_| {
            JournalError::chain(&[
                ErrorKind::FileWriteError,
                ErrorKind::Wrapped("Failed to symlink stylesheel".to_string()),
            ])
        })?;

        let index_path = self.journal.dir.path().join("index.org");

        std::fs::write(&index_path, index.join("\n\n")).unwrap();

        self.spawn_exporter(&index_path)?;

        self.exported_path = Some(index_path.with_extension("html"));

        Ok(())
    }
}
