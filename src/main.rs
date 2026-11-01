use std::io::BufReader;
use std::path::PathBuf;
use std::{fs::File, time::Duration};

use color_eyre::Result;
use dirs::config_dir;

use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind};
use errors::{ErrorKind, JournalError};
use ratatui::{Terminal, backend::Backend};
use clap::Parser;

mod app;
mod config;
mod errors;
mod journal;
mod traits;
mod tui;
mod ui;

use crate::{app::App, config::Config, ui::ui};

const APP_NAME: &str = "personal-journal";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Configuration file to use
    #[arg(short, long)]
    config: Option<String>,
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

    let file = File::open(config_path)?;
    let reader = BufReader::new(file);
    let config: Config = serde_json::from_reader(reader)?;

    println!("{}", config);

    let mut app: app::App = app::App::new(&config)?;

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
