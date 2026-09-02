use crate::{
	components::virtualized,
	file_tree::Message,
	icons::{file, file_headphone, file_music, file_play, file_video_camera, play},
	widget::LINE_HEIGHT,
};
use generic_daw_widget::stateful::Stateful;
use iced::{
	Element, Fill,
	widget::{button, mouse_area, row, text},
};
use infer::{audio::is_midi, is_audio, is_video};
use smol::io::AsyncReadExt as _;
use std::{io, path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FileKind {
	Midi,
	Audio,
	Video,
	Project,
	#[default]
	Unknown,
}

#[derive(Clone, Debug)]
pub struct File {
	path: Arc<Path>,
	name: Arc<str>,
	kind: FileKind,
}

impl File {
	pub async fn new(path: impl AsRef<Path>) -> Self {
		let path = path.as_ref();
		let name = path.file_name().unwrap().to_str().unwrap();

		let kind = file_kind(path).await.unwrap_or_default();

		Self {
			path: path.into(),
			name: name.into(),
			kind,
		}
	}

	pub fn view<'a>(&'a self, audio_preview: Option<&'a Path>) -> (Element<'a, Message>, f32) {
		#[derive(Clone)]
		enum Event {
			Press,
			Release,
			Exit,
		}

		(
			virtualized(move || {
				button(Stateful::new(
					|state, event| match event {
						Event::Press => {
							*state = true;
							None
						}
						Event::Release => std::mem::take(state)
							.then(|| Message::OpenFile(self.path.clone(), self.kind)),
						Event::Exit => std::mem::take(state)
							.then(|| Message::DragFile(self.path.clone(), self.kind)),
					},
					move |_| {
						mouse_area(
							row![
								if Some(&*self.path) == audio_preview {
									play()
								} else {
									match self.kind {
										FileKind::Midi => file_music(),
										FileKind::Audio => file_headphone(),
										FileKind::Video => file_video_camera(),
										FileKind::Project => file_play(),
										FileKind::Unknown => file(),
									}
								},
								text(&*self.name)
									.wrapping(text::Wrapping::None)
									.ellipsis(text::Ellipsis::End)
							]
							.padding(1)
							.spacing(2)
							.width(Fill),
						)
						.on_press(Event::Press)
						.on_release(Event::Release)
						.on_exit(Event::Exit)
						.into()
					},
				))
				.padding(0)
				.height(LINE_HEIGHT + 2.0)
				.style(button::text)
				.on_press_with(|| unreachable!())
				.into()
			}),
			LINE_HEIGHT + 2.0,
		)
	}

	pub fn name(&self) -> &str {
		&self.name
	}
}

async fn file_kind(path: &Path) -> io::Result<FileKind> {
	let mut file = smol::fs::File::open(path).await?;
	let limit = file.metadata().await?.len().min(257) as usize;
	let buf = &mut [0; 257][..limit];
	file.read_exact(buf).await?;
	Ok(if is_midi(buf) {
		FileKind::Midi
	} else if is_audio(buf) {
		FileKind::Audio
	} else if is_video(buf) {
		FileKind::Video
	} else if buf.get(..3) == Some(b"gdp") {
		FileKind::Project
	} else {
		FileKind::Unknown
	})
}
