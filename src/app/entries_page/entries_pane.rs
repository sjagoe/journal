use git2::Status;
use ratatui::{
    Frame,
    layout::Rect,
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem},
};

use time::format_description::StaticFormatDescription;
use time::macros::format_description;

use super::ActivePane;
use crate::{
    app::pagination::pagination,
    journal::Journal,
    traits::{Drawable, Navigable},
};

#[allow(unused)]
static TIME_FORMAT: StaticFormatDescription = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second]+[offset_hour]:[offset_minute]"
);

fn format_status(status: &Option<Status>) -> Line<'_> {
    let style = Style::default().fg(Color::Gray);

    let status_line = if let Some(status) = status {
        let index = if status.is_index_deleted() {
            "D"
        } else if status.is_index_modified() || status.is_index_typechange() {
            "M"
        } else if status.is_index_new() {
            "A"
        } else if status.is_index_renamed() {
            "R"
        } else {
            " "
        };
        let wt = if status.is_wt_deleted() {
            "D"
        } else if status.is_wt_modified() || status.is_wt_typechange() {
            "M"
        } else if status.is_wt_new() {
            "?"
        } else if status.is_wt_renamed() {
            "R"
        } else {
            " "
        };

        Line::from(vec![
            Span::styled(index, style.fg(Color::Green)),
            Span::styled(wt, style.fg(Color::Red)),
        ])
    } else {
        Line::from(vec![
            Span::styled("?", style.add_modifier(Modifier::DIM)),
            Span::styled("?", style.add_modifier(Modifier::DIM)),
        ])
    };

    status_line + Span::raw(" ")
}

pub(super) struct EntriesPane {
    count: usize,
    selected_index: usize,
    active: bool,
    pane_kind: ActivePane,
}

impl EntriesPane {
    pub(super) fn new(
        journal: &Journal,
        pane_kind: ActivePane,
        active_pane: &ActivePane,
    ) -> EntriesPane {
        let section = journal.section(&(&pane_kind).into());
        EntriesPane {
            count: section.len(),
            selected_index: 0,
            active: active_pane == &pane_kind,
            pane_kind,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.count
    }

    pub(super) fn activate(&mut self, active_pane: &ActivePane) {
        self.active = active_pane == &self.pane_kind;
    }

    pub(super) fn update(&mut self, journal: &Journal) {
        let section = journal.section(&(&self.pane_kind).into());
        self.count = section.len();
        if self.selected_index > self.count - 1 {
            self.end();
        }
    }
}

impl Drawable for EntriesPane {
    fn draw(&self, frame: &mut Frame, area: Rect, content_block: Block, journal: &Journal) -> Rect {
        let section = journal.section(&(&self.pane_kind).into());

        let viewport = content_block.inner(area);

        let mut list_items = Vec::<ListItem>::new();

        let visible = viewport.height;
        let (_page, _pages, page_start_index) =
            pagination(self.len(), visible.into(), self.selected_index);

        let end_slice = if page_start_index + usize::from(visible) >= self.len() {
            self.len()
        } else {
            page_start_index + usize::from(visible)
        };
        let display_items = &section.entries[page_start_index..end_slice];

        for (pos, item) in display_items.iter().enumerate() {
            let style = if pos + page_start_index == self.selected_index {
                let [fg, bg] = match self.active {
                    true => [Color::Black, Color::Cyan],
                    false => [Color::White, Color::DarkGray],
                };
                Style::default().fg(fg).bg(bg)
            } else {
                Style::default().fg(Color::Gray)
            };

            let last_modified = match item.last_modified {
                Some(last_modified) => match last_modified.format(TIME_FORMAT) {
                    Ok(commit_time) => commit_time,
                    _ => "unknown".to_string(),
                },
                _ => "unknown".to_string(),
            };

            let line = format_status(&item.status)
                + Span::styled(
                    format!("{:30}", last_modified),
                    style.add_modifier(Modifier::DIM),
                )
                + Span::styled(&item.name, style);

            list_items.push(ListItem::new(line));
        }

        let content = List::new(list_items).block(content_block);
        frame.render_widget(content, area);

        viewport
    }

    fn title(&self) -> Vec<Span<'_>> {
        let name = match self.pane_kind {
            ActivePane::General => "General Entries",
            ActivePane::Narrative => "Narrative Entries",
        };
        let mut parts = vec![Span::from(" ")];
        let fg = match self.active {
            true => Color::Green,
            false => Color::Gray,
        };

        parts.push(Span::styled(
            name,
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ));

        parts.push(Span::from(" "));
        parts
    }
}

impl Navigable for EntriesPane {
    fn home(&mut self) {
        self.selected_index = 0;
    }

    fn end(&mut self) {
        self.selected_index = if self.len() > 0 { self.len() - 1 } else { 0 };
    }

    fn pagedown(&mut self, page_size: u16) {
        let h: usize = page_size.into();
        let index = self.selected_index.saturating_add(h);
        self.selected_index = index.min(self.len().saturating_sub(1));
    }

    fn pageup(&mut self, page_size: u16) {
        let h: usize = page_size.into();
        self.selected_index = self.selected_index.saturating_sub(h);
    }

    fn next_selection(&mut self) {
        if self.selected_index < self.len() - 1 {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    fn previous_selection(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.len() - 1;
        }
    }

    fn select(&self, journal: &Journal) -> Option<String> {
        self.selected_item(journal)
    }

    fn selected_item(&self, journal: &Journal) -> Option<String> {
        let section = journal.section(&(&self.pane_kind).into());
        if section.is_empty() {
            None
        } else {
            Some(section[self.selected_index].name.to_string())
        }
    }

    fn switch_pane(&mut self) {}
}
