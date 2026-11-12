use std::{
    fmt::{self, Display},
    str::FromStr,
};

use crossterm::event::KeyEvent;

use crate::errors::{ErrorKind, JournalError};

use super::InputMode;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Action {
    AppExit,
    DisplayError(JournalError),

    ExportEntry,
    ExportJournal,
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

    TextEntry(KeyEvent),
    ClearTextEntry,

    NextMode(InputMode),

    NoOp,
}

impl Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let result = match self {
            Action::AppExit => "AppExit",

            Action::ExportEntry => "ExportEntry",
            Action::ExportJournal => "ExportJournal",
            Action::NameNewEntry => "NameNewEntry",
            Action::ActivateSelectedEntry => "ActivateSelectedEntry",

            Action::Refresh => "Refresh",

            Action::NavigateNext => "NavigateNext",
            Action::NavigatePrevious => "NavigatePrevious",
            Action::NavigatePageUp => "NavigatePageUp",
            Action::NavigatePageDown => "NavigatePageDown",
            Action::NavigateFirst => "NavigateFirst",
            Action::NavigateLast => "NavigateLast",
            Action::NavigateSwitchPane => "NavigateSwitchPane",
            Action::NavigateBack => "NavigateBack",

            Action::GitStageFile => "GitStageFile",
            Action::GitUnstageFile => "GitUnstageFile",
            Action::GitCommitMessage => "GitCommitMessage",

            Action::ClearTextEntry => "ClearTextEntry",

            Action::DisplayError(_) => "DisplayError",
            Action::CreateEntry(_) => "CreateEntry",
            Action::GitPerformCommit(_) => "GitPerformCommit",
            Action::TextEntry(_) => "TextEntry",
            Action::NextMode(_) => "NextMode",

            Action::NoOp => "NoOp",
        };
        write!(f, "{}", result)
    }
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

            _ => {
                return Err(JournalError::Error(ErrorKind::Wrapped(
                    "fixme failed to convert action".to_string(),
                )));
            }
        };
        Ok(result)
    }
}
