use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub editor: Option<String>,
    pub identity: String,
    pub journal_path: String,
    pub export: Option<String>,
    // Map action name to list of key bindings
    // e.g NavigateNext can be mapped to both C-n and Down
    pub key_map: HashMap<String, Vec<String>>,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "editor: {:?}", self.editor)?;
        writeln!(f, "export: {:?}", self.export)?;
        writeln!(f, "identity: {}", self.identity)?;
        writeln!(f, "journal_path: {}", self.journal_path)?;
        writeln!(f, "Key Map {{")?;

        for (action, keys) in self.key_map.iter() {
            writeln!(f, "  {}: {:?},", action, keys)?;
        }

        write!(f, "}}")?;
        Ok(())
    }
}
