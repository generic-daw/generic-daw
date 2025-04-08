use crate::components::styled_scrollable_with_direction;
use dir::Dir;
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
    None,
    Action(Arc<Path>, Action),
    File(Arc<Path>),
}

#[derive(Clone, Debug)]
pub enum Action {
    DirToggleOpen,
    DirOpened(Box<[Dir]>, Box<[File]>),
}

pub struct FileTree(Box<[Dir]>);

impl FileTree {
    pub fn view(&self) -> Element<'_, Message> {
        styled_scrollable_with_direction(
            column(self.0.iter().map(|dir| dir.view().0)),
            Direction::Vertical(Scrollbar::default()),
        )
        .into()
    }

    pub fn update(&mut self, path: &Path, action: &Action) -> Task<Message> {
        Task::batch(self.0.iter_mut().map(|dir| dir.update(path, action)))
    }
}

impl<I> From<I> for FileTree
where
    I: IntoIterator<Item: AsRef<Path>>,
{
    fn from(value: I) -> Self {
        Self(value.into_iter().map(|t| Dir::new(t.as_ref())).collect())
    }
}
