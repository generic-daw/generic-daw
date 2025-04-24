use crate::components::styled_scrollable_with_direction;
use dir::{Dir, DirId};
use file::File;
use iced::{
    Element, Task,
    widget::{
        column,
        scrollable::{Direction, Scrollbar},
    },
};
use std::{path::Path, sync::Arc};

mod dir;
mod file;

#[derive(Clone, Debug)]
pub enum Message {
    Action(DirId, Action),
    File(Arc<Path>),
}

#[derive(Clone, Debug)]
pub enum Action {
    DirToggleOpen,
    DirOpened(Box<[Dir]>, Box<[File]>),
}

pub struct FileTree(Vec<Dir>);

impl FileTree {
    pub fn new(dirs: impl IntoIterator<Item: AsRef<Path>>) -> Self {
        Self(dirs.into_iter().map(Dir::new).collect())
    }

    pub fn view(&self) -> Element<'_, Message> {
        styled_scrollable_with_direction(
            column(self.0.iter().map(|dir| dir.view().0)),
            Direction::Vertical(Scrollbar::default()),
        )
        .into()
    }

    pub fn update(&mut self, id: DirId, action: &Action) -> Task<Message> {
        self.0
            .iter_mut()
            .find_map(|dir| dir.update(id, action))
            .unwrap_or_else(Task::none)
    }
}
