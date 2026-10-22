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

use crate::traits::{Drawable, Navigable};
use crate::{
    app::pagination::pagination,
    journal::{EntryKind, Journal},
};

#[allow(unused)]
static TIME_FORMAT: StaticFormatDescription = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second]+[offset_hour]:[offset_minute]"
);

#[derive(Clone, Debug, PartialEq)]
pub enum ActivePane {
    General,
    Narrative,
}

impl Into<EntryKind> for &ActivePane {
    fn into(self) -> EntryKind {
        match self {
            ActivePane::General => EntryKind::General,
            ActivePane::Narrative => EntryKind::Narrative,
        }
    }
}

pub struct EntriesPane {
    count: usize,
    selected_index: usize,
    active: bool,
    pane_kind: ActivePane,
}

impl EntriesPane {
    pub fn new(journal: &Journal, pane_kind: ActivePane, active_pane: &ActivePane) -> EntriesPane {
        let section = journal.section(&(&pane_kind).into());
        EntriesPane {
            count: section.len(),
            selected_index: 0,
            active: active_pane == &pane_kind,
            pane_kind,
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn activate(&mut self, active_pane: &ActivePane) {
        self.active = active_pane == &self.pane_kind;
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

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", item.status),
                    style.add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    format!("{:30}", item.last_modified),
                    style.add_modifier(Modifier::DIM),
                ),
                Span::styled(&item.name, style),
            ]);
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
    fn home(&mut self, _page_size: u16) {
        self.selected_index = 0;
    }

    fn end(&mut self, _page_size: u16) {
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
        let section = journal.section(&(&self.pane_kind).into());
        Some(section[self.selected_index].name.to_string())
    }

    // fn selected_item(&self) -> String {
    //     if self.section.is_empty() {
    //         "".to_string()
    //     } else {
    //         self.section[self.selected_index].name.to_string()
    //     }
    // }

    fn switch_pane(&mut self) {}
}
