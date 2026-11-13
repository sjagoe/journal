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
    config::Config,
    errors::{ErrorKind, JournalError},
    journal::{EntryKind, age::decrypt, entry_filename},
    tui,
};

use super::age::{encrypt, load_identities};

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
    pub(super) identities: Vec<String>,
    pub(super) journal_path: String,
    editor: String,
    editor_args: Vec<String>,
    dir: TempDir,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PublicKeys {
    pub keys: Vec<String>,
}

impl PublicKeys {
    pub fn new(journal_path: &str) -> Result<Self, JournalError> {
        let mut keys_path = PathBuf::new();
        keys_path.push(journal_path);
        keys_path.push("metadata.json");

        let file = File::open(keys_path).map_err(|e| {
            JournalError::chain(&[
                ErrorKind::Encryption,
                ErrorKind::FileReadError,
                ErrorKind::Wrapped(e.to_string()),
            ])
        })?;
        let reader = BufReader::new(file);
        let public_keys: Self = serde_json::from_reader(reader).map_err(|e| {
            JournalError::chain(&[
                ErrorKind::Encryption,
                ErrorKind::FileReadError,
                ErrorKind::Wrapped(e.to_string()),
            ])
        })?;

        Ok(public_keys)
    }
}

impl EntryEditor {
    pub fn new(config: &Config) -> Result<Self, JournalError> {
        let dir = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        let (editor, editor_args) =
            get_editor_command(config.editor.as_ref().map(|editor| editor.to_owned()))?;

        let entry_editor = EntryEditor {
            identities: config.get_identities(),
            editor,
            editor_args,
            journal_path: config.journal_path.to_owned(),
            dir,
        };
        Ok(entry_editor)
    }

    fn encrypt<P: AsRef<Path>>(
        &self,
        encrypted_path: P,
        decrypted_path: Option<P>,
        text: Option<&str>,
    ) -> Result<(), JournalError> {
        let public_keys = PublicKeys::new(&self.journal_path)?;

        if let Some(path) = decrypted_path {
            let mut input =
                File::open(path).map_err(|_| JournalError::Error(ErrorKind::FileReadError))?;
            encrypt(public_keys.keys, &mut input, encrypted_path)?;
        } else if let Some(text) = text {
            let mut input = BufReader::new(text.as_bytes());
            encrypt(public_keys.keys, &mut input, encrypted_path)?;
        } else {
            panic!("encrypt() MUST have exactly one of decrypted_path or text passed")
        };

        Ok(())
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

    fn decrypt_and_edit_entry(&self, encrypted_file_path: &PathBuf) -> Result<bool, JournalError> {
        let dir = &self.dir;
        fs::set_permissions(dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        let identities = load_identities(&self.identities)?;
        let input_path = dir.path().join("input.org");

        decrypt(encrypted_file_path, input_path.clone(), &identities).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
        })?;

        let pre_edit_hash = sha256(&input_path).map_err(|_| {
            JournalError::chain(&[
                ErrorKind::EditingEntry,
                ErrorKind::Wrapped("Failed to hash original content".to_string()),
            ])
        })?;

        self.spawn_editor(&input_path).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::EditingEntry, e]),
        })?;

        // fixme: We don't want to discard changes in this case!
        let post_edit_hash = sha256(&input_path).map_err(|_| {
            JournalError::chain(&[
                ErrorKind::EditingEntry,
                ErrorKind::Wrapped("Failed to hash new content".to_string()),
            ])
        })?;

        if pre_edit_hash == post_edit_hash {
            return Ok(false);
        }

        self.encrypt(encrypted_file_path, Some(&input_path), None)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::Encryption, e]),
            })?;

        Ok(true)
    }

    fn create_new_entry(&self, encrypted_file_path: &PathBuf) -> Result<(), JournalError> {
        let mut directory_path = encrypted_file_path.to_owned();
        directory_path.pop();

        if let Some(err) = metadata(&directory_path).err()
            && err.kind() == NotFound
        {
            fs::create_dir(&directory_path).map_err(|err| {
                JournalError::chain(&[
                    ErrorKind::FileWriteError,
                    ErrorKind::Wrapped(err.to_string()),
                ])
            })?;
        }

        let _ = File::create_new(encrypted_file_path).map_err(|err| {
            JournalError::chain(&[
                ErrorKind::FileWriteError,
                ErrorKind::Wrapped(err.to_string()),
            ])
        })?;

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

        self.encrypt(encrypted_file_path, None, Some(&text))
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::Encryption, e]),
            })?;

        Ok(())
    }

    fn create_or_edit_entry(&self, filename: &str) -> Result<bool, JournalError> {
        let mut encrypted_file_path = PathBuf::new();
        encrypted_file_path.push(&self.journal_path);
        encrypted_file_path.push(filename);

        if !encrypted_file_path.exists() {
            return Err(JournalError::Error(ErrorKind::FileMissingError));
        }

        let display_editor = if self.editor_args.is_empty() {
            self.editor.to_string()
        } else {
            format!("{} {}", self.editor, join(&self.editor_args))
        };

        eprintln!("Opening {} with {}", filename, display_editor);

        let edited = self.decrypt_and_edit_entry(&encrypted_file_path)?;
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

        let mut encrypted_file_path = PathBuf::new();
        encrypted_file_path.push(&self.journal_path);
        encrypted_file_path.push(filename);

        if !encrypted_file_path.exists() {
            self.create_new_entry(&encrypted_file_path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
