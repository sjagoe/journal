use std::{
    io::{self, BufReader},
    path::Path,
};

use age::{
    armor::ArmoredReader,
    cli_common::{
        StdinGuard,
        file_io::{InputReader, OutputFormat, OutputWriter},
    },
    decryptor::RecipientsDecryptor,
};

use crate::errors::{ErrorKind, JournalError};

pub(super) fn load_identities(identity: &str) -> Result<Vec<Box<dyn age::Identity>>, JournalError> {
    let mut stdin_guard = StdinGuard::new(true);
    let identities =
        age::cli_common::read_identities(vec![identity.to_string()], None, &mut stdin_guard)
            .map_err(|_| JournalError::Error(ErrorKind::Decryption))?;

    Ok(identities)
}

pub(super) fn get_age_decryptor<P: AsRef<Path>>(
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
