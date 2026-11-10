use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum Identities {
    One(String),
    Multiple(Vec<String>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub editor: Option<String>,
    #[serde(alias="identity")]
    identities: Identities,
    pub journal_path: String,
    pub export: Option<String>,
    // Map action name to list of key bindings
    // e.g NavigateNext can be mapped to both C-n and Down
    pub key_map: HashMap<String, Vec<String>>,
}

impl Config {
    pub fn get_identities(&self) -> Vec<String> {
        match self.identities {
            Identities::One(ref old) => {
                vec![old.to_owned()]
            }
            Identities::Multiple(ref identities) => {
                identities.clone()
            }
        }
    }

    pub fn with_new_identity(&self, identity: String) -> Self {
        let mut identities = self.get_identities();
        identities.push(identity);
        Self {
            editor: self.editor.clone(),
            identities: Identities::Multiple(identities),
            journal_path: self.journal_path.to_owned(),
            export: self.export.clone(),
            key_map: self.key_map.clone(),
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "editor: {:?}", self.editor)?;
        writeln!(f, "export: {:?}", self.export)?;
        writeln!(f, "identities: {:?}", self.identities)?;
        writeln!(f, "journal_path: {}", self.journal_path)?;
        writeln!(f, "Key Map {{")?;

        for (action, keys) in self.key_map.iter() {
            writeln!(f, "  {}: {:?},", action, keys)?;
        }

        write!(f, "}}")?;
        Ok(())
    }
}
