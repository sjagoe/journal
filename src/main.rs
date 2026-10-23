use std::fs::File;
use std::io::BufReader;

use color_eyre::Result;
use dirs::config_dir;

use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::Backend};

mod app;
mod config;
mod errors;
mod journal;
mod traits;
mod tui;
mod ui;

use crate::{
    app::{App, events::Event},
    config::Config,
    ui::ui,
};

const APP_NAME: &str = "personal-journal";

fn main() -> Result<()> {
    errors::install_hooks()?;

    let mut config_path = config_dir().unwrap();
    config_path.push(APP_NAME);
    config_path.push("config.json");

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

        let read_event = event::read()?;

        if let CrosstermEvent::Key(key) = read_event {
            if key.kind == KeyEventKind::Release {
                // Skip events that are not KeyEventKind::Press
                continue;
            }
            if key.code == KeyCode::Char('x') && key.modifiers == KeyModifiers::CONTROL {
                return Ok(true);
            }
            let navigation_action = Event::from(key);
            redraw = match app.handle_event(&navigation_action) {
                Ok(redraw) => redraw.0,
                Err(error) => {
                    app.error(error);
                    true
                }
            }
        }
    }
}
