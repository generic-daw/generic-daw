use super::Message;
use crate::{
    components::styled_button,
    icons::{file, file_music},
    widget::{LINE_HEIGHT, shaping_of},
};
use iced::{
    Element, Fill,
    widget::{
        container, row, text,
        text::{Shaping, Wrapping},
    },
};
use std::{
    fs,
    io::{self, Read as _},
    path::Path,
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct File {
    path: Arc<Path>,
    name: Arc<str>,
    shaping: Shaping,
    is_audio: bool,
}

impl File {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let name = path.file_name().unwrap().to_str().unwrap();
        let shaping = shaping_of(name);

        let is_audio = is_audio(path).unwrap_or_default();

        Self {
            path: path.into(),
            name: name.into(),
            shaping,
            is_audio,
        }
    }

    pub fn view(&self) -> (Element<'_, Message>, f32) {
        (
            styled_button(row![
                container(if self.is_audio { file_music() } else { file() }).clip(true),
                container(
                    text(&*self.name)
                        .shaping(self.shaping)
                        .wrapping(Wrapping::None)
                )
                .clip(true)
            ])
            .width(Fill)
            .padding(0)
            .on_press(Message::File(self.path.clone()))
            .into(),
            LINE_HEIGHT,
        )
    }
}

pub fn is_audio(path: &Path) -> io::Result<bool> {
    let file = fs::File::open(path)?;
    let limit = file.metadata()?.len().min(36);
    let mut buf = Vec::with_capacity(36);
    file.take(limit).read_to_end(&mut buf)?;
    Ok(infer::get(&buf).is_some_and(|x| x.matcher_type() == infer::MatcherType::Audio))
}
