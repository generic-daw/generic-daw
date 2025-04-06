use crate::components::styled_scrollable_with_direction;
use dir::Dir;
use iced::{
    Element,
    widget::{
        column,
        scrollable::{Direction, Scrollbar},
    },
};
use std::{path::Path, sync::Arc};

mod dir;
mod file;

#[derive(Clone, Debug)]
pub enum FileTreeAction {
    None,
    Dir(Arc<Path>),
    File(Arc<Path>),
}

pub struct FileTree(Box<[Dir]>);

impl FileTree {
    pub fn view(&self) -> Element<'_, FileTreeAction> {
        styled_scrollable_with_direction(
            column(self.0.iter().map(|dir| dir.view().0)),
            Direction::Vertical(Scrollbar::default()),
        )
        .into()
    }

    pub fn update(&mut self, path: &Path) {
        for dir in &mut self.0 {
            dir.update(path);
        }
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
