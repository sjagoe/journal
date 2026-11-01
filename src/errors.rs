use std::error::Error;
use std::fmt;
use std::panic;

use color_eyre::{config::HookBuilder, eyre};

use crate::tui;

pub fn install_hooks() -> color_eyre::Result<()> {
    let (panic_hook, eyre_hook) = HookBuilder::default().into_hooks();

    let panic_hook = panic_hook.into_panic_hook();
    panic::set_hook(Box::new(move |panic_info| {
        tui::restore().unwrap();
        panic_hook(panic_info);
    }));

    let eyre_hook = eyre_hook.into_eyre_hook();
    eyre::set_hook(Box::new(
        move |error: &(dyn std::error::Error + 'static)| {
            tui::restore().unwrap();
            eyre_hook(error)
        },
    ))?;

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum JournalError {
    Error(ErrorKind),
}

impl JournalError {
    #[allow(dead_code)]
    pub fn as_str(&self) -> String {
        match &self {
            JournalError::Error(err) => err.as_str(),
        }
    }

    pub fn chain(errors: &[ErrorKind]) -> Self {
        let chain = ChainedError {
            errors: errors.to_vec(),
        };
        JournalError::Error(ErrorKind::Chained(chain))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    Chained(ChainedError),
    Wrapped(String),
    Subprocess,
    TemporaryFile,
    TerminalInit,
    Decryption,
    Encryption,
    EncryptionPipeline,
    DecryptionPipeline,
    DecryptionPassphrase,
    InvalidMode,
    NewEntry,
    GetCurrentTime,
    FormatDateTime,
    TextEntryFailed,
    FileReadError,
    FileWriteError,
    CommandParseError,
    EditingEntry,
    ExportingEntry,
    // EntryExists,
    AppExit,
    // GitError,
    GitIndexError,
    FileMissingError,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChainedError {
    errors: Vec<ErrorKind>,
}

impl ErrorKind {
    pub fn description(&self) -> &str {
        match *self {
            ErrorKind::Chained(_) => "Multiple errors",
            ErrorKind::Wrapped(_) => "Wrapped error",
            ErrorKind::Subprocess => "Failed to execute subprocess",
            ErrorKind::TemporaryFile => "Failed to write temporary file",
            ErrorKind::TerminalInit => "Failed to reinitialize terminal",
            ErrorKind::Decryption => "Failed to decrypt entry",
            ErrorKind::Encryption => "Failed to encrypt entry",
            ErrorKind::EncryptionPipeline => "Failed to construct encryption pipeline",
            ErrorKind::InvalidMode => "App entered invalid state",
            ErrorKind::GetCurrentTime => "Unable to fetch system time in local timezone",
            ErrorKind::FormatDateTime => "Failed to format time to string",
            ErrorKind::TextEntryFailed => "Text entry failed",
            ErrorKind::FileReadError => "Failed to read file",
            ErrorKind::FileWriteError => "Failed to write file",
            ErrorKind::DecryptionPipeline => "Failed to create decryption pipeline",
            ErrorKind::DecryptionPassphrase => "Passphrases are not supported",
            ErrorKind::CommandParseError => "Failed to parse command",
            ErrorKind::EditingEntry => "Failed to edit file",
            ErrorKind::ExportingEntry => "Failed to export the entry",
            ErrorKind::NewEntry => "Failed to create new entry",
            // ErrorKind::EntryExists => "New entry is already tracked",
            ErrorKind::AppExit => "Normal application exit",
            // ErrorKind::GitError => "Error performing git action",
            ErrorKind::GitIndexError => "Failed to reference the git index",
            ErrorKind::FileMissingError => "File to be edited does not exist,",
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            ErrorKind::Chained(chained) => {
                if chained.errors.len() == 1
                    && let Some(error) = chained.errors.first()
                {
                    return error.description().to_string();
                }
                let mut message = String::new();
                for error in chained.errors.iter() {
                    let end = match error {
                        ErrorKind::Chained(_) => "",
                        _ => "\n  ",
                    };
                    let submsg = format!("{}{}", error.as_str(), end);
                    message.push_str(&submsg);
                }
                message
            }
            ErrorKind::Wrapped(value) => value.to_string(),
            _ => self.description().to_string(),
        }
    }
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JournalError::Error(err) => write!(f, "{}", err.as_str()),
        }
    }
}

impl Error for JournalError {
    fn description(&self) -> &str {
        match &self {
            JournalError::Error(err) => err.description(),
        }
    }
}
