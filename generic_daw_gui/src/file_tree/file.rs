use crate::{
	file_tree::Message,
	icons::{file, file_headphone, file_music},
	widget::LINE_HEIGHT,
};
use iced::{
	Element, Fill,
	futures::AsyncReadExt as _,
	widget::{button, mouse_area, row, text},
};
use infer::{audio::is_midi, is_audio};
use std::{io, path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, Default)]
enum Icon {
	FileMusic,
	FileHeadphone,
	#[default]
	File,
}

#[derive(Clone, Debug)]
pub struct File {
	path: Arc<Path>,
	name: Arc<str>,
	icon: Icon,
}

impl File {
	pub async fn new(path: impl AsRef<Path>) -> Self {
		let path = path.as_ref();
		let name = path.file_name().unwrap().to_str().unwrap();

		let icon = icon(path).await.unwrap_or_default();

		Self {
			path: path.into(),
			name: name.into(),
			icon,
		}
	}

	pub fn view(&self) -> (Element<'_, Message>, f32) {
		(
			button(
				mouse_area(
					row![
						match self.icon {
							Icon::FileMusic => file_music,
							Icon::FileHeadphone => file_headphone,
							Icon::File => file,
						}(),
						text(&*self.name).wrapping(text::Wrapping::None)
					]
					.padding(1)
					.spacing(2)
					.width(Fill),
				)
				.on_press(Message::File(self.path.clone())),
			)
			.padding(0)
			.style(button::text)
			.on_press_with(|| unreachable!())
			.into(),
			LINE_HEIGHT + 2.0,
		)
	}
}

async fn icon(path: &Path) -> io::Result<Icon> {
	let mut file = smol::fs::File::open(path).await?;
	let limit = file.metadata().await?.len() as usize;
	let buf = &mut [0; 36][..limit.min(36)];
	file.read_exact(buf).await?;
	Ok(if is_midi(buf) {
		Icon::FileMusic
	} else if is_audio(buf) {
		Icon::FileHeadphone
	} else {
		Icon::File
	})
}
