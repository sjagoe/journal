use std::collections::HashMap;

use git2::{ObjectType, Repository, Sort};

use time::UtcOffset;
use time::macros::format_description;
use time::{Timestamp, format_description::StaticFormatDescription};

static TIME_FORMAT: StaticFormatDescription = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second]+[offset_hour]:[offset_minute]"
);

pub struct LastChanged {
    // Map of filename to last changed time
    last_changed: HashMap<String, String>,
}

impl LastChanged {
    pub fn new(repo: &Repository) -> LastChanged {
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

    pub fn get(&self, filename: &String) -> Option<String> {
        self.last_changed.get(filename).map(|time| time.to_string())
    }
}
