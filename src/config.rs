use std::{fmt, path::PathBuf};

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub editor: Option<String>,
    pub identity: String,
    pub journal_path: String,
}

impl Config {
    pub fn entries_path(&self) -> PathBuf {
        let mut path = PathBuf::new();
        path.push(&self.journal_path);
        path.push("entries.json");
        path
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "editor: {:?}", self.editor)?;
        writeln!(f, "identity: {}", self.identity)?;
        write!(f, "journal_path: {}", self.journal_path)?;
        Ok(())
    }
}
