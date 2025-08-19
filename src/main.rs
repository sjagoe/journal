use std::fmt;
use std::fs::File;
use std::io::BufReader;

use color_eyre::Result;
use dirs::config_dir;
use serde::Deserialize;

const APP_NAME: &str = "personal-journal";

#[derive(Debug, Deserialize)]
struct Config {
    identity: String,
    journal_path: String,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "identity: {}", self.identity)?;
        write!(f, "journal_path: {}", self.journal_path)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    let mut config_path = config_dir().unwrap();
    config_path.push(APP_NAME);
    config_path.push("config.json");

    let file = File::open(config_path)?;
    let reader = BufReader::new(file);
    let config: Config = serde_json::from_reader(reader)?;

    println!("{}", config);

    Ok(())
}
