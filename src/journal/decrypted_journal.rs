use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{BufReader, BufWriter},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use tempfile::TempDir;

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
};

use super::{
    Journal,
    age::{decrypt, encrypt, load_identities},
    entry_filename,
};

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

pub struct DecryptedJournal {
    pub(super) dir: TempDir,
    journal_path: String,
}

impl DecryptedJournal {
    pub fn new(config: &Config, journal: &Journal) -> Result<Self, JournalError> {
        let dir = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;
        fs::set_permissions(&dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        decrypt_journal(&dir, &config.get_identities(), journal)?;

        Ok(DecryptedJournal {
            dir,
            journal_path: journal.path.clone(),
        })
    }

    pub(super) fn encrypt_path<D: AsRef<Path>>(
        &self,
        filename: &str,
        decrypted_path: D,
    ) -> Result<(), JournalError> {
        let final_path = self.encrypted_file_path(filename);
        let temporary_path = final_path.with_added_extension("tmp");

        let public_keys = PublicKeys::new(&self.journal_path)?;

        let mut input = File::open(decrypted_path)
            .map_err(|_| JournalError::Error(ErrorKind::FileReadError))?;
        encrypt(public_keys.keys, &mut input, &temporary_path)?;

        std::fs::rename(&temporary_path, &final_path)
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        Ok(())
    }

    pub(super) fn encrypt_text(&self, filename: &str, text: &str) -> Result<(), JournalError> {
        let final_path = self.encrypted_file_path(filename);
        let temporary_path = final_path.with_added_extension("tmp");

        let public_keys = PublicKeys::new(&self.journal_path)?;

        let mut input = BufReader::new(text.as_bytes());
        encrypt(public_keys.keys, &mut input, &temporary_path)?;

        std::fs::rename(&temporary_path, &final_path)
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        Ok(())
    }

    fn encrypted_file_path(&self, filename: &str) -> PathBuf {
        PathBuf::from(&self.journal_path).join(filename)
    }

    pub(super) fn item_path(&self, filename: &str) -> PathBuf {
        decrypted_file_path(self.dir.path(), filename)
    }

    pub(super) fn edit_item(&self, filename: &str) -> Result<PathBuf, JournalError> {
        let item_path = self.item_path(filename);

        if !item_path.exists() {
            return Err(JournalError::Error(ErrorKind::FileMissingError));
        }

        fs::set_permissions(&item_path, PermissionsExt::from_mode(0o600))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        Ok(item_path)
    }

    pub(super) fn encrypted_exists(&self, filename: &str) -> bool {
        self.encrypted_file_path(filename).exists()
    }

    pub fn re_key_file<P: AsRef<OsStr>>(
        &self,
        filename: &str,
        new_key: P,
    ) -> Result<(), JournalError> {
        let dir = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;
        fs::set_permissions(&dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        println!("Re-keying {:?}", filename);

        let item_path = self.item_path(filename);

        let final_path = self.encrypted_file_path(filename);
        let encrypted_path = final_path.with_added_extension("rekey");

        let public_keys = PublicKeys::new(&self.journal_path)?;

        let mut input =
            File::open(&item_path).map_err(|_| JournalError::Error(ErrorKind::FileReadError))?;
        encrypt(public_keys.keys, &mut input, &encrypted_path)?;

        // FIXME
        let new_keys: Vec<String> = vec![new_key.as_ref().to_string_lossy().to_string()];
        // We test with only the new identity to ensure the rekey has worked
        let new_identities = load_identities(&new_keys)?;
        decrypt(&encrypted_path, &item_path, &new_identities).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
        })?;

        std::fs::rename(&encrypted_path, &final_path)
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        Ok(())
    }
}

fn copy_file<F: AsRef<Path>, T: AsRef<Path>>(from: F, to: T) -> Result<(), std::io::Error> {
    let mut from_buf = BufReader::new(fs::File::open(from)?);
    let mut to_buf = BufWriter::new(fs::File::create(to)?);

    let _ = std::io::copy(&mut from_buf, &mut to_buf)?;

    Ok(())
}

fn decrypted_file_path<D: AsRef<Path>, F: AsRef<OsStr>>(dir: D, filename: F) -> PathBuf {
    let mut decrypted = PathBuf::from(dir.as_ref());
    decrypted.push(filename.as_ref());
    decrypted.set_extension("org");
    decrypted
}

fn decrypt_item<D: AsRef<Path>, E: AsRef<Path>>(
    dir: D,
    encrypted_path: E,
    identities: &Vec<Box<dyn age::Identity>>,
) -> Result<PathBuf, JournalError> {
    let decrypted = decrypted_file_path(dir, encrypted_path.as_ref().file_name().unwrap());

    decrypt(encrypted_path, &decrypted, identities).map_err(|err| match err {
        JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
    })?;

    Ok(decrypted)
}

fn copy_style<D: AsRef<Path>>(journal_path: &str, dir: D) -> Result<(), JournalError> {
    let style_name = "style.css.org";
    let mut style_path = PathBuf::from(&journal_path);
    style_path.push(style_name);

    copy_file(&style_path, dir.as_ref().join(style_name))
        .map_err(|err| JournalError::Error(ErrorKind::Wrapped(err.to_string())))?;

    Ok(())
}

fn decrypt_journal<D: AsRef<Path>>(
    dir: D,
    identities: &[String],
    journal: &Journal,
) -> Result<(), JournalError> {
    let identities = load_identities(identities)?;

    copy_style(&journal.path, &dir)?;

    for section in journal.sections() {
        let kind_path = dir.as_ref().join(String::from(&section.kind));

        std::fs::create_dir(&kind_path).unwrap();
        fs::set_permissions(&kind_path, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        copy_style(&journal.path, &kind_path)?;

        for entry_name in journal.section(&section.kind).iter_entries() {
            let mut entry_path = PathBuf::from(&journal.path);
            entry_path.push(entry_filename(&section.kind, entry_name));
            let decrypted_path = decrypt_item(&kind_path, &entry_path, &identities)?;

            fs::set_permissions(&decrypted_path, PermissionsExt::from_mode(0o400)).map_err(
                |_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]),
            )?;
        }
    }

    Ok(())
}
