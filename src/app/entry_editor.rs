use std::{
    env,
    ffi::OsStr,
    fs::{self, File, read_dir},
    io::{self, BufReader, Write},
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
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
use tempfile::TempDir;
use time::macros::format_description;
use time::{OffsetDateTime, format_description::StaticFormatDescription};

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
    journal::{EntryKind, entry_filename},
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
        InputReader::new(s).map_err(|_| JournalError::Error(ErrorKind::FileReadError))?;
    let decryptor = age::Decryptor::new(ArmoredReader::new(input_reader)).map_err(|_| {
        JournalError::chain(&[ErrorKind::FileReadError, ErrorKind::DecryptionPipeline])
    })?;

    match decryptor {
        age::Decryptor::Passphrase(_) => Err(JournalError::Error(ErrorKind::DecryptionPassphrase)),
        age::Decryptor::Recipients(decryptor) => Ok(decryptor),
    }
}

fn decrypt<P: AsRef<Path>>(
    input_file: P,
    output_file: P,
    identities: &[Box<dyn age::Identity>],
) -> Result<(), JournalError> {
    let output_file_mode: u32 = 0o600;
    let decryptor = get_age_decryptor(input_file)?;
    decryptor
        .decrypt(identities.iter().map(|i| i.as_ref() as &dyn age::Identity))
        .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))
        .and_then(|mut plaintext_reader| {
            let output = output_file
                .as_ref()
                .to_str()
                .map(std::string::ToString::to_string);
            let mut ciphertext_writer =
                OutputWriter::new(output, true, OutputFormat::Unknown, output_file_mode, false)
                    .map_err(|_| JournalError::Error(ErrorKind::FileWriteError))?;
            io::copy(&mut plaintext_reader, &mut ciphertext_writer)
                .map_err(|_| JournalError::Error(ErrorKind::FileWriteError))?;
            Ok(())
        })
}

