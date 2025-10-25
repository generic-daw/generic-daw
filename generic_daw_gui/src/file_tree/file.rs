use crate::{
	file_tree::Message,
	icons::{file, file_music},
	widget::LINE_HEIGHT,
};
use iced::{
	Element, Fill,
	futures::AsyncReadExt as _,
	widget::{button, row, text},
};
use std::{io, path::Path, sync::Arc};

#[derive(Clone, Debug)]
pub struct File {
	path: Arc<Path>,
	name: Arc<str>,
	is_music: bool,
}

impl File {
	pub async fn new(path: impl AsRef<Path>) -> Self {
		let path = path.as_ref();
		let name = path.file_name().unwrap().to_str().unwrap();

		let is_music = is_music(path).await.unwrap_or_default();

		Self {
			path: path.into(),
			name: name.into(),
			is_music,
		}
	}

	pub fn view(&self) -> (Element<'_, Message>, f32) {
		(
			button(
				row![
					if self.is_music { file_music() } else { file() },
					text(&*self.name).wrapping(text::Wrapping::None)
				]
				.spacing(2),
			)
			.style(button::text)
			.padding(1)
			.width(Fill)
			.on_press(Message::File(self.path.clone()))
			.into(),
			LINE_HEIGHT + 2.0,
		)
	}
}

pub async fn is_music(path: &Path) -> io::Result<bool> {
	let mut file = smol::fs::File::open(path).await?;
	let limit = file.metadata().await?.len() as usize;
	let buf = &mut [0; 36][..limit.min(36)];
	file.read_exact(buf).await?;
	Ok(infer::get(buf).is_some_and(|x| x.matcher_type() == infer::MatcherType::Audio))
}
