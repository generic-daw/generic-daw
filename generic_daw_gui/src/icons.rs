use iced::widget::svg;
use std::sync::LazyLock;

pub static AUDIO_FILE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--audio-file-outline-rounded.svg"
    ))
});

pub static CHEVRON_RIGHT: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--chevron-right-rounded.svg"
    ))
});

pub static CIRCLE_LINE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--line-end-circle-rounded.svg"
    ))
});

pub static CIRCLE_LINE_OUTLINE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--line-end-circle-outline-rounded.svg"
    ))
});

pub static GENERIC_FILE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--draft-outline-rounded.svg"
    ))
});

pub static HANDLE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--drag-handle-rounded.svg"
    ))
});

pub static RECORD: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--fiber-manual-record.svg"
    ))
});

pub static PAUSE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--pause-rounded.svg"
    ))
});

pub static PLAY: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--play-arrow-rounded.svg"
    ))
});

pub static STOP: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--stop-rounded.svg"
    ))
});