fn editor_from_string(editor: &str) -> Result<(String, Vec<String>), JournalError> {
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
        Some(ref editor) => editor_from_string(editor),
        None => {
            if let Some(visual_env) = env::var_os("VISUAL") {
                let pager = visual_env.into_string().map_err(|_| {
                    JournalError::chain(&[
                        ErrorKind::Subprocess,
                        ErrorKind::CommandParseError,
                        ErrorKind::Wrapped("Could not parse VISUAL env var".to_string()),
                    ])
                })?;
                editor_from_string(&pager)
            } else if let Some(editor_env) = env::var_os("EDITOR") {
                let pager = editor_env.into_string().map_err(|_| {
                    JournalError::chain(&[
                        ErrorKind::Subprocess,
                        ErrorKind::CommandParseError,
                        ErrorKind::Wrapped("Could not parse EDITOR env var".to_string()),
                    ])
                })?;
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
                File::open(path).map_err(|_| JournalError::Error(ErrorKind::FileReadError))?;
            Some(stdin_file)
        } else if text.is_some() {
            None
        } else {
            panic!("encrypt() MUST have exactly one of decrypted_path or text passed")
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
            panic!("This condition is already checked; we shouldn't get here")
        }

        if let Ok(mut child) = command.spawn() {
            if let Some(stdin_text) = text {
                let subproc_stdin = stdin_text.to_owned();
                let mut stdin = child
                    .stdin
                    .take()
                    .expect("child stdin should be retrievable");
                std::thread::spawn(move || {
                    let _ = stdin.write_all(subproc_stdin.as_bytes()).map_err(|_| {
                        JournalError::chain(&[
                            ErrorKind::Subprocess,
                            ErrorKind::FileWriteError,
                            ErrorKind::Wrapped(
                                "Failed to send data to ragenix to encrypt".to_string(),
                            ),
                        ])
                    });
                });
            }
            if let Ok(status) = child.wait() {
                if !status.success() {
                    let message = format!("ragenix exited with code {:?}", status.code());
                    return Err(JournalError::chain(&[
                        ErrorKind::Subprocess,
                        ErrorKind::Wrapped(message),
                    ]));
                }
            } else {
                return Err(JournalError::chain(&[
                    ErrorKind::Subprocess,
                    ErrorKind::Wrapped("Failed to wait for ragenix to complete".to_string()),
                ]));
            }
        } else {
            return Err(JournalError::chain(&[
                ErrorKind::Subprocess,
                ErrorKind::Wrapped("Failed to execute ragenix to encrypt entry".to_string()),
            ]));
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
        let dir = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;
        fs::set_permissions(&dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        let identities = self.load_identities()?;
        let input_path = dir.path().join("input.org");

        decrypt(encrypted_file_path, &input_path.clone(), &identities).map_err(
            |err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
            },
        )?;

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
}

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
}

impl EntryExporter {
    pub fn new(config: &Config, kind: &EntryKind, entry: &str) -> Result<Self, JournalError> {
        let filename = entry_filename(kind, entry);
        let dir = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;
        let entry_editor = EntryEditor::new(config)?;
        let exporter = Self {
            filename,
            progress: 0.0,
            entry_editor,
            dir,
            export_process: None,
        };
        Ok(exporter)
    }

    fn check_progress(&mut self) -> Result<(), JournalError> {
        let msg = format!("checking process {:?}", &self.export_process);
        self.log(&msg);
        self.log_dir();
        if let Some(ref mut export_process) = self.export_process {
            match export_process {
                ExportProcess::Export(child) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        let msg = format!("child status {:?}", &status);
                        self.log(&msg);
                        if status.success() {
                            self.export_process = None;
                            self.spawn_browser(&self.exported_path())?;
                        } else {
                            let message = format!("Editor exited with code {:?}", status.code());
                            self.log(&message);
                            return Err(JournalError::chain(&[
                                ErrorKind::Subprocess,
                                ErrorKind::Wrapped(message),
                            ]));
                        }
                    } else {
                        let msg = format!("child still running");
                        self.log(&msg);
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
            if self.progress >= 1.0 {
                return true;
            } else {
                let duration: f64 = tick_duration.into();
                let new = self.progress + (duration / 1000.0);
                self.progress = new;
                false
            }
        };
        if let Some(ref export_process) = self.export_process {
            match export_process {
                ExportProcess::Export(_) => {
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
        if self.progress >= 1.0 {
            1.0
        } else {
            self.progress
        }
    }

    fn log(&self, line: &str) {
        let mut location = PathBuf::new();
        location.push(&self.entry_editor.journal_path);
        location.push("export.log");
        let mut file = File::options()
            .write(true)
            .create(true)
            .append(true)
            .open(&location)
            .expect("Unable to open entries data");
        writeln!(&mut file, "{line}").unwrap();
        file.flush().unwrap();
    }

    fn log_dir(&self) {
        let path = self.dir.path();
        let msg = format!("Directory {:?}", &path);
        self.log(&msg);
        if let Some(dir) = read_dir(&path).ok() {
            for entry in dir {
                if let Some(item) = entry.ok() {
                    let msg = format!("  dir entry {:?}", item);
                    self.log(&msg);
                }
            }
        }
        self.log("");
    }

    fn spawn_browser(&mut self, exported_path: &PathBuf) -> Result<(), JournalError> {
        let mut command = Command::new("xdg-open");
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
            .current_dir(&self.dir.path())
            .env("_JOURNAL_EMACS_EMBEDDED", "t");

        self.log("Creating emacs export process");

        self.log_dir();

        if let Ok(child) = command.spawn() {
            self.export_process = Some(ExportProcess::Export(child));
            let msg = format!("started process {:?}", &self.export_process);
            self.log(&msg);
            self.log_dir();
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
        encrypted_file_path.push(&journal_path);
        encrypted_file_path.push(&self.filename);

        let mut style_path = PathBuf::new();
        style_path.push(&journal_path);
        style_path.push("style.css.org");

        fs::set_permissions(&self.dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        self.log_dir();

        let identities = self.entry_editor.load_identities()?;
        let input_path = self.decrypted_path();

        let target_style_path = self.dir.path().join("style.css.org");

        decrypt(encrypted_file_path, input_path.clone(), &identities).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
        })?;

        self.log_dir();

        symlink(&style_path, &target_style_path).map_err(|_| {
            JournalError::chain(&[
                ErrorKind::FileWriteError,
                ErrorKind::Wrapped("Failed to symlink stylesheel".to_string()),
            ])
        })?;

        self.log_dir();

        self.spawn_exporter(&input_path)?;

        self.log_dir();

        // let exported_path =

        // self.spawn_browser(&exported_path)?;

        // // FIXME this is terrible.
        // // We need to background this and wait for it to complete
        // // The sleep for firefox to read the file before we delete it is also not great...
        // std::thread::sleep(std::time::Duration::from_millis(500));

        Ok(())
    }
}
