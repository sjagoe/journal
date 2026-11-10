use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Error},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use tempfile::TempDir;

use crate::{
    config::Config,
    errors::{ErrorKind, JournalError},
};

use super::{
    Journal,
    age::{decrypt, encrypt, load_identities},
    edit::PublicKeys,
};

pub struct EntryReKey {
    identities: Vec<String>,
    journal_path: String,
    dir: TempDir,
}

impl EntryReKey {
    pub fn new(config: &Config) -> Result<Self, JournalError> {
        let dir = tempfile::tempdir()
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;
        fs::set_permissions(&dir, PermissionsExt::from_mode(0o700))
            .map_err(|_| JournalError::chain(&[ErrorKind::Decryption, ErrorKind::TemporaryFile]))?;

        Ok(Self {
            identities: config.get_identities(),
            journal_path: config.journal_path.to_owned(),
            dir,
        })
    }

    fn encrypt<P: AsRef<Path>, D: AsRef<Path>>(
        &self,
        encrypted_path: P,
        decrypted_path: D,
    ) -> Result<(), JournalError> {
        let public_keys = PublicKeys::new(&self.journal_path)?;

        let mut input = File::open(decrypted_path)
            .map_err(|_| JournalError::Error(ErrorKind::FileReadError))?;
        encrypt(public_keys.keys, &mut input, encrypted_path)?;

        Ok(())
    }

    fn copy_new_file(
        &self,
        new_path: &PathBuf,
        encrypted_file_path: &PathBuf,
    ) -> Result<(), Error> {
        let mut new_file = BufReader::new(fs::File::open(new_path)?);
        let mut stored_file = BufWriter::new(fs::File::create(encrypted_file_path)?);

        let _ = std::io::copy(&mut new_file, &mut stored_file)?;

        Ok(())
    }

    fn re_key_file(
        &mut self,
        encrypted_file_path: &PathBuf,
        new_key_path: &Path,
    ) -> Result<(), JournalError> {
        let identities = load_identities(&self.identities)?;
        let input_path = self.dir.path().join("input.org");

        println!("Re-keying {:?}", encrypted_file_path.display());
        decrypt(encrypted_file_path, &input_path, &identities).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
        })?;

        let output_path = self.dir.path().join("output.age");

        self.encrypt(&output_path, &input_path)
            .map_err(|err| match err {
                JournalError::Error(e) => JournalError::chain(&[ErrorKind::Encryption, e]),
            })?;

        // FIXME
        let new_keys: Vec<String> = vec![new_key_path.as_os_str().to_string_lossy().to_string()];
        // We test with only the new identity to ensure the rekey has worked
        let new_identities = load_identities(&new_keys)?;
        decrypt(&output_path, &input_path, &new_identities).map_err(|err| match err {
            JournalError::Error(e) => JournalError::chain(&[ErrorKind::Decryption, e]),
        })?;

        self.copy_new_file(&output_path, encrypted_file_path)
            .map_err(|e| JournalError::Error(ErrorKind::Wrapped(e.to_string())))?;

        Ok(())
    }

    pub fn re_key_repository(
        &mut self,
        journal: &Journal,
        new_key_path: &Path,
    ) -> Result<(), JournalError> {
        for item in journal.iter_files() {
            let mut encrypted_path = PathBuf::new();
            encrypted_path.push(&self.journal_path);
            encrypted_path.push(&item);
            self.re_key_file(&encrypted_path, new_key_path)?;
        }
        Ok(())
    }
}
