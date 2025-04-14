use super::Message;
use crate::{
    components::{styled_button, styled_svg},
    icons::{AUDIO_FILE, GENERIC_FILE},
    widget::{Clipped, LINE_HEIGHT, shaping_of},
};
use iced::{
    Element, Length,
    widget::{
        container, mouse_area, row, svg, text,
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
    icon: svg::Handle,
}

impl File {
    pub fn new(path: &Path) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap();
        let shaping = shaping_of(name);

        let icon = if is_audio(path).unwrap_or_default() {
            AUDIO_FILE.clone()
        } else {
            GENERIC_FILE.clone()
        };

        Self {
            path: path.into(),
            name: name.into(),
            shaping,
            icon,
        }
    }

    pub fn view(&self) -> (Element<'_, Message>, f32) {
        (
            mouse_area(
                styled_button(row![
                    Clipped::new(styled_svg(self.icon.clone()).height(LINE_HEIGHT)),
                    container(
                        text(&*self.name)
                            .shaping(self.shaping)
                            .wrapping(Wrapping::None)
                    )
                    .clip(true)
                ])
                .width(Length::Fill)
                .padding(0)
                .on_press(Message::None),
            )
            .on_double_click(Message::File(self.path.clone()))
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
