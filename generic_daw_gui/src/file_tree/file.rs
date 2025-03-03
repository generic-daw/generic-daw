use crate::{
    arrangement_view::Message as ArrangementMessage,
    daw::Message as DawMessage,
    widget::{FileTreeEntry, LINE_HEIGHT},
};
use iced::{Element, widget::svg};
use std::{path::Path, sync::LazyLock};

static FILE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../../assets/material-symbols--draft-outline-rounded.svg"
    ))
});

pub struct File {
    pub path: Box<Path>,
}

impl File {
    pub fn new(path: &Path) -> Self {
        Self {
            path: Box::from(path),
        }
    }

    pub fn view(&self) -> (Element<'_, DawMessage>, f32) {
        (
            FileTreeEntry::new(&self.path, FILE.clone())
                .on_double_click(|p| {
                    DawMessage::Arrangement(ArrangementMessage::LoadSample(Box::from(p)))
                })
                .into(),
            LINE_HEIGHT,
        )
    }
}
