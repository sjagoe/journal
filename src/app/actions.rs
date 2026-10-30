use std::str::FromStr;

use Action::ExportEntry;
use crossterm::event::KeyCode;

use crate::errors::{ErrorKind, JournalError};

#[derive(Clone, Debug)]
pub(super) enum Action {
    AppExit,
    DisplayError(JournalError),

    ExportEntry,
    NameNewEntry,
    CreateEntry(String),
    ActivateSelectedEntry,

    Refresh,

    NavigateNext,
    NavigatePrevious,
    NavigatePageUp,
    NavigatePageDown,
    NavigateFirst,
    NavigateLast,
    NavigateSwitchPane,
    NavigateBack,

    GitStageFile,
    GitUnstageFile,
    GitCommitMessage,
    GitPerformCommit(String),

    TextEntry(KeyCode),
    ClearTextEntry,

    NoOp,
}

impl FromStr for Action {
    type Err = JournalError;

    fn from_str(value: &str) -> Result<Action, Self::Err> {
        let result = match value {
            "AppExit" => Action::AppExit,

            "ExportEntry" => Action::ExportEntry,
            "NameNewEntry" => Action::NameNewEntry,
            "ActivateSelectedEntry" => Action::ActivateSelectedEntry,

            "Refresh" => Action::Refresh,

            "NavigateNext" => Action::NavigateNext,
            "NavigatePrevious" => Action::NavigatePrevious,
            "NavigatePageUp" => Action::NavigatePageUp,
            "NavigatePageDown" => Action::NavigatePageDown,
            "NavigateFirst" => Action::NavigateFirst,
            "NavigateLast" => Action::NavigateLast,
            "NavigateSwitchPane" => Action::NavigateSwitchPane,
            "NavigateBack" => Action::NavigateBack,

            "GitStageFile" => Action::GitStageFile,
            "GitUnstageFile" => Action::GitUnstageFile,
            "GitCommitMessage" => Action::GitCommitMessage,

            "ClearTextEntry" => Action::ClearTextEntry,

            _ => return Err(JournalError::Error(ErrorKind::Wrapped("fixme failed to convert action".to_string()))),
        };
        Ok(result)
    }
}
