use std::{
    fs::OpenOptions,
    io::{self, BufReader, Read},
    path::Path,
};

use age::{
    armor::{ArmoredReader, ArmoredWriter, Format},
    cli_common::{
        StdinGuard,
        file_io::{InputReader, OutputFormat, OutputWriter},
    },
};

use crate::errors::{ErrorKind, JournalError};

pub(super) fn load_identities(identities: &[String]) -> Result<Vec<Box<dyn age::Identity>>, JournalError> {
    let mut stdin_guard = StdinGuard::new(true);
    let identities =
        age::cli_common::read_identities(identities.to_vec(), None, &mut stdin_guard)
            .map_err(|_| JournalError::Error(ErrorKind::Decryption))?;

    Ok(identities)
}

pub(super) fn get_age_decryptor<P: AsRef<Path>>(
    path: P,
) -> Result<age::Decryptor<ArmoredReader<BufReader<InputReader>>>, JournalError> {
    let s = path.as_ref().to_str().map(std::string::ToString::to_string);
    let input_reader =
        InputReader::new(s).map_err(|_| JournalError::Error(ErrorKind::FileReadError))?;
    let decryptor = age::Decryptor::new(ArmoredReader::new(input_reader)).map_err(|_| {
        JournalError::chain(&[ErrorKind::FileReadError, ErrorKind::DecryptionPipeline])
    })?;

    if decryptor.is_scrypt() {
        return Err(JournalError::Error(ErrorKind::DecryptionPassphrase));
    }

    Ok(decryptor)
}

pub(super) fn decrypt<P: AsRef<Path>>(
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

// let unencrypted_data: BufReader::new(value.as_bytes()),
pub(super) fn encrypt<P: AsRef<Path>>(
    recipient_keys: Vec<String>,
    input: &mut dyn Read,
    encrypted_path: P,
) -> Result<(), JournalError> {
    let mut stdin_guard = StdinGuard::new(true);
    let recipients =
        age::cli_common::read_recipients(recipient_keys, vec![], vec![], None, &mut stdin_guard)
            .map_err(|_| JournalError::chain(&[ErrorKind::EncryptionPipeline]))?;

    let format = Format::AsciiArmor;

    let encryptor = age::Encryptor::with_recipients(recipients.iter().map(|r| r.as_ref() as _))
        .map_err(|_| JournalError::chain(&[ErrorKind::EncryptionPipeline]))?;

    let output = OpenOptions::new()
        .truncate(true)
        .write(true)
        .create(true)
        .open(&encrypted_path)
        .map_err(|_| JournalError::chain(&[ErrorKind::EncryptionPipeline]))?;

    let mut output = encryptor
        .wrap_output(
            ArmoredWriter::wrap_output(output, format)
                .map_err(|_| JournalError::chain(&[ErrorKind::EncryptionPipeline]))?,
        )
        .map_err(|_| JournalError::chain(&[ErrorKind::EncryptionPipeline]))?;

    io::copy(input, &mut output).map_err(|_| JournalError::chain(&[ErrorKind::Encryption]))?;

    output
        .finish()
        .and_then(|armor| armor.finish())
        .map_err(|_| JournalError::chain(&[ErrorKind::Encryption]))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use age::secrecy::ExposeSecret;
    use age::x25519::Identity;

    use super::*;

    #[test]
    fn test_load_identities() {
        let test_identity = Identity::generate();
        let dir = tempfile::tempdir().expect("Unable to create temp dir");

        let mut identity_file = PathBuf::new();
        identity_file.push(&dir);
        identity_file.push("identity");

        let secret_string = test_identity.to_string();
        let identity_secret = secret_string.expose_secret();

        std::fs::write(&identity_file, &identity_secret).unwrap();

        let identity_path_str = identity_file.into_string().unwrap();
        let identities = load_identities(&identity_path_str).unwrap();

        assert_eq!(identities.len(), 1);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = Identity::generate();
        let pubkey = key.to_public().to_string();

        let dir = tempfile::tempdir().expect("Unable to create temp dir");

        let mut path = PathBuf::new();
        path.push(&dir);
        path.push("encrypted");

        let content = "content";
        let mut encrypt_input = BufReader::new(content.as_bytes());

        encrypt(vec![pubkey], &mut encrypt_input, &path).unwrap();

        let mut decrypt_path = PathBuf::new();
        decrypt_path.push(&dir);
        decrypt_path.push("decrypted");

        let mut identities: Vec<Box<dyn age::Identity>> = Vec::new();
        identities.push(Box::new(key));

        decrypt(&path, &decrypt_path, &identities).unwrap();

        let decrypted_content = std::fs::read_to_string(&decrypt_path).unwrap();

        assert_eq!(decrypted_content, content);
    }
}
