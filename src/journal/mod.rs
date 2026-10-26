use std::path::Path;
use std::process::Command;
use std::path::PathBuf;

use git2::{Index, Repository, Status};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

mod age;
mod changes;
pub(crate) mod edit;
pub(crate) mod export;

use crate::errors::{ErrorKind, JournalError};

use self::changes::LastChanged;

const ENTRY_DIRECTORY_DOCUMENT: &str = "documents";
const ENTRY_DIRECTORY_GENERAL: &str = "general";
const ENTRY_DIRECTORY_NARRATIVE: &str = "narrative";

#[derive(Clone, Debug)]
pub enum EntryKind {
    General,
    Narrative,
}

impl From<&EntryKind> for String {
    fn from(val: &EntryKind) -> String {
        match val {
            EntryKind::General => ENTRY_DIRECTORY_GENERAL.to_owned(),
            EntryKind::Narrative => ENTRY_DIRECTORY_NARRATIVE.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct SerializedEntries {
    documents: Vec<String>,
    general: Vec<String>,
    narrative: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct JournalEntry {
    pub name: String,
    pub status: Option<Status>,
    pub last_modified: Option<OffsetDateTime>,
}

impl JournalEntry {
    fn new(
        name: &str,
        status: Option<Status>,
        last_modified: Option<OffsetDateTime>,
    ) -> JournalEntry {
        JournalEntry {
            name: name.to_owned(),
            status,
            last_modified,
        }
    }
}

#[derive(Clone, Debug)]
pub struct JournalSection {
    pub _kind: EntryKind,
    pub entries: Vec<JournalEntry>,
}

impl JournalSection {
    fn new(
        kind: EntryKind,
        repo: &Repository,
        changes: &LastChanged,
        items: &[String],
    ) -> JournalSection {
        let mut entries: Vec<JournalEntry> = items
            .iter()
            .map(|name| {
                let filename = entry_filename(&kind, name);
                let path = Path::new(&filename);
                let status = repo.status_file(path).ok();
                let time = changes.get(&filename).copied();
                JournalEntry::new(name, status, time)
            })
            .collect();
        entries.sort_by(|a, b| b.name.cmp(&a.name));

        JournalSection {
            _kind: kind,
            entries,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl std::ops::Index<usize> for JournalSection {
    type Output = JournalEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

#[derive(Clone, Debug)]
pub struct Journal {
    _documents: Vec<String>,
    pub general: JournalSection,
    pub narrative: JournalSection,
}

pub fn entry_filename(kind: &EntryKind, name: &str) -> String {
    let directory = String::from(kind);
    format!("{}/{}.age", directory, name)
}

fn nix_eval_rules(path: &str) -> SerializedEntries {
    let mut entries: SerializedEntries = SerializedEntries::default();
    let mut command = Command::new("nix");
    command
        .args([
            "eval",
            "--no-net",
            "--json",
            "--file",
            "./rules.nix",
        ])
        .current_dir(path);
    let entry_match = Regex::new(r"^(?<type>[a-z]+)/(?<name>[-_a-zA-Z0-9]+)\.age$").unwrap();
    if let Ok(result) = command.output()
        && let Ok(stdout) = str::from_utf8(&result.stdout)
            && let Value::Object(value) = serde_json::from_str(stdout).expect("Failed to parse `nix evaluate` output") {
                for entry_path in value.keys() {
                    if let Some(captures) = entry_match.captures(entry_path) {
                        let entry_type = captures.name("type").unwrap().as_str();
                        let entry_name = captures.name("name").unwrap().as_str();
                        match entry_type {
                            ENTRY_DIRECTORY_DOCUMENT => entries.documents.push(entry_name.to_owned()),
                            ENTRY_DIRECTORY_GENERAL => entries.general.push(entry_name.to_owned()),
                            ENTRY_DIRECTORY_NARRATIVE => entries.narrative.push(entry_name.to_owned()),
                            _ => panic!("unexpected"),
                        };
                    }
                }
            }
    entries
}

impl Journal {
    pub fn from_file(path: &str, repo: &Repository) -> Journal {
        let entries = nix_eval_rules(path);

        let changes = LastChanged::new(repo);

        let general = JournalSection::new(EntryKind::General, repo, &changes, &entries.general);
        let narrative =
            JournalSection::new(EntryKind::Narrative, repo, &changes, &entries.narrative);

        Journal {
            _documents: entries.documents,
            general,
            narrative,
        }
    }

    pub fn section(&self, kind: &EntryKind) -> &JournalSection {
        match kind {
            EntryKind::General => &self.general,
            EntryKind::Narrative => &self.narrative,
        }
    }

    fn git_index(&self, repo: &Repository, filename: &str, action: &dyn Fn(&mut Index, &PathBuf) -> Result<(), git2::Error>) -> Result<(), JournalError> {
        let mut filepath = PathBuf::new();
        filepath.push(filename);
        match repo.index() {
            Ok(mut index) => {
                if let Some(err) = action(&mut index, &filepath).err() {
                    return Err(JournalError::chain(&[
                        ErrorKind::GitIndexError,
                        ErrorKind::Wrapped(err.to_string()),
                    ]));
                }
                if let Some(err) = index.write().err() {
                    return Err(JournalError::chain(&[
                        ErrorKind::GitIndexError,
                        ErrorKind::Wrapped(err.to_string()),
                    ]));
                }
                Ok(())
            }
            Err(err) => Err(JournalError::chain(&[
                ErrorKind::GitIndexError,
                ErrorKind::Wrapped(err.to_string()),
            ])),
        }
    }

    pub fn git_index_remove(&self, repo: &Repository, filename: &str) -> Result<(), JournalError> {
        let action = |index: &mut Index, filepath: &PathBuf| -> Result<(), git2::Error> {
            index.remove_path(filepath)
        };
        self.git_index(repo, filename, &action)
    }

    pub fn git_index_add(&self, repo: &Repository, filename: &str) -> Result<(), JournalError> {
        let action = |index: &mut Index, filepath: &PathBuf| -> Result<(), git2::Error> {
            index.add_path(filepath)
        };
        self.git_index(repo, filename, &action)
    }

    pub fn git_index_add_entry(&self, repo: &Repository, kind: &EntryKind, entry: &str) -> Result<(), JournalError> {
        let filename = entry_filename(kind, entry);
        self.git_index_add(repo, &filename)
    }

    pub fn git_index_remove_entry(&self, repo: &Repository, kind: &EntryKind, entry: &str) -> Result<(), JournalError> {
        let filename = entry_filename(kind, entry);
        self.git_index_remove(repo, &filename)
    }
}
