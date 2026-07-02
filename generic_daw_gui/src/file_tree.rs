use crate::stylefns::{scrollable_style, weakest_bordered_box};
use dir::{Dir, DirId};
use file::File;
use iced::{
	Element, Fill, Task, padding,
	widget::{column, container, scrollable},
};
use std::{path::Path, sync::Arc};

mod dir;
mod file;

pub use file::FileKind;

#[derive(Clone, Debug)]
pub enum Message {
	Action(DirId, Action),
	DragFile(Arc<Path>, FileKind),
	OpenFile(Arc<Path>, FileKind),
}

#[derive(Clone, Debug)]
pub enum Action {
	ToggleOpen,
	Reload,
	Loaded(Result<(Vec<Dir>, Vec<File>), Arc<std::io::Error>>),
}

#[derive(Debug)]
pub struct FileTree {
	dirs: Vec<Dir>,
}

impl FileTree {
	pub fn new(dirs: &[impl AsRef<Path>]) -> Self {
		Self {
			dirs: dirs.iter().map(Dir::new).collect(),
		}
	}

	pub fn view(&self) -> Element<'_, Message> {
		container(
			scrollable(column(self.dirs.iter().map(|dir| dir.view().0)))
				.width(Fill)
				.height(Fill)
				.spacing(5)
				.style(scrollable_style),
		)
		.style(weakest_bordered_box)
		.padding(padding::all(1).left(0))
		.into()
	}

	pub fn update(&mut self, id: DirId, action: &Action) -> Option<Task<Message>> {
		self.dirs.iter_mut().find_map(|dir| dir.update(id, action))
	}

	pub fn diff(&mut self, dirs: &[impl AsRef<Path>]) {
		for (i, dir) in dirs.iter().enumerate() {
			let j = self.dirs[i..]
				.iter()
				.position(|entry| entry.path() == dir.as_ref())
				.unwrap_or_default();
			self.dirs.drain(i..i + j);

			if self
				.dirs
				.get(i)
				.is_none_or(|entry| entry.path() != dir.as_ref())
			{
				self.dirs.insert(i, Dir::new(dir));
			}
		}

		self.dirs.truncate(dirs.len());
	}
}
