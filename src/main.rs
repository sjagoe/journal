use std::io::BufReader;
use std::path::PathBuf;
use std::{fs::File, time::Duration};

use color_eyre::Result;
use color_eyre::eyre::eyre;
use dirs::config_dir;

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind};
use errors::{ErrorKind, JournalError};
use ratatui::{Terminal, backend::Backend};

mod app;
mod config;
mod errors;
mod journal;
mod traits;
mod tui;
mod ui;

use crate::{app::App, config::Config, ui::ui};

const APP_NAME: &str = "personal-journal";

#[derive(Clone, Subcommand, Debug)]
enum RekeyCommand {
    ReKey {
        /// The new encryption key, to update configuration
        new_key: String,
    },
}

#[derive(Parser, Clone, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Configuration file to use
    #[arg(short, long)]
    config: Option<String>,
    /// Re-key the repository
    #[command(subcommand)]
    re_key: Option<RekeyCommand>,
}

fn main() -> Result<()> {
    errors::install_hooks()?;

    let args = Args::parse();

    let config_path = if let Some(config) = args.config {
        let mut config_path = PathBuf::new();
        config_path.push(config);
        config_path
    } else {
        let mut config_path = config_dir().unwrap();
        config_path.push(APP_NAME);
        config_path.push("config.json");
        config_path
    };

    let file = File::open(&config_path)?;
    let reader = BufReader::new(file);
    let config: Config = serde_json::from_reader(reader)?;

    println!("{}", config);

    let mut app: app::App = app::App::new(&config)?;

    if let Some(rk) = args.re_key {
        match rk {
            RekeyCommand::ReKey { new_key } => {
                let mut path = PathBuf::new();
                path.push(new_key);
                let new_key_path = std::path::absolute(&path)?;
                if !new_key_path.is_file() {
                    return Err(eyre!("{} is not a file", &new_key_path.display()));
                }
                app.re_key(&new_key_path)?;

                let config = match new_key_path.as_os_str().to_str() {
                    Some(new_key) => config.with_new_identity(new_key.to_string()),
                    None => panic!("Failed to store new key in config"),
                };

                let mut new_config_path = PathBuf::new();
                new_config_path.push(&config_path);
                new_config_path.set_extension("rekey.tmp");
                let j = serde_json::to_vec_pretty(&config)?;
                std::fs::write(&new_config_path, &j)?;
                std::fs::rename(&new_config_path, &config_path)?;

                return Ok(());
            }
        }
    }

    let mut terminal = tui::init()?;
    run_app(&mut terminal, &mut app)?;
    tui::restore()?;

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<bool>
where
    <B as Backend>::Error: Send,
    <B as Backend>::Error: Sync,
    <B as Backend>::Error: 'static,
{
    let mut redraw = false;
    loop {
        if redraw {
            terminal.clear()?;
        }
        terminal.draw(|f| ui(f, app))?;

        let tick_duration: u8 = 50;
        if !event::poll(Duration::from_millis(tick_duration.into()))? {
            redraw = match app.tick(tick_duration) {
                Ok(redraw) => redraw.0,
                Err(error) => {
                    app.error(error);
                    true
                }
            };
            continue;
        }

        let read_event = event::read()?;

        if let CrosstermEvent::Key(key) = read_event {
            if key.kind == KeyEventKind::Release {
                // Skip events that are not KeyEventKind::Press
                continue;
            }
            redraw = match app.handle_event(&key) {
                Ok(redraw) => redraw.0,
                Err(ref error) => {
                    match error {
                        JournalError::Error(e) => match e {
                            ErrorKind::AppExit => {
                                return Ok(true);
                            }
                            _ => app.error(error.clone()),
                        },
                    }
                    true
                }
            }
        }
    }
}
