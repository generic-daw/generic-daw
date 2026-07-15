use crate::{
	components::{context_menu_entry, labeled_icon_button},
	file_tree::{Action, Message, file::File},
	icons::{chevron_down, chevron_right, folder_open, folder_sync, hourglass, triangle_alert},
	stylefns::{
		button_warning_text, container_with_radius, weak_bordered_box, weaker_bordered_box,
	},
	widget::LINE_HEIGHT,
};
use generic_daw_widget::context_menu::ContextMenu;
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
	Syncing {
		dirs: Vec<Dir>,
		files: Vec<File>,
		open: bool,
	},
	Loaded {
		dirs: Vec<Dir>,
		files: Vec<File>,
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
				Action::ToggleOpen => {
					if let Status::Loaded { open, .. } = &mut self.children {
						*open ^= true;
						Task::none()
					} else {
						self.children = Status::Loading;
						Task::perform(Self::load(self.path.clone()), move |res| {
							Message::Action(id, Action::Loaded(res))
						})
					}
				}
				Action::Sync => {
					if let Status::Loaded { dirs, files, open } = &mut self.children {
						self.children = Status::Syncing {
							dirs: std::mem::take(dirs),
							files: std::mem::take(files),
							open: *open,
						};

						Task::perform(Self::load(self.path.clone()), move |res| {
							Message::Action(id, Action::Loaded(res))
						})
					} else {
						Task::none()
					}
				}
				Action::Loaded(res) => match res.clone() {
					Ok((mut dirs, files)) => {
						let mut tasks = Vec::new();
						let mut open = true;

						if let Status::Syncing {
							dirs: old_dirs,
							open: old_open,
							..
						} = &mut self.children
						{
							open = *old_open;

							for (i, dir) in dirs.iter().enumerate() {
								let j = old_dirs[i..]
									.iter()
									.position(|old_dir| old_dir.path() == dir.path())
									.unwrap_or_default();
								old_dirs.drain(i..i + j);

								if let Some(old_dir) = old_dirs.get_mut(i) {
									if old_dir.path() == dir.path() {
										tasks.push(
											old_dir.update(old_dir.id, &Action::Sync).unwrap(),
										);
									} else {
										old_dirs.insert(i, dir.clone());
									}
								} else {
									old_dirs.push(dir.clone());
								}
							}

							old_dirs.truncate(dirs.len());

							dirs = std::mem::take(old_dirs);
						}

						self.children = Status::Loaded { dirs, files, open };

						Task::batch(tasks)
					}
					Err(err) => {
						self.children = Status::Errored(err);
						Task::none()
					}
				},
			})
		} else if let Status::Loaded { dirs, .. } = &mut self.children {
			dirs.iter_mut().find_map(|dir| dir.update(id, action))
		} else {
			None
		}
	}

	pub fn view(&self) -> (Element<'_, Message>, f32) {
		let mut height = 0.0;
		(
			column![
				ContextMenu::new(
					match &self.children {
						Status::Unloaded =>
							labeled_icon_button(chevron_right(), &*self.name, button::text)
								.width(Fill)
								.on_press(Message::Action(self.id, Action::ToggleOpen))
								.into(),
						Status::Loading | Status::Syncing { .. } =>
							labeled_icon_button(hourglass(), &*self.name, button::text)
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
						.on_press(Message::Action(self.id, Action::ToggleOpen))
						.into(),
						Status::Errored(err) => Element::new(tooltip(
							labeled_icon_button(triangle_alert(), &*self.name, button_warning_text)
								.width(Fill)
								.on_press(Message::Action(self.id, Action::ToggleOpen)),
							container(value(err).line_height(1.0))
								.padding(3)
								.style(container_with_radius(weak_bordered_box, 2)),
							tooltip::Position::Bottom,
						)),
					},
					container(column![
						context_menu_entry(folder_sync(), "Sync", "").on_press_maybe(
							matches!(self.children, Status::Loaded { .. })
								.then_some(Message::Action(self.id, Action::Sync))
						),
						context_menu_entry(
							folder_open(),
							concat!(
								"Open in ",
								cfg_select!(
								target_os = "windows" => "File Explorer",
								target_os = "macos" => "Finder",
								_ => "File Manager")
							),
							""
						)
						.on_press(Message::OpenDir(self.path.clone())),
					])
					.width(if cfg!(target_os = "macos") { 160 } else { 200 })
					.style(container_with_radius(weaker_bordered_box, 5)),
				),
				if let Status::Loaded {
					dirs,
					files,
					open: true,
				}
				| Status::Syncing {
					dirs,
					files,
					open: true,
				} = &self.children
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

	async fn load(path: Arc<Path>) -> Result<(Vec<Self>, Vec<File>), Arc<std::io::Error>> {
		let entry = smol::fs::read_dir(path).await?;

		let files = smol::lock::Mutex::new(Vec::new());
		let dirs = smol::lock::Mutex::new(Vec::new());

		entry
			.into_stream()
			.for_each_concurrent(None, async |entry| {
				let Ok(entry) = entry else {
					return;
				};

				if cfg!(target_os = "macos") && entry.file_name() == ".DS_STORE" {
					return;
				}

				let Ok(ty) = smol::fs::metadata(entry.path()).await else {
					return;
				};

				if ty.is_file() {
					let file = File::new(entry.path()).await;
					files.lock().await.push(file);
				} else if ty.is_dir() {
					let dir = Self::new(entry.path());
					dirs.lock().await.push(dir);
				}
			})
			.await;

		let mut files = files.into_inner();
		let mut dirs = dirs.into_inner();

		dirs.sort_unstable_by(|a, b| natural_cmp(a.name.as_bytes(), b.name.as_bytes()));
		files.sort_unstable_by(|a, b| natural_cmp(a.name().as_bytes(), b.name().as_bytes()));

		Ok((dirs, files))
	}

	pub fn path(&self) -> &Path {
		&self.path
	}
}
