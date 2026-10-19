use std::fmt;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub identity: String,
    pub journal_path: String,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "identity: {}", self.identity)?;
        write!(f, "journal_path: {}", self.journal_path)?;
        Ok(())
    }
}
