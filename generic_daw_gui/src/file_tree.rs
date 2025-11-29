use crate::stylefns::{bordered_box_with_radius, scrollable_style};
use dir::{Dir, DirId};
use file::File;
use iced::{
	Element, Fill, Task, padding,
	widget::{column, container, scrollable},
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

#[derive(Debug)]
pub struct FileTree {
	dirs: Vec<Dir>,
}

impl FileTree {
	pub fn new(dirs: impl IntoIterator<Item: AsRef<Path>>) -> Self {
		Self {
			dirs: dirs.into_iter().map(Dir::new).collect(),
		}
	}

	pub fn view(&self) -> Element<'_, Message> {
		container(
			scrollable(container(column(self.dirs.iter().map(|dir| dir.view().0))).clip(true))
				.spacing(5)
				.style(scrollable_style),
		)
		.style(|t| {
			bordered_box_with_radius(0)(t).background(t.extended_palette().background.weakest.color)
		})
		.padding(padding::all(1).left(0))
		.height(Fill)
		.into()
	}

	pub fn update(&mut self, id: DirId, action: &Action) -> Option<Task<Message>> {
		self.dirs.iter_mut().find_map(|dir| dir.update(id, action))
	}

	pub fn diff(&mut self, dirs: impl IntoIterator<Item: AsRef<Path>>) {
		for (i, dir) in dirs.into_iter().enumerate() {
			let j = self.dirs[i..]
				.iter()
				.position(|entry| entry.path() == dir.as_ref())
				.map_or(self.dirs.len(), |j| j + i);
			self.dirs.drain(i..j);

			if i >= self.dirs.len() {
				self.dirs.push(Dir::new(dir));
			}
		}
	}
}
