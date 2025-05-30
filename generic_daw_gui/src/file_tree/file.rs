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
    is_music: bool,
}

impl File {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let name = path.file_name().unwrap().to_str().unwrap();
        let shaping = shaping_of(name);

        let is_music = is_music(path).unwrap_or_default();

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

pub fn is_music(path: &Path) -> io::Result<bool> {
    let file = fs::File::open(path)?;
    let limit = file.metadata()?.len().min(36);
    let mut buf = Vec::with_capacity(36);
    file.take(limit).read_to_end(&mut buf)?;
    Ok(infer::get(&buf).is_some_and(|x| x.matcher_type() == infer::MatcherType::Audio))
}
