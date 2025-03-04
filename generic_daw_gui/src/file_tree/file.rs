use crate::{
    arrangement_view::Message as ArrangementMessage,
    components::styled_button,
    daw::Message as DawMessage,
    widget::{FileTreeEntry, LINE_HEIGHT},
};
use iced::{
    Element,
    widget::{mouse_area, svg},
};
use std::{path::Path, sync::LazyLock};

static FILE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../../assets/material-symbols--draft-outline-rounded.svg"
    ))
});

pub struct File {
    path: Box<Path>,
    name: Box<str>,
}

impl File {
    pub fn new(path: &Path) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap().into();

        Self {
            path: path.into(),
            name,
        }
    }

    pub fn view(&self) -> (Element<'_, DawMessage>, f32) {
        (
            mouse_area(
                styled_button(FileTreeEntry::new(&self.name, FILE.clone()))
                    .on_press(DawMessage::FileTree(self.path.clone()))
                    .padding(0),
            )
            .on_double_click(DawMessage::Arrangement(ArrangementMessage::LoadSample(
                self.path.clone(),
            )))
            .into(),
            LINE_HEIGHT,
        )
    }
}
