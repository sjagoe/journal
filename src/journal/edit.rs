use std::{
    env,
    ffi::OsStr,
    fs::{self, File, metadata},
    io::{self, BufReader, ErrorKind::NotFound},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use color_eyre::Result;
use digest_io::IoWrapper;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use shell_words::{join, split};
use tempfile::TempDir;
use time::macros::format_description;
use time::{OffsetDateTime, format_description::StaticFormatDescription};

use crate::{
    config::Config, errors::{ErrorKind, JournalError}, journal::{EntryKind, age::decrypt, decrypted_journal, entry_filename}, tui,
};

use super::{Journal, age::load_identities, decrypted_journal::DecryptedJournal};

static ORG_DATE_FORMAT: StaticFormatDescription =
    format_description!("[year]-[month]-[day] [weekday repr:short]");

pub(crate) fn sha256<P: AsRef<Path>>(path: P) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut hasher = IoWrapper(Sha256::new());
    io::copy(&mut file, &mut hasher)?;
    Ok(hasher.0.finalize().to_vec())
}

pub fn program_from_string(editor: &str) -> Result<(String, Vec<String>), JournalError> {
    let parts = split(editor).map_err(|_| {
        JournalError::chain(&[
            ErrorKind::Subprocess,
            ErrorKind::CommandParseError,
            ErrorKind::Wrapped("Could not parse editor string in config".to_string()),
        ])
    })?;
    let prog = &parts[0];
    let args = &parts[1..];
    Ok((prog.to_string(), args.to_vec()))
}

fn get_editor_command(
    configured_editor: Option<String>,
) -> Result<(String, Vec<String>), JournalError> {
    match configured_editor {
        Some(ref editor) => program_from_string(editor),
        None => {
            if let Some(visual_env) = env::var_os("VISUAL") {
                let pager = visual_env.into_string().map_err(|_| {
                    JournalError::chain(&[
                        ErrorKind::Subprocess,
                        ErrorKind::CommandParseError,
                        ErrorKind::Wrapped("Could not parse VISUAL env var".to_string()),
                    ])
                })?;
                program_from_string(&pager)
            } else if let Some(editor_env) = env::var_os("EDITOR") {
                let pager = editor_env.into_string().map_err(|_| {
                    JournalError::chain(&[
                        ErrorKind::Subprocess,
                        ErrorKind::CommandParseError,
                        ErrorKind::Wrapped("Could not parse EDITOR env var".to_string()),
                    ])
                })?;
                program_from_string(&pager)
            } else {
                let args: Vec<String> = vec![];
                Ok(("nano".to_string(), args))
            }
        }
    }
}

pub struct EntryEditor {
    editor: String,
    editor_args: Vec<String>,
    journal: DecryptedJournal,
}

impl EntryEditor {
    pub fn new(config: &Config, journal: &Journal) -> Result<Self, JournalError> {
        let (editor, editor_args) =
            get_editor_command(config.editor.as_ref().map(|editor| editor.to_owned()))?;

        let decrypted = DecryptedJournal::new(config, journal)?;
        let entry_editor = EntryEditor {
            editor,
            editor_args,
            journal: decrypted,
        };
        Ok(entry_editor)
    }

    fn spawn_editor<P: AsRef<OsStr>>(&self, decrypted_path: P) -> Result<(), JournalError> {
        let mut command = Command::new(&self.editor);
        command
            .args(&self.editor_args)
            .arg(decrypted_path)
            .env("_JOURNAL_EMACS_EMBEDDED", "t");

        if let Ok(mut child) = command.spawn() {
            if let Ok(status) = child.wait() {
                if !status.success() {
                    let message = format!("Editor exited with code {:?}", status.code());
                    return Err(JournalError::chain(&[
                        ErrorKind::Subprocess,
                        ErrorKind::Wrapped(message),
                    ]));
                }
            } else {
                return Err(JournalError::chain(&[
                    ErrorKind::Subprocess,
                    ErrorKind::Wrapped("Failed to wait for editor to exit".to_string()),
                ]));
            }
        } else {
            return Err(JournalError::chain(&[
                ErrorKind::Subprocess,
                ErrorKind::Wrapped("Failed to execute editor".to_string()),
            ]));
        }

        Ok(())
    }

    fn edit_entry(&self, filename: &str, decrypted_file_path: &PathBuf) -> Result<bool, JournalError> {
        let pre_edit_hash = sha256(&decrypted_file_path).map_err(|_| {
            JournalError::chain(&[
                ErrorKind::EditingEntry,
                ErrorKind::Wrapped("Failed to hash original content".to_string()),
            ])
        })?;

        self.spawn_editor(&decrypted_file_path).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::EditingEntry, e]),
        })?;

        // fixme: We don't want to discard changes in this case!
        let post_edit_hash = sha256(&decrypted_file_path).map_err(|_| {
            JournalError::chain(&[
                ErrorKind::EditingEntry,
                ErrorKind::Wrapped("Failed to hash new content".to_string()),
            ])
        })?;

        if pre_edit_hash == post_edit_hash {
            return Ok(false);
        }

        self.journal.encrypt_path(filename, &decrypted_file_path)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::Encryption, e]),
            })?;

        Ok(true)
    }

    fn create_new_entry(&self, filename: &str) -> Result<(), JournalError> {
        let now = OffsetDateTime::now_local()
            .map_err(|_| JournalError::chain(&[ErrorKind::NewEntry, ErrorKind::GetCurrentTime]))?;

        let now_org_str = now
            .format(ORG_DATE_FORMAT)
            .map_err(|_| JournalError::chain(&[ErrorKind::NewEntry, ErrorKind::FormatDateTime]))?;

        let text = format!(
            "#+SETUPFILE: style.css.org

* [{}]
",
            now_org_str
        );

        self.journal.encrypt_text(filename, &text)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::Encryption, e]),
            })?;

        Ok(())
    }

    fn create_or_edit_entry(&self, filename: &str) -> Result<bool, JournalError> {
        let decrypted_path = self.journal.edit_item(filename)?;

        let display_editor = if self.editor_args.is_empty() {
            self.editor.to_string()
        } else {
            format!("{} {}", self.editor, join(&self.editor_args))
        };

        eprintln!("Opening {} with {}", filename, display_editor);

        let edited = self.edit_entry(filename, &decrypted_path)?;
        if !edited {
            println!("{} wasn't changed, skipping re-encryption.", filename);
        }
        Ok(edited)
    }

    pub fn edit(&self, kind: &EntryKind, entry: &str) -> Result<bool, JournalError> {
        let filename = entry_filename(kind, entry);

        tui::restore().map_err(|_| JournalError::Error(ErrorKind::TerminalInit))?;

        let (changed, error) = match self.create_or_edit_entry(&filename) {
            Ok(changed) => (changed, None),
            Err(error) => (true, Some(error)),
        };

        tui::init().map_err(|_| JournalError::Error(ErrorKind::TerminalInit))?;

        if let Some(e) = error {
            return Err(e);
        }
        Ok(changed)
    }

    pub fn create_entry(&self, kind: &EntryKind, entry: &str) -> Result<bool, JournalError> {
        let filename = entry_filename(kind, entry);

        if !self.journal.encrypted_exists(&filename) {
            self.create_new_entry(&filename)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
