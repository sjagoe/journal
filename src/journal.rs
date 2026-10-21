use std::path::Path;
use std::{collections::HashMap, fs::File, io::BufReader, path::PathBuf};

use git2::{ObjectType, Oid, Repository, Revwalk, Sort};
use serde::Deserialize;

use time::UtcOffset;
use time::macros::format_description;
use time::{Timestamp, format_description::StaticFormatDescription};

use crate::errors::{ErrorKind, JournalError};

static TIME_FORMAT: StaticFormatDescription = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second]+[offset_hour]:[offset_minute]"
);

#[derive(Clone, Debug, Deserialize)]
pub struct SerializedEntries {
    documents: Vec<String>,
    general: Vec<String>,
    narrative: Vec<String>,
}

struct LastChanged {
    // Map of filename to last changed time
    last_changed: HashMap<String, String>,
}

impl LastChanged {
    fn new(repo: &Repository) -> LastChanged {
        let mut last_changed: HashMap<String, String> = HashMap::new();

        let mut walk = match repo.revwalk() {
            Ok(walk) => walk,
            Err(e) => panic!("Error walking repository: {}", e),
        };

        match walk.set_sorting(Sort::TOPOLOGICAL) {
            Ok(_) => {
                if let Some(e) = walk.push_head().err() {
                    panic!("Error walking repository: {}", e);
                }
            }
            Err(e) => panic!("Error walking repository: {}", e),
        }

        for item in walk {
            let Some(oid) = item.ok() else {
                continue;
            };

            let Ok(object) = repo.find_object(oid, Some(ObjectType::Commit)) else {
                continue;
            };

            let Some(commit) = object.as_commit() else {
                continue;
            };

            let Some(tree) = commit.tree().ok() else {
                continue;
            };

            for parent in commit.parents() {
                let Some(parent_tree) = parent.tree().ok() else {
                    continue;
                };

                let Some(diff) = repo
                    .diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)
                    .ok()
                else {
                    continue;
                };

                for delta in diff.deltas() {
                    let file = delta.new_file();
                    let Some(path) = file.path() else {
                        continue;
                    };
                    let Some(name) = path.to_str() else {
                        continue;
                    };

                    if last_changed.contains_key(name) {
                        continue;
                    }

                    let time_raw = commit.time();

                    let Some(offset) =
                        UtcOffset::from_whole_seconds(time_raw.offset_minutes() * 60).ok()
                    else {
                        continue;
                    };
                    let Some(timestamp) = Timestamp::from_seconds(time_raw.seconds()).ok() else {
                        continue;
                    };

                    let time = timestamp.to_offset(offset);

                    if let Ok(commit_time) = time.format(TIME_FORMAT) {
                        last_changed.insert(name.into(), commit_time);
                    }
                }
            }
        }

        LastChanged { last_changed }
    }

    pub fn get(&self, filename: &String) -> Result<String, JournalError> {
        if let Some(time) = self.last_changed.get(filename) {
            return Ok(time.to_string());
        }

        Err(JournalError::Error(ErrorKind::RepositoryWalkError))
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub struct JournalSection {
    pub entries: Vec<JournalEntry>,
}

impl JournalSection {
    fn new(entries: Vec<JournalEntry>) -> JournalSection {
        JournalSection { entries }
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

#[derive(Debug)]
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

        let general = JournalSection::new(to_entries(repo, &changes, &entries.general));
        let narrative = JournalSection::new(to_entries(repo, &changes, &entries.narrative));

        Journal {
            _documents: entries.documents,
            general: general,
            narrative: narrative,
        }
    }
}
