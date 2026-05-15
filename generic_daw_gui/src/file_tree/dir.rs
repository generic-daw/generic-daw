use crate::{
	components::labeled_icon_button,
	file_tree::{Action, Message, file::File},
	icons::{chevron_down, chevron_right, hourglass, triangle_alert},
	stylefns::{button_warning_text, container_with_radius, weak_bordered_box},
	widget::LINE_HEIGHT,
};
use iced::{
	Element, Fill, Task,
	futures::{StreamExt as _, TryStreamExt as _},
	padding,
	widget::{button, column, container, row, rule, tooltip, value},
};
use std::{path::Path, sync::Arc};
use utils::{natural_cmp, unique_id};

unique_id!(dir_id);

pub use dir_id::Id as DirId;

#[derive(Clone, Debug)]
pub struct Dir {
	id: DirId,
	name: Arc<str>,
	path: Arc<Path>,
	children: Status,
}

#[derive(Clone, Debug)]
enum Status {
	Unloaded,
	Loading,
	Loaded {
		dirs: Box<[Dir]>,
		files: Box<[File]>,
		open: bool,
	},
	Errored(Arc<std::io::Error>),
}

impl Dir {
	pub fn new(path: impl AsRef<Path>) -> Self {
		let path = path.as_ref();
		let name = path.file_name().unwrap().to_str().unwrap();

		Self {
			id: DirId::unique(),
			name: name.into(),
			path: path.into(),
			children: Status::Unloaded,
		}
	}

	pub fn update(&mut self, id: DirId, action: &Action) -> Option<Task<Message>> {
		if id == self.id {
			Some(match action {
				Action::DirLoaded(res) => {
					self.children = match res.clone() {
						Ok((dirs, files)) => Status::Loaded {
							dirs,
							files,
							open: true,
						},
						Err(err) => Status::Errored(err),
					};

					Task::none()
				}
				Action::DirToggleOpen => {
					if let Status::Loaded { open, .. } = &mut self.children {
						*open ^= true;
						Task::none()
					} else {
						let path = self.path.clone();
						let id = self.id;
						self.children = Status::Loading;

						Task::perform(Self::load(path), move |res| {
							Message::Action(id, Action::DirLoaded(res))
						})
					}
				}
			})
		} else if let Status::Loaded { dirs, open, .. } = &mut self.children
			&& *open
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
				match &self.children {
					Status::Unloaded =>
						labeled_icon_button(chevron_right(), &*self.name, button::text)
							.width(Fill)
							.on_press(Message::Action(self.id, Action::DirToggleOpen))
							.into(),
					Status::Loading => labeled_icon_button(hourglass(), &*self.name, button::text)
						.width(Fill)
						.into(),
					Status::Loaded { open, .. } => labeled_icon_button(
						if *open {
							chevron_down()
						} else {
							chevron_right()
						},
						&*self.name,
						button::text
					)
					.width(Fill)
					.on_press(Message::Action(self.id, Action::DirToggleOpen))
					.into(),
					Status::Errored(err) => Element::new(tooltip(
						labeled_icon_button(triangle_alert(), &*self.name, button_warning_text)
							.width(Fill)
							.on_press(Message::Action(self.id, Action::DirToggleOpen)),
						container(value(err).line_height(1.0))
							.padding(3)
							.style(container_with_radius(weak_bordered_box, 2)),
						tooltip::Position::Bottom,
					)),
				},
				if let Status::Loaded { dirs, files, open } = &self.children
					&& *open && !(dirs.is_empty() && files.is_empty())
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

	async fn load(path: Arc<Path>) -> Result<(Box<[Self]>, Box<[File]>), Arc<std::io::Error>> {
		let entry = smol::fs::read_dir(path).await?;

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

				if cfg!(target_os = "macos") && name == ".DS_STORE" {
					return;
				}

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

		files.sort_unstable_by(|(_, aname), (_, bname)| {
			natural_cmp(aname.as_encoded_bytes(), bname.as_encoded_bytes())
		});
		dirs.sort_unstable_by(|(_, aname), (_, bname)| {
			natural_cmp(aname.as_encoded_bytes(), bname.as_encoded_bytes())
		});

		let files = files.into_iter().map(|(file, _)| file).collect();
		let dirs = dirs.into_iter().map(|(dir, _)| dir).collect();

		Ok((dirs, files))
	}

	pub fn path(&self) -> &Path {
		&self.path
	}
}
