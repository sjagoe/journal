use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use color_eyre::Result;
use tempfile::TempDir;

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
};

use super::{
    EntryKind, Journal, decrypted_journal::DecryptedJournal, edit::program_from_string,
    entry_filename,
};

#[derive(Debug)]
enum ExportProcess {
    Export(Child),
    Render(Child),
}

pub struct EntryExporter {
    progress: f64,
    journal: DecryptedJournal,
    export_process: Option<ExportProcess>,
    launch_started: Option<Instant>,
    export_command: String,
    export_command_args: Vec<String>,
    publish_path: TempDir,
    open_entry: Option<String>,
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
        let publish_path = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;
        fs::set_permissions(&publish_path, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        let decrypted = DecryptedJournal::new(config, journal)?;

        let (export_command, export_command_args) =
            get_export_command(config.export.as_ref().map(|export| export.to_owned()))?;

        let exporter = Self {
            progress: 0.0,
            journal: decrypted,
            export_process: None,
            launch_started: None,
            export_command,
            export_command_args,
            publish_path,
            open_entry: None,
        };
        Ok(exporter)
    }

    pub fn open_entry(&mut self, kind: &EntryKind, name: &str) {
        self.open_entry = Some(entry_filename(kind, name));
    }

    fn check_progress(&mut self) -> Result<(), JournalError> {
        if let Some(ref mut export_process) = self.export_process {
            match export_process {
                ExportProcess::Export(child) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        if status.success() {
                            let exported_path = if let Some(ref open_entry) = self.open_entry {
                                self.publish_path
                                    .path()
                                    .join(open_entry)
                                    .with_extension("html")
                            } else {
                                self.publish_path.path().join("index.html")
                            };
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
        export_elisp: Option<PathBuf>,
    ) -> Result<(), JournalError> {
        let mut command = Command::new("emacs");
        if let Some(elisp) = export_elisp {
            command
                .args(["--batch", "--script"])
                .arg(elisp)
                .args(["-f", "org-mode", "-f", "org-publish-all", "--kill"])
                .stderr(Stdio::null());
        } else {
            command.arg(decrypted_path).args([
                "--batch",
                "-f",
                "org-mode",
                "-f",
                "org-html-export-to-html",
                "--kill",
            ]);
        }
        command
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

    pub fn export_journal(&mut self, journal: &Journal) -> Result<(), JournalError> {
        let mut index: Vec<String> = Vec::new();

        index.push("#+SETUPFILE: style.css.org".to_string());
        index.push("* Index".to_string());

        for section in journal.sections() {
            index.push(format!("** {}\n", String::from(&section.kind)));

            for entry_name in journal.section(&section.kind).iter_entries() {
                let filename = entry_filename(&section.kind, entry_name);
                let decrypted_path = self.journal.item_path(&filename);

                let link = format!("[[file:{}][~{}~]]", decrypted_path.display(), entry_name);
                index.push(link);
            }
        }

        let index_path = self.journal.dir.path().join("index.org");

        std::fs::write(&index_path, index.join("\n\n")).unwrap();

        let project_config = format!(
            "(setq org-publish-project-alist
  '((\"journal\"
    :base-directory {:?}
    :publishing-directory {:?}
    :recursive t)))
",
            self.journal.dir.path().display(),
            self.publish_path.path().display(),
        );

        let project_path = PathBuf::from(self.journal.dir.path()).join("export.el");
        std::fs::write(&project_path, &project_config).unwrap();

        self.spawn_exporter(&index_path, Some(project_path))?;

        Ok(())
    }
}
