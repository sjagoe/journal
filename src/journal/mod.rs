use std::path::Path;
use std::{
    fs::{File, OpenOptions},
    io::BufReader,
    path::PathBuf,
};

use git2::{Repository, Status};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

mod age;
mod changes;
pub(crate) mod edit;
pub(crate) mod export;

use crate::errors::{ErrorKind, JournalError};

use self::changes::LastChanged;

#[derive(Clone, Debug)]
pub enum EntryKind {
    General,
    Narrative,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn contains(&self, entry_name: &str) -> bool {
        for entry in self.entries.iter() {
            if entry.name == entry_name {
                return true;
            }
        }
        false
    }

    // pub fn is_empty(&self) -> bool {
    //     self.entries.is_empty()
    // }
}

impl std::ops::Index<usize> for JournalSection {
    type Output = JournalEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

#[derive(Clone, Debug)]
pub struct Journal {
    location: PathBuf,
    _documents: Vec<String>,
    pub general: JournalSection,
    pub narrative: JournalSection,
}

pub fn entry_filename(kind: &EntryKind, name: &str) -> String {
    let directory = match kind {
        EntryKind::General => "general",
        EntryKind::Narrative => "narrative",
    };
    format!("{}/{}.age", directory, name)
}

impl Journal {
    pub fn from_file(path: &PathBuf, repo: &Repository) -> Journal {
        let file = File::open(path).expect("Unable to open entries data");
        let reader = BufReader::new(file);
        let entries: SerializedEntries =
            serde_json::from_reader(reader).expect("Unable to deserialize entries");

        let changes = LastChanged::new(repo);

        let general = JournalSection::new(EntryKind::General, repo, &changes, &entries.general);
        let narrative =
            JournalSection::new(EntryKind::Narrative, repo, &changes, &entries.narrative);

        Journal {
            location: path.to_owned(),
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

    pub fn with_new_entry(
        &self,
        kind: &EntryKind,
        name: &str,
        repo: &Repository,
    ) -> Result<Journal, JournalError> {
        if match kind {
            EntryKind::General => self.general.contains(name),
            EntryKind::Narrative => self.narrative.contains(name),
        } {
            return Err(JournalError::Error(ErrorKind::EntryExists));
        }

        let mut general: Vec<String> = self
            .general
            .entries
            .iter()
            .map(|i| i.name.to_owned())
            .collect();
        let mut narrative: Vec<String> = self
            .narrative
            .entries
            .iter()
            .map(|i| i.name.to_owned())
            .collect();

        let entries_kind = match kind {
            EntryKind::General => &mut general,
            EntryKind::Narrative => &mut narrative,
        };
        entries_kind.push(name.to_owned());
        general.sort();
        narrative.sort();

        let entries = SerializedEntries {
            documents: self._documents.to_owned(),
            general,
            narrative,
        };

        let tmp_location = self.location.with_added_extension("tmp");
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_location)
            .expect("Unable to open entries data");

        // fixme unwrap!
        serde_json::to_writer_pretty(file, &entries).unwrap();

        std::fs::rename(&tmp_location, &self.location).unwrap();

        Ok(Journal::from_file(&self.location, repo))
    }

    pub fn git_add(&self, repo: &Repository, filename: &str) -> Result<(), JournalError> {
        let mut filepath = PathBuf::new();
        filepath.push(filename);
        match repo.index() {
            Ok(mut index) => {
                if let Some(err) = index.add_path(&filepath).err() {
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
}
