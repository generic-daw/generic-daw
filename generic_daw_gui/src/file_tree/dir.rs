use crate::{
	file_tree::{Action, Message, file::File},
	icons::{chevron_down, chevron_right},
	widget::LINE_HEIGHT,
};
use generic_daw_utils::unique_id;
use iced::{
	Element, Fill, Task,
	futures::{StreamExt as _, TryStreamExt as _},
	padding,
	widget::{button, column, container, row, rule, text},
};
use std::{path::Path, sync::Arc};

unique_id!(dir_entry);

pub use dir_entry::Id as DirId;

#[derive(Clone, Debug)]
pub struct Dir {
	id: DirId,
	name: Arc<str>,
	path: Arc<Path>,
	children: LoadStatus,
	open: bool,
}

#[derive(Clone, Debug)]
enum LoadStatus {
	Unloaded,
	Loading,
	Loaded {
		dirs: Box<[Dir]>,
		files: Box<[File]>,
	},
}

impl Dir {
	pub fn new(path: impl AsRef<Path>) -> Self {
		let path = path.as_ref();
		let name = path.file_name().unwrap().to_str().unwrap();

		Self {
			id: DirId::unique(),
			name: name.into(),
			path: path.into(),
			children: LoadStatus::Unloaded,
			open: false,
		}
	}

	pub fn update(&mut self, id: DirId, action: &Action) -> Option<Task<Message>> {
		if id == self.id {
			Some(match action {
				Action::DirOpened(dirs, files) => {
					self.children = LoadStatus::Loaded {
						dirs: dirs.clone(),
						files: files.clone(),
					};

					Task::none()
				}
				Action::DirToggleOpen => {
					self.open ^= true;

					if matches!(self.children, LoadStatus::Unloaded) {
						let path = self.path.clone();
						let id = self.id;
						self.children = LoadStatus::Loading;

						Task::perform(Self::load(path), move |(dirs, files)| {
							Message::Action(id, Action::DirOpened(dirs, files))
						})
					} else {
						Task::none()
					}
				}
			})
		} else if self.open
			&& let LoadStatus::Loaded { dirs, .. } = &mut self.children
		{
			dirs.iter_mut().find_map(|dir| dir.update(id, action))
		} else {
			None
		}
	}

	pub fn view(&self) -> (Element<'_, Message>, f32) {
		let mut height = 0.0;
		(
			column![
				button(
					row![
						if self.open {
							chevron_down
						} else {
							chevron_right
						}(),
						text(&*self.name).wrapping(text::Wrapping::None)
					]
					.spacing(2),
				)
				.style(button::text)
				.padding(1)
				.width(Fill)
				.on_press(Message::Action(self.id, Action::DirToggleOpen)),
				if self.open
					&& let LoadStatus::Loaded { dirs, files } = &self.children
					&& !(dirs.is_empty() && files.is_empty())
				{
					let children = column(
						dirs.iter()
							.map(Self::view)
							.chain(files.iter().map(File::view))
							.map(|(e, h)| {
								height += h;
								e
							}),
					);

					Some(row![
						container(rule::vertical(2))
							.padding(padding::left(LINE_HEIGHT / 2.0).right(LINE_HEIGHT / 4.0))
							.height(height),
						children
					])
				} else {
					None
				}
			]
			.into(),
			height + LINE_HEIGHT + 2.0,
		)
	}

	async fn load(path: Arc<Path>) -> (Box<[Self]>, Box<[File]>) {
		let Ok(entry) = smol::fs::read_dir(path).await else {
			return ([].into(), [].into());
		};

		let files = smol::lock::Mutex::new(Vec::new());
		let dirs = smol::lock::Mutex::new(Vec::new());

		entry
			.into_stream()
			.for_each_concurrent(None, async |entry| {
				let Ok(entry) = entry else {
					return;
				};

				let Ok(ty) = entry.file_type().await else {
					return;
				};

				let mut name = entry.file_name();
				name.make_ascii_lowercase();

				if ty.is_file() {
					let file = File::new(entry.path()).await;
					files.lock().await.push((file, name));
				} else if ty.is_dir() {
					let dir = Self::new(entry.path());
					dirs.lock().await.push((dir, name));
				}
			})
			.await;

		let mut files = files.into_inner();
		let mut dirs = dirs.into_inner();

		files.sort_unstable_by(|(_, aname), (_, bname)| aname.cmp(bname));
		dirs.sort_unstable_by(|(_, aname), (_, bname)| aname.cmp(bname));

		let files = files.into_iter().map(|(file, _)| file).collect();
		let dirs = dirs.into_iter().map(|(dir, _)| dir).collect();

		(dirs, files)
	}

	pub fn path(&self) -> &Path {
		&self.path
	}
}
