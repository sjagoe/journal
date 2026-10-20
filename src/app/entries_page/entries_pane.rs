use std::path::Path;

use color_eyre::Result;

use git2::{Object, Repository, Status};
use ratatui::{
    Frame,
    layout::Rect,
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem},
};

use crate::app::pagination::pagination;
use crate::traits::{Drawable, Navigable};

#[derive(PartialEq)]
pub enum ActivePane {
    General,
    Narrative,
}

struct EntryContent {
    name: String,
    last_modified: String,
    status: Option<Status>,
}

impl EntryContent {
    pub fn from_repo(name: &String, repo: &Repository) -> EntryContent {
        let filename: String = format!("{}.age", &name);
        let path = Path::new(&filename);
        let status = match repo.status_file(&path) {
            Ok(status) => Some(status),
            Err(_) => None,
        };

        // let commit: Option<Object> = match args.commit_id {
        //     Some(commit_id) => match repo.revparse_single("HEAD") {
        //         Ok(object) => Some(object),
        //         Err(e) => panic!("Failed to get commit {}", e),
        //     },
        //     _ => None,
        // };

        EntryContent {
            name: name.to_string(),
            last_modified: "2020-09-13 21:13".to_string(),
            status,
        }
    }

    pub fn status(&self) -> String {
        let mut index = "?";
        let mut wt = "?";

        if let Some(status) = self.status {
            index = " ";
            wt = " ";
            if status.is_index_deleted() {
                index = "D";
            } else if status.is_index_modified() || status.is_index_typechange() {
                index = "M";
            } else if status.is_index_new() {
                index = "A";
            } else if status.is_index_renamed() {
                index = "R";
            }
            if status.is_wt_deleted() {
                wt = "D";
            } else if status.is_wt_modified() || status.is_wt_typechange() {
                wt = "M";
            } else if status.is_wt_new() {
                wt = "A";
            } else if status.is_wt_renamed() {
                wt = "R";
            }
        }

        format!("{}{}", index, wt)
    }
}

pub struct EntriesPane<'a> {
    repo: &'a Repository,
    entries: Vec<EntryContent>,
    selected_index: usize,
    active: bool,
    pane_kind: ActivePane,
}

impl<'a> EntriesPane<'a> {
    pub fn new(
        repo: &'a Repository,
        entry_names: Vec<String>,
        pane_kind: ActivePane,
        active_pane: &ActivePane,
    ) -> EntriesPane<'a> {
        let mut entries: Vec<EntryContent> = vec![];

        for item in entry_names.iter() {
            entries.push(EntryContent::from_repo(item, repo));
        }

        EntriesPane {
            repo,
            entries,
            selected_index: 0,
            active: active_pane == &pane_kind,
            pane_kind,
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn activate(&mut self, active_pane: &ActivePane) {
        self.active = active_pane == &self.pane_kind;
    }
}

impl Drawable for EntriesPane<'_> {
    fn draw(&self, frame: &mut Frame, area: Rect, content_block: Block) -> Rect {
        let viewport = content_block.inner(area);

        let mut list_items = Vec::<ListItem>::new();

        let visible = viewport.height;
        let (_page, _pages, page_start_index) =
            pagination(self.entries.len(), visible.into(), self.selected_index);

        let end_slice = if page_start_index + usize::from(visible) >= self.entries.len() {
            self.entries.len()
        } else {
            page_start_index + usize::from(visible)
        };
        let display_items = &self.entries[page_start_index..end_slice];

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
                    format!("{} ", item.status()),
                    style.clone().add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    format!("{:18}", &item.last_modified),
                    style.clone().add_modifier(Modifier::DIM),
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

impl Navigable for EntriesPane<'_> {
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

    fn select(&self) -> Option<String> {
        Some("test".into())
    }

    fn selected_item(&self) -> String {
        let items = &self.entries;
        if items.is_empty() {
            "".to_string()
        } else {
            items[self.selected_index].name.to_string()
        }
    }

    fn switch_pane(&mut self) {}

    fn next_tick(&mut self, _block: bool) -> Result<()> {
        Ok(())
    }
}
