use std::{
    env, fs::{self, File}, io::{self, BufReader}, os::unix::fs::PermissionsExt, path::{Path, PathBuf}, process::Command,
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
use shell_words::split;
use sha2::{Digest, Sha256};

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
    tui,
};

pub struct EntryEditor {
    identity: String,
    editor: Option<String>,
    journal_path: String,
    filename: String,
}

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
    let decryptor = get_age_decryptor(input_file)
        .map_err(|_| JournalError::Error(ErrorKind::Decryption))?;
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

impl EntryEditor {
    pub fn new(entry: &str, config: &Config) -> Result<Self, JournalError> {
        let filename = format!("{}.age", entry);

        let editor = match config.editor {
            Some(ref editor) => Some(editor.to_owned()),
            None => None,
        };

        let entry_editor = EntryEditor {
            identity: config.identity.to_owned(),
            editor,
            filename,
            journal_path: config.journal_path.to_owned(),
        };
        Ok(entry_editor)
    }

    fn editor_from_string(&self, editor: &str) -> Result<(String, Vec<String>), JournalError>{
        let parts = split(&editor).map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;
        let prog = &parts[0];
        let args = &parts[1..];
        Ok((prog.to_string(), args.to_vec()))
    }

    fn get_editor_command(&self) -> Result<(String, Vec<String>), JournalError> {
        match self.editor {
            Some(ref editor) => {
                self.editor_from_string(&editor)
            }
            None => {
                if let Some(visual_env) = env::var_os("VISUAL") {
                    let pager = visual_env.into_string().map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;
                    self.editor_from_string(&pager)
                } else if let Some(editor_env) = env::var_os("EDITOR") {
                    let pager = editor_env.into_string().map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;
                    self.editor_from_string(&pager)
                } else {
                    let args: Vec<String> = vec![];
                    Ok(("nano".to_string(), args))
                }
            }
        }
    }

    fn load_identities(&self) -> Result<Vec<Box<dyn age::Identity>>, JournalError> {
        let mut stdin_guard = StdinGuard::new(true);
        let identities =
            age::cli_common::read_identities(vec![self.identity.clone()], None, &mut stdin_guard)
                .map_err(|_| JournalError::Error(ErrorKind::Decryption))?;

        Ok(identities)
    }

    fn encrypt<P: AsRef<Path>>(&self, encrypted_path: P, decrypted_path: P)  -> Result<(), JournalError> {
        let file = File::open(decrypted_path).map_err(|_| JournalError::Error(ErrorKind::Decryption))?;
        let mut command = Command::new("ragenix");
        command.args([
            "--identity",
            &self.identity,
            "--rules",
            "./rules.nix",
            "--editor",
            "-",
            "--edit",
        ])
            .arg(encrypted_path.as_ref())
            .stdin(file)
            .current_dir(&self.journal_path);

        if let Ok(mut child) = command.spawn() {
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

    fn spawn_editor(&self) -> Result<(), JournalError> {
        let dir = tempfile::tempdir().map_err(|_| JournalError::Error(ErrorKind::TemporaryFile))?;
        fs::set_permissions(&dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::Error(ErrorKind::TemporaryFile))?;

        let identities = self.load_identities()?;
        let input_path = dir.path().join("input.org");

        let mut output_path = PathBuf::new();
        output_path.push(&self.journal_path);
        output_path.push(&self.filename);
        decrypt(output_path.clone(), input_path.clone(), &identities)?;

        let pre_edit_hash = sha256(&input_path).map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        let (prog, args) = self.get_editor_command()?;
        let mut command = Command::new(&prog);
        command.args(args);
        command.arg(&input_path);

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

        let post_edit_hash = sha256(&input_path).map_err(|_| JournalError::Error(ErrorKind::Subprocess))?;

        if pre_edit_hash == post_edit_hash {
            println!(
                "{} wasn't changed, skipping re-encryption.",
                self.filename
            );
            return Ok(());
        }

        self.encrypt(output_path, input_path)?;

        Ok(())
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
