use std::{
    env,
    ffi::OsStr,
    fs::{self, File},
    io::{self, BufReader, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use age::{
    armor::ArmoredReader,
    cli_common::{
        StdinGuard,
        file_io::{InputReader, OutputFormat, OutputWriter},
    },
    decryptor::RecipientsDecryptor,
};

use color_eyre::Result;
use digest_io::IoWrapper;
use sha2::{Digest, Sha256};
use shell_words::{join, split};
use time::macros::format_description;
use time::{OffsetDateTime, format_description::StaticFormatDescription};

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
    tui,
};

static ORG_DATE_FORMAT: StaticFormatDescription =
    format_description!("[year]-[month]-[day] [weekday repr:short]");

pub(crate) fn sha256<P: AsRef<Path>>(path: P) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut hasher = IoWrapper(Sha256::new());
    io::copy(&mut file, &mut hasher)?;
    Ok(hasher.0.finalize().to_vec())
}

fn get_age_decryptor<P: AsRef<Path>>(
    path: P,
) -> Result<RecipientsDecryptor<ArmoredReader<BufReader<InputReader>>>, JournalError> {
    let s = path.as_ref().to_str().map(std::string::ToString::to_string);
    let input_reader =
        InputReader::new(s).map_err(|_| JournalError::Error(ErrorKind::Decryption))?;
    let decryptor = age::Decryptor::new(ArmoredReader::new(input_reader))
        .map_err(|_| JournalError::Error(ErrorKind::Decryption))?;

    match decryptor {
        age::Decryptor::Passphrase(_) => Err(JournalError::Error(ErrorKind::Decryption)),
        age::Decryptor::Recipients(decryptor) => Ok(decryptor),
    }
}

fn decrypt<P: AsRef<Path>>(
    input_file: P,
    output_file: P,
    identities: &[Box<dyn age::Identity>],
) -> Result<(), JournalError> {
    let output_file_mode: u32 = 0o600;
    let decryptor =
        get_age_decryptor(input_file).map_err(|_| JournalError::Error(ErrorKind::Decryption))?;
    decryptor
        .decrypt(identities.iter().map(|i| i.as_ref() as &dyn age::Identity))
        .map_err(|_| JournalError::Error(ErrorKind::Decryption))
        .and_then(|mut plaintext_reader| {
            let output = output_file
                .as_ref()
                .to_str()
                .map(std::string::ToString::to_string);
            let mut ciphertext_writer =
                OutputWriter::new(output, true, OutputFormat::Unknown, output_file_mode, false)
                    .map_err(|_| JournalError::Error(ErrorKind::Decryption))?;
            io::copy(&mut plaintext_reader, &mut ciphertext_writer)
                .map_err(|_| JournalError::Error(ErrorKind::Decryption))?;
            Ok(())
        })
}

fn editor_from_string(editor: &str) -> Result<(String, Vec<String>), JournalError> {
    let parts = split(editor).map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;
    let prog = &parts[0];
    let args = &parts[1..];
    Ok((prog.to_string(), args.to_vec()))
}

fn get_editor_command(
    configured_editor: Option<String>,
) -> Result<(String, Vec<String>), JournalError> {
    match configured_editor {
        Some(ref editor) => editor_from_string(editor),
        None => {
            if let Some(visual_env) = env::var_os("VISUAL") {
                let pager = visual_env
                    .into_string()
                    .map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;
                editor_from_string(&pager)
            } else if let Some(editor_env) = env::var_os("EDITOR") {
                let pager = editor_env
                    .into_string()
                    .map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;
                editor_from_string(&pager)
            } else {
                let args: Vec<String> = vec![];
                Ok(("nano".to_string(), args))
            }
        }
    }
}

pub struct EntryEditor {
    identity: String,
    journal_path: String,
    editor: String,
    editor_args: Vec<String>,
}

impl EntryEditor {
    pub fn new(config: &Config) -> Result<Self, JournalError> {
        let (editor, editor_args) =
            get_editor_command(config.editor.as_ref().map(|editor| editor.to_owned()))?;

        let entry_editor = EntryEditor {
            identity: config.identity.to_owned(),
            editor,
            editor_args,
            journal_path: config.journal_path.to_owned(),
        };
        Ok(entry_editor)
    }

    fn load_identities(&self) -> Result<Vec<Box<dyn age::Identity>>, JournalError> {
        let mut stdin_guard = StdinGuard::new(true);
        let identities =
            age::cli_common::read_identities(vec![self.identity.clone()], None, &mut stdin_guard)
                .map_err(|_| JournalError::Error(ErrorKind::Decryption))?;

        Ok(identities)
    }

