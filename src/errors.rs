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

#[derive(Clone, Copy, Debug)]
pub enum JournalError {
    Error(ErrorKind),
}

impl JournalError {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match &self {
            JournalError::Error(err) => err.as_str(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    Subprocess,
    // TemporaryFile,
    TerminalInit,
    RepositoryWalkError,
}

impl ErrorKind {
    pub fn as_str(&self) -> &str {
        match *self {
            ErrorKind::Subprocess => "Failed to execute subprocess",
            // ErrorKind::TemporaryFile => "Failed to write temporary file",
            ErrorKind::TerminalInit => "Failed to reinitialize terminal",
            ErrorKind::RepositoryWalkError => "Failed to walk repository commits",
        }
    }
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            JournalError::Error(err) => write!(f, "{:?}", err.as_str()),
        }
    }
}

impl Error for JournalError {
    fn description(&self) -> &str {
        match &self {
            JournalError::Error(err) => err.as_str(),
        }
    }
}
