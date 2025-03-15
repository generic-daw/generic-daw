use iced::widget::svg;
use std::sync::LazyLock;

pub static AUDIO_FILE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--audio-file-outline-rounded.svg"
    ))
});

pub static CANCEL: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--cancel-rounded.svg"
    ))
});

pub static CHEVRON_RIGHT: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--chevron-right-rounded.svg"
    ))
});

pub static GENERIC_FILE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--draft-outline-rounded.svg"
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

pub static REOPEN: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--reopen-window-rounded.svg"
    ))
});

pub static STOP: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--stop-rounded.svg"
    ))
});
