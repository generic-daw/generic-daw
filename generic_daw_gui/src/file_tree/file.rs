use super::Message;
use crate::{
	icons::{file, file_music},
	widget::{LINE_HEIGHT, shaping_of},
};
use iced::{
	Element, Fill,
	widget::{
		button, row, text,
		text::{Shaping, Wrapping},
	},
};
use smol::io::AsyncReadExt as _;
use std::{io, path::Path, sync::Arc};

#[derive(Clone, Debug)]
pub struct File {
	path: Arc<Path>,
	name: Arc<str>,
	shaping: Shaping,
	is_music: bool,
}

impl File {
	pub async fn new(path: impl AsRef<Path>) -> Self {
		let path = path.as_ref();
		let name = path.file_name().unwrap().to_str().unwrap();
		let shaping = shaping_of(name);

		let is_music = is_music(path).await.unwrap_or_default();

		Self {
			path: path.into(),
			name: name.into(),
			shaping,
			is_music,
		}
	}

	pub fn view(&self) -> (Element<'_, Message>, f32) {
		(
			button(
				row![
					if self.is_music { file_music() } else { file() },
					text(&*self.name)
						.shaping(self.shaping)
						.wrapping(Wrapping::None)
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
