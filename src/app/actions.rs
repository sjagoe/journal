use crossterm::event::KeyCode;

use crate::errors::JournalError;

#[derive(Debug)]
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
