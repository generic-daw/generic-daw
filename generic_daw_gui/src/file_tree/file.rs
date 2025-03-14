use crate::{
    arrangement_view::Message as ArrangementMessage,
    components::styled_button,
    daw::Message as DawMessage,
    icons::{AUDIO_FILE, GENERIC_FILE},
    widget::{FileTreeEntry, LINE_HEIGHT},
};
use generic_daw_core::Position;
use iced::{
    Element,
    widget::{mouse_area, svg},
};
use std::{
    cell::RefCell,
    fs,
    io::{self, Read as _},
    path::Path,
};

pub struct File {
    path: Box<Path>,
    name: Box<str>,
    icon: svg::Handle,
}

impl File {
    pub fn new(path: &Path) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap().into();
        let icon = if is_audio(path).unwrap_or_default() {
            AUDIO_FILE.clone()
        } else {
            GENERIC_FILE.clone()
        };

        Self {
            path: path.into(),
            name,
            icon,
        }
    }

    pub fn view(&self) -> (Element<'_, DawMessage>, f32) {
        (
            mouse_area(
                styled_button(FileTreeEntry::new(&self.name, self.icon.clone()))
                    .on_press(DawMessage::FileTree(self.path.clone()))
                    .padding(0),
            )
            .on_double_click(DawMessage::Arrangement(ArrangementMessage::LoadSample(
                self.path.clone(),
                Position::default(),
            )))
            .into(),
            LINE_HEIGHT,
        )
    }
}

pub fn is_audio(path: &Path) -> io::Result<bool> {
    thread_local! { static BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(36)) };
    let file = fs::File::open(path)?;
    let limit = file.metadata()?.len().min(36);
    BUF.with_borrow_mut(|buf| {
        buf.clear();
        file.take(limit).read_to_end(buf)?;
        Ok(infer::get(buf).is_some_and(|x| x.matcher_type() == infer::MatcherType::Audio))
    })
}
