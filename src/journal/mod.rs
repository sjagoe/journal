use std::{iter::chain, path::Path};

use git::git_index_add;
use git2::{Repository, Status};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use walkdir::WalkDir;

mod age;
mod changes;
pub(crate) mod decrypted_journal;
pub(crate) mod edit;
pub(crate) mod export;
pub(crate) mod git;

use crate::errors::{ErrorKind, JournalError};

use self::changes::LastChanged;

const ENTRY_DIRECTORY_DOCUMENT: &str = "documents";
const ENTRY_DIRECTORY_GENERAL: &str = "general";
const ENTRY_DIRECTORY_NARRATIVE: &str = "narrative";

#[derive(Clone, Debug)]
pub enum EntryKind {
    Document,
    General,
    Narrative,
}

impl From<&EntryKind> for String {
    fn from(val: &EntryKind) -> String {
        match val {
            EntryKind::Document => ENTRY_DIRECTORY_DOCUMENT.to_owned(),
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
    pub kind: EntryKind,
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

        JournalSection { kind, entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn iter_entries(&self) -> impl Iterator<Item = &std::string::String> {
        self.entries.iter().map(|entry| &entry.name)
    }

    fn iter_files(&self) -> impl Iterator<Item = std::string::String> {
        self.entries
            .iter()
            .map(|entry| entry_filename(&self.kind, &entry.name))
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
    pub(self) path: String,
    _documents: Vec<String>,
    pub general: JournalSection,
    pub narrative: JournalSection,
}

pub fn entry_filename(kind: &EntryKind, name: &str) -> String {
    let directory = String::from(kind);
    format!("{}/{}.age", directory, name)
}

fn load_entries_from_path(path: &str) -> SerializedEntries {
    let mut entries: SerializedEntries = SerializedEntries::default();

    for entry in WalkDir::new(path)
        .min_depth(2)
        .max_depth(2)
        .into_iter()
        .flatten()
    {
        let entry_path = entry.path();
        if let Some(extension) = entry_path.extension()
            && extension == "age"
            && let Some(parent) = entry_path.parent()
            && let Some(parent_name) = parent.file_name()
            && let Some(entry_type) = parent_name.to_str()
            && let Some(file_name) = entry_path.with_extension("").file_name()
            && let Some(entry_name) = file_name.to_str()
        {
            match entry_type {
                ENTRY_DIRECTORY_DOCUMENT => entries.documents.push(entry_name.to_owned()),
                ENTRY_DIRECTORY_GENERAL => entries.general.push(entry_name.to_owned()),
                ENTRY_DIRECTORY_NARRATIVE => entries.narrative.push(entry_name.to_owned()),
                _ => {}
            };
        }
    }

    entries
}

impl Journal {
    pub fn iter_files(&self) -> impl Iterator<Item = std::string::String> {
        chain(self.general.iter_files(), self.narrative.iter_files())
    }

    pub fn from_file(path: &str, repo: &Repository) -> Journal {
        let entries = load_entries_from_path(path);

        let changes = LastChanged::new(repo);

        let general = JournalSection::new(EntryKind::General, repo, &changes, &entries.general);
        let narrative =
            JournalSection::new(EntryKind::Narrative, repo, &changes, &entries.narrative);

        Journal {
            path: path.to_string(),
            _documents: entries.documents,
            general,
            narrative,
        }
    }

    pub fn sections(&self) -> [&JournalSection; 2] {
        [&self.general, &self.narrative]
    }

    pub fn section(&self, kind: &EntryKind) -> &JournalSection {
        match kind {
            EntryKind::General => &self.general,
            EntryKind::Narrative => &self.narrative,
            EntryKind::Document => todo!("Document view not implemented yet"),
        }
    }

    pub fn git_index_add_entry(
        &self,
        repo: &Repository,
        kind: &EntryKind,
        entry: &str,
    ) -> Result<(), JournalError> {
        let filename = entry_filename(kind, entry);
        git_index_add(repo, &filename).map_err(|err| {
            JournalError::chain(&[
                ErrorKind::GitIndexError,
                ErrorKind::Wrapped(err.to_string()),
            ])
        })
    }

    pub fn git_index_unstage_entry(
        &self,
        repo: &Repository,
        kind: &EntryKind,
        entry: &str,
    ) -> Result<(), JournalError> {
        let filename = entry_filename(kind, entry);
        let action = || -> Result<(), git2::Error> {
            let tree = repo.revparse_single("HEAD")?;
            repo.reset_default(Some(&tree), &[filename])?;
            Ok(())
        };
        action().map_err(|err| {
            JournalError::chain(&[
                ErrorKind::GitIndexError,
                ErrorKind::Wrapped(err.to_string()),
            ])
        })
    }
}