    fn encrypt<P: AsRef<Path>>(
        &self,
        encrypted_path: P,
        decrypted_path: Option<P>,
        text: Option<&str>,
    ) -> Result<(), JournalError> {
        let file = if let Some(path) = decrypted_path {
            let stdin_file =
                File::open(path).map_err(|_| JournalError::Error(ErrorKind::Decryption))?;
            Some(stdin_file)
        } else if text.is_some() {
            None
        } else {
            return Err(JournalError::Error(ErrorKind::Subprocess));
        };
        let mut command = Command::new("ragenix");
        command
            .args([
                "--identity",
                &self.identity,
                "--rules",
                "./rules.nix",
                "--editor",
                "-",
                "--edit",
            ])
            .current_dir(&self.journal_path)
            .arg(encrypted_path.as_ref());

        if let Some(stdin_file) = file {
            command.stdin(stdin_file);
        } else if text.is_some() {
            command.stdin(Stdio::piped());
        } else {
            return Err(JournalError::Error(ErrorKind::Subprocess));
        }

        if let Ok(mut child) = command.spawn() {
            if let Some(stdin_text) = text {
                let subproc_stdin = stdin_text.to_owned();
                let mut stdin = child
                    .stdin
                    .take()
                    .expect("child stdin should be retrievable");
                std::thread::spawn(move || {
                    let _ = stdin
                        .write_all(subproc_stdin.as_bytes())
                        .map_err(|_| JournalError::Error(ErrorKind::Subprocess));
                });
            }
            if let Ok(status) = child.wait() {
                if !status.success() {
                    println!("ragenix exited with code {:?}", status.code());
                    return Err(JournalError::Error(ErrorKind::Subprocess));
                }
            } else {
                return Err(JournalError::Error(ErrorKind::Subprocess));
            }
        } else {
            return Err(JournalError::Error(ErrorKind::Subprocess));
        }
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
                    println!("Editor exited with code {:?}", status.code());
                    return Err(JournalError::Error(ErrorKind::Subprocess));
                }
            } else {
                return Err(JournalError::Error(ErrorKind::Subprocess));
            }
        } else {
            return Err(JournalError::Error(ErrorKind::Subprocess));
        }

        Ok(())
    }

    fn decrypt_and_edit_entry(&self, encrypted_file_path: &PathBuf) -> Result<bool, JournalError> {
        let dir = tempfile::tempdir().map_err(|_| JournalError::Error(ErrorKind::TemporaryFile))?;
        fs::set_permissions(&dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::Error(ErrorKind::TemporaryFile))?;

        let identities = self.load_identities()?;
        let input_path = dir.path().join("input.org");

        decrypt(encrypted_file_path, &input_path.clone(), &identities)?;

        let pre_edit_hash =
            sha256(&input_path).map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        self.spawn_editor(&input_path)?;

        // fixme: We don't want to discard changes in this case!
        let post_edit_hash =
            sha256(&input_path).map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        if pre_edit_hash == post_edit_hash {
            return Ok(false);
        }

        self.encrypt(encrypted_file_path, Some(&input_path), None)?;

        Ok(true)
    }

    fn create_new_entry(&self, encrypted_file_path: &PathBuf) -> Result<(), JournalError> {
        let now =
            OffsetDateTime::now_local().map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        let now_org_str = now
            .format(ORG_DATE_FORMAT)
            .map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        let text = format!(
            "#+SETUPFILE: style.css.org

* [{}]
",
            now_org_str
        );

        self.encrypt(encrypted_file_path, None, Some(&text))?;

        Ok(())
    }

    fn create_or_edit_entry(&self, filename: &str) -> Result<bool, JournalError> {
        let display_editor = if self.editor_args.is_empty() {
            self.editor.to_string()
        } else {
            format!("{} {}", self.editor, join(&self.editor_args))
        };

        eprintln!("Opening {} with {}", filename, display_editor);

        let mut encrypted_file_path = PathBuf::new();
        encrypted_file_path.push(&self.journal_path);
        encrypted_file_path.push(filename);

        let new_file = if !encrypted_file_path.exists() {
            self.create_new_entry(&encrypted_file_path)?;
            true
        } else {
            false
        };

        let edited = self.decrypt_and_edit_entry(&encrypted_file_path)?;
        if !edited {
            println!("{} wasn't changed, skipping re-encryption.", filename);
        }
        Ok(new_file || edited)
    }

    pub fn edit(&self, entry: &str) -> Result<bool, JournalError> {
        let filename = format!("{}.age", entry);

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
}
