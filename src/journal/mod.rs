use std::path::Path;
use std::{fs::File, io::BufReader, path::PathBuf};

use git2::Repository;
use serde::Deserialize;

mod changes;

use self::changes::LastChanged;

#[derive(Clone, Debug)]
pub enum EntryKind {
    General,
    Narrative,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SerializedEntries {
    documents: Vec<String>,
    general: Vec<String>,
    narrative: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct JournalEntry {
    pub name: String,
    pub status: String,
    pub last_modified: String,
}

impl JournalEntry {
    fn new(name: &str, status: &str, last_modified: &str) -> JournalEntry {
        JournalEntry {
            name: name.to_owned(),
            status: status.to_owned(),
            last_modified: last_modified.to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct JournalSection {
    pub kind: EntryKind,
    pub entries: Vec<JournalEntry>,
}

impl JournalSection {
    fn new(kind: EntryKind, entries: Vec<JournalEntry>) -> JournalSection {
        JournalSection { kind, entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
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
    _documents: Vec<String>,
    pub general: JournalSection,
    pub narrative: JournalSection,
}

fn status(repo: &Repository, filename: &String) -> String {
    let path = Path::new(&filename);
    let mut index = "?";
    let mut wt = "?";

    if let Ok(status) = repo.status_file(path) {
        index = " ";
        wt = " ";
        if status.is_index_deleted() {
            index = "D";
        } else if status.is_index_modified() || status.is_index_typechange() {
            index = "M";
        } else if status.is_index_new() {
            index = "A";
        } else if status.is_index_renamed() {
            index = "R";
        }
        if status.is_wt_deleted() {
            wt = "D";
        } else if status.is_wt_modified() || status.is_wt_typechange() {
            wt = "M";
        } else if status.is_wt_new() {
            wt = "A";
        } else if status.is_wt_renamed() {
            wt = "R";
        }
    };

    format!("{}{}", index, wt)
}

fn to_entries(repo: &Repository, changes: &LastChanged, items: &[String]) -> Vec<JournalEntry> {
    items
        .iter()
        .map(|name| {
            let filename = format!("{}.age", name);
            let status_str = status(repo, &filename);
            match changes.get(&filename) {
                Ok(time) => JournalEntry::new(name, &status_str, &time),
                Err(_) => JournalEntry::new(name, &status_str, "unknown"),
            }
        })
        .collect()
}

impl Journal {
    pub fn from_file(path: &PathBuf, repo: &Repository) -> Journal {
        let file = File::open(path).expect("Unable to open entries data");
        let reader = BufReader::new(file);
        let entries: SerializedEntries =
            serde_json::from_reader(reader).expect("Unable to deserialize entries");

        let changes = LastChanged::new(repo);

        let general = JournalSection::new(
            EntryKind::General,
            to_entries(repo, &changes, &entries.general),
        );
        let narrative = JournalSection::new(
            EntryKind::Narrative,
            to_entries(repo, &changes, &entries.narrative),
        );

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
}
