use crate::{
	file_tree::Message,
	icons::{file, file_headphone, file_music, file_play},
	widget::LINE_HEIGHT,
};
use iced::{
	Element, Fill,
	futures::AsyncReadExt as _,
	widget::{button, mouse_area, row, text},
};
use infer::{audio::is_midi, is_audio};
use std::{io, path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FileKind {
	Midi,
	Audio,
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

		let icon = file_kind(path).await.unwrap_or_default();

		Self {
			path: path.into(),
			name: name.into(),
			kind: icon,
		}
	}

	pub fn view(&self) -> (Element<'_, Message>, f32) {
		(
			button(
				mouse_area(
					row![
						match self.kind {
							FileKind::Midi => file_music(),
							FileKind::Audio => file_headphone(),
							FileKind::Project => file_play(),
							FileKind::Unknown => file(),
						},
						text(&*self.name)
							.wrapping(text::Wrapping::None)
							.ellipsis(text::Ellipsis::End)
					]
					.padding(1)
					.spacing(2)
					.width(Fill),
				)
				.on_press(Message::File(self.path.clone(), self.kind)),
			)
			.padding(0)
			.style(button::text)
			.on_press_with(|| unreachable!())
			.into(),
			LINE_HEIGHT + 2.0,
		)
	}
}

async fn file_kind(path: &Path) -> io::Result<FileKind> {
	let mut file = smol::fs::File::open(path).await?;
	let limit = file.metadata().await?.len() as usize;
	let buf = &mut [0; 36][..limit.min(36)];
	file.read_exact(buf).await?;
	Ok(if is_midi(buf) {
		FileKind::Midi
	} else if is_audio(buf) {
		FileKind::Audio
	} else if buf.get(..3) == Some(b"gdp") {
		FileKind::Project
	} else {
		FileKind::Unknown
	})
}
