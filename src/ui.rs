use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::app::App;

pub fn ui(f: &mut Frame, app: &mut App) {
    // Create the layout sections.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(1)]);
    let [window, statusbar] = chunks.areas(f.area());

    app.draw(f, window);
    app.draw_context_hint(f, statusbar);
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
pub fn centered_rect(x: Constraint, y: Constraint, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), y, Constraint::Fill(1)])
        .split(r);

    // Then cut the middle vertical piece into three width-wise pieces
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), x, Constraint::Fill(1)])
        .split(popup_layout[1])[1] // Return the middle chunk
}
