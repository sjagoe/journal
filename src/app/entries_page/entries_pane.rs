use std::{collections::HashMap, path::Path};

use color_eyre::Result;

use git2::{ObjectType, Repository, Sort, Status};
use ratatui::{
    Frame,
    layout::Rect,
    prelude::Modifier,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem},
};

use time::UtcOffset;
use time::macros::format_description;
use time::{Timestamp, format_description::StaticFormatDescription};

use crate::{app::pagination::pagination, errors::JournalError};
use crate::{
    journal::JournalSection,
    traits::{Drawable, Navigable},
};

static TIME_FORMAT: StaticFormatDescription = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second]+[offset_hour]:[offset_minute]"
);

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
    fn new(name: &String, last_modified: &String, status: Option<Status>) -> EntryContent {
        EntryContent {
            name: name.to_string(),
            last_modified: last_modified.to_string(),
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

fn make_entries(
    repo: &Repository,
    entry_names: &Vec<String>,
) -> Result<Vec<EntryContent>, JournalError> {
    let mut entries: Vec<EntryContent> = vec![];

    let filename = |i| format!("{}.age", i);

    let filenames: HashMap<String, &String> = entry_names
        .into_iter()
        .map(|x| (filename(x), x))
        .collect::<HashMap<_, _>>();

    // Map filename to entry name and last changed date
    let mut filenames_map: HashMap<String, String> = HashMap::new();

    let mut walk = match repo.revwalk() {
        Ok(walk) => walk,
        Err(e) => panic!("Error walking repository: {}", e),
    };

    match walk.set_sorting(Sort::TOPOLOGICAL) {
        Ok(_) => match walk.push_head() {
            Ok(_) => {}
            Err(e) => panic!("Error walking repository: {}", e),
        },
        Err(e) => panic!("Error walking repository: {}", e),
    }

    for item in walk {
        let Some(oid) = item.ok() else {
            continue;
        };

        let Ok(object) = repo.find_object(oid, Some(ObjectType::Commit)) else {
            continue;
        };

        let Some(commit) = object.as_commit() else {
            continue;
        };

        let Some(tree) = commit.tree().ok() else {
            continue;
        };

        for parent in commit.parents() {
            let Some(parent_tree) = parent.tree().ok() else {
                continue;
            };

            let Some(diff) = repo
                .diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)
                .ok()
            else {
                continue;
            };

            for delta in diff.deltas() {
                let file = delta.new_file();
                let Some(path) = file.path() else {
                    continue;
                };
                let Some(name) = path.to_str() else {
                    continue;
                };

                if filenames_map.contains_key(name) {
                    continue;
                }
                let Some(entry_name) = filenames.get(name) else {
                    continue;
                };

                let time_raw = commit.time();

                let Some(offset) =
                    UtcOffset::from_whole_seconds(time_raw.offset_minutes() * 60).ok()
                else {
                    continue;
                };
                let Some(timestamp) = Timestamp::from_seconds(time_raw.seconds()).ok() else {
                    continue;
                };

                let time = timestamp.to_offset(offset);

                if let Ok(commit_time) = time.format(TIME_FORMAT) {
                    filenames_map.insert(name.into(), commit_time);
                }
            }

            if filenames_map.len() == filenames.len() {
                break;
            }
        }
    }

    for entry_name in entry_names.iter() {
        let file = filename(entry_name);
        match filenames_map.get(&file) {
            Some(time) => {
                let path = Path::new(&file);
                let status = match repo.status_file(&path) {
                    Ok(status) => Some(status),
                    Err(_) => None,
                };
                entries.push(EntryContent::new(entry_name, time, status));
            }
            None => panic!("Failed to parse all entries time stamps"),
        }
    }

    Ok(entries)
}

pub struct EntriesPane<'a> {
    section: &'a JournalSection,
    selected_index: usize,
    active: bool,
    pane_kind: ActivePane,
}

impl<'a> EntriesPane<'a> {
    pub fn new(
        section: &'a JournalSection,
        pane_kind: ActivePane,
        active_pane: &ActivePane,
    ) -> EntriesPane<'a> {
        EntriesPane {
            section,
            selected_index: 0,
            active: active_pane == &pane_kind,
            pane_kind,
        }
    }

    fn len(&self) -> usize {
        self.section.len()
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
            pagination(self.len(), visible.into(), self.selected_index);

        let end_slice = if page_start_index + usize::from(visible) >= self.len() {
            self.len()
        } else {
            page_start_index + usize::from(visible)
        };
        let display_items = &self.section.entries[page_start_index..end_slice];

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
                    style.clone().add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    format!("{:30}", &item.last_modified),
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
        if self.section.is_empty() {
            "".to_string()
        } else {
            self.section[self.selected_index].name.to_string()
        }
    }

    fn switch_pane(&mut self) {}
}
